// Added: Plugin executor service for TMAIL-132
// PURPOSE: Executes registered plugins when email events occur
// EXTERNAL: Uses reqwest for webhook HTTP calls

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::plugin::{Plugin, PluginExecution, PluginHook, PluginType};

/// PURPOSE: Context passed to plugins during hook execution
/// NOTE: Contains all relevant information about the email event
#[derive(Debug, Clone, serde::Serialize)]
pub struct HookContext {
    pub event: String,
    pub message_uid: Option<u32>,
    pub folder: Option<String>,
    pub subject: Option<String>,
    pub from: Option<String>,
}

/// PURPOSE: Result of a single plugin execution
#[derive(Debug, Clone)]
pub struct PluginResult {
    pub plugin_id: Uuid,
    pub plugin_name: String,
    pub success: bool,
    pub duration_ms: i32,
    pub error: Option<String>,
}

/// PURPOSE: Execute all enabled plugins for a given hook and user
/// CONSTRAINTS: Each plugin has a 10s timeout for webhook calls
pub async fn execute_hook(
    pool: &PgPool,
    user_id: Uuid,
    hook: PluginHook,
    context: &HookContext,
) -> Vec<PluginResult> {
    let hook_str = hook.to_string();

    // Added: Query enabled plugins subscribed to this hook
    let plugins = match Plugin::list_enabled_for_hook(pool, user_id, &hook_str).await {
        Ok(p) => p,
        Err(err) => {
            tracing::error!(
                "Failed to query plugins for user_id={}, hook={}: {}",
                user_id,
                hook_str,
                err
            );
            return vec![];
        }
    };

    if plugins.is_empty() {
        return vec![];
    }

    let mut results = Vec::new();

    for plugin in &plugins {
        let result = match PluginType::from_str(&plugin.plugin_type) {
            Some(PluginType::Webhook) => execute_webhook_plugin(pool, plugin, context).await,
            Some(PluginType::Filter) => execute_filter_plugin(pool, plugin, context).await,
            Some(PluginType::Script) => {
                // NOTE: Script execution is not yet implemented
                PluginResult {
                    plugin_id: plugin.id,
                    plugin_name: plugin.name.clone(),
                    success: false,
                    duration_ms: 0,
                    error: Some("Script plugins are not yet supported".to_string()),
                }
            }
            None => {
                tracing::warn!(
                    "Unknown plugin type '{}' for plugin {}",
                    plugin.plugin_type,
                    plugin.id
                );
                PluginResult {
                    plugin_id: plugin.id,
                    plugin_name: plugin.name.clone(),
                    success: false,
                    duration_ms: 0,
                    error: Some(format!("Unknown plugin type: {}", plugin.plugin_type)),
                }
            }
        };

        // Added: Record execution in the database
        let status = if result.success { "success" } else { "error" };
        let _ = PluginExecution::create(
            pool,
            plugin.id,
            &context.event,
            status,
            Some(result.duration_ms),
            result.error.clone(),
        )
        .await;

        results.push(result);
    }

    results
}

/// PURPOSE: Execute a webhook plugin by POSTing the context to the configured URL
/// CONSTRAINTS: 10 second timeout, URL must be in plugin config
pub async fn execute_webhook_plugin(
    pool: &PgPool,
    plugin: &Plugin,
    context: &HookContext,
) -> PluginResult {
    let _ = pool; // NOTE: Reserved for future use (recording additional data)

    let url = match plugin.config.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.to_string(),
        None => {
            return PluginResult {
                plugin_id: plugin.id,
                plugin_name: plugin.name.clone(),
                success: false,
                duration_ms: 0,
                error: Some("Webhook plugin config missing 'url' field".to_string()),
            };
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let payload = serde_json::to_value(context).unwrap_or_default();
    let start = std::time::Instant::now();

    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-Plugin-Event", &context.event)
        .header("X-Plugin-Name", &plugin.name)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            let duration_ms = start.elapsed().as_millis() as i32;
            let status = resp.status().as_u16();
            let success = (200..300).contains(&(status as i32));

            if !success {
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    "Plugin '{}' webhook to {} returned status {}: {}",
                    plugin.name,
                    url,
                    status,
                    body.chars().take(200).collect::<String>()
                );
                PluginResult {
                    plugin_id: plugin.id,
                    plugin_name: plugin.name.clone(),
                    success: false,
                    duration_ms,
                    error: Some(format!("HTTP {}", status)),
                }
            } else {
                PluginResult {
                    plugin_id: plugin.id,
                    plugin_name: plugin.name.clone(),
                    success: true,
                    duration_ms,
                    error: None,
                }
            }
        }
        Err(err) => {
            let duration_ms = start.elapsed().as_millis() as i32;
            tracing::error!(
                "Plugin '{}' webhook to {} failed: {}",
                plugin.name,
                url,
                err
            );
            PluginResult {
                plugin_id: plugin.id,
                plugin_name: plugin.name.clone(),
                success: false,
                duration_ms,
                error: Some(format!("Connection error: {}", err)),
            }
        }
    }
}

/// PURPOSE: Execute a filter plugin by applying rules from config to the context
/// NOTE: Filter rules use simple field matching (from, subject patterns)
pub async fn execute_filter_plugin(
    _pool: &PgPool,
    plugin: &Plugin,
    context: &HookContext,
) -> PluginResult {
    let start = std::time::Instant::now();

    // Added: Extract filter rules from plugin config
    let rules = match plugin.config.get("rules").and_then(|v| v.as_array()) {
        Some(r) => r,
        None => {
            let duration_ms = start.elapsed().as_millis() as i32;
            return PluginResult {
                plugin_id: plugin.id,
                plugin_name: plugin.name.clone(),
                success: true,
                duration_ms,
                error: None, // NOTE: No rules means nothing to filter — not an error
            };
        }
    };

    // Added: Apply each filter rule against the context
    let mut matched = false;
    for rule in rules {
        let field = rule.get("field").and_then(|v| v.as_str()).unwrap_or("");
        let pattern = rule.get("pattern").and_then(|v| v.as_str()).unwrap_or("");

        if pattern.is_empty() {
            continue;
        }

        let value = match field {
            "from" => context.from.as_deref().unwrap_or(""),
            "subject" => context.subject.as_deref().unwrap_or(""),
            "folder" => context.folder.as_deref().unwrap_or(""),
            "event" => &context.event,
            _ => continue,
        };

        // NOTE: Simple case-insensitive contains match
        if value.to_lowercase().contains(&pattern.to_lowercase()) {
            matched = true;
            break;
        }
    }

    let duration_ms = start.elapsed().as_millis() as i32;

    PluginResult {
        plugin_id: plugin.id,
        plugin_name: plugin.name.clone(),
        success: true,
        duration_ms,
        error: if matched {
            None
        } else {
            Some("No filter rules matched".to_string())
        },
    }
}

/// PURPOSE: Build a HookContext for a test-fire from the handler
pub fn build_test_context(hook: &str) -> HookContext {
    HookContext {
        event: hook.to_string(),
        message_uid: Some(12345),
        folder: Some("INBOX".to_string()),
        subject: Some("Test plugin execution".to_string()),
        from: Some("test@example.com".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_context_serialization() {
        let ctx = HookContext {
            event: "on_receive".to_string(),
            message_uid: Some(42),
            folder: Some("INBOX".to_string()),
            subject: Some("Hello World".to_string()),
            from: Some("alice@example.com".to_string()),
        };

        let json = serde_json::to_value(&ctx).unwrap();
        assert_eq!(json["event"], "on_receive");
        assert_eq!(json["message_uid"], 42);
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["subject"], "Hello World");
        assert_eq!(json["from"], "alice@example.com");
    }

    #[test]
    fn test_hook_context_with_none_fields() {
        let ctx = HookContext {
            event: "on_delete".to_string(),
            message_uid: None,
            folder: None,
            subject: None,
            from: None,
        };

        let json = serde_json::to_value(&ctx).unwrap();
        assert_eq!(json["event"], "on_delete");
        assert!(json["message_uid"].is_null());
        assert!(json["folder"].is_null());
    }

    #[test]
    fn test_plugin_result_success() {
        let result = PluginResult {
            plugin_id: Uuid::new_v4(),
            plugin_name: "Test Plugin".to_string(),
            success: true,
            duration_ms: 150,
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.duration_ms, 150);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_plugin_result_failure() {
        let result = PluginResult {
            plugin_id: Uuid::new_v4(),
            plugin_name: "Broken Plugin".to_string(),
            success: false,
            duration_ms: 10000,
            error: Some("Connection timeout".to_string()),
        };

        assert!(!result.success);
        assert_eq!(result.error.unwrap(), "Connection timeout");
    }

    #[test]
    fn test_build_test_context() {
        let ctx = build_test_context("on_receive");
        assert_eq!(ctx.event, "on_receive");
        assert_eq!(ctx.message_uid, Some(12345));
        assert_eq!(ctx.folder.as_deref(), Some("INBOX"));
        assert!(ctx.subject.is_some());
        assert!(ctx.from.is_some());
    }

    #[test]
    fn test_build_test_context_different_hooks() {
        let hooks = ["on_receive", "on_send", "on_delete", "on_move", "on_flag", "on_read"];
        for hook in &hooks {
            let ctx = build_test_context(hook);
            assert_eq!(ctx.event, *hook);
        }
    }

    #[test]
    fn test_hook_context_clone() {
        let ctx = HookContext {
            event: "on_send".to_string(),
            message_uid: Some(99),
            folder: Some("Sent".to_string()),
            subject: Some("Meeting".to_string()),
            from: Some("bob@example.com".to_string()),
        };

        let cloned = ctx.clone();
        assert_eq!(cloned.event, ctx.event);
        assert_eq!(cloned.message_uid, ctx.message_uid);
        assert_eq!(cloned.folder, ctx.folder);
    }

    #[test]
    fn test_hook_context_debug() {
        let ctx = HookContext {
            event: "on_flag".to_string(),
            message_uid: Some(1),
            folder: None,
            subject: None,
            from: None,
        };

        // NOTE: Verify Debug trait is implemented
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("on_flag"));
    }

    #[test]
    fn test_plugin_result_with_http_error() {
        let result = PluginResult {
            plugin_id: Uuid::new_v4(),
            plugin_name: "Webhook Plugin".to_string(),
            success: false,
            duration_ms: 250,
            error: Some("HTTP 500".to_string()),
        };

        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("HTTP 500"));
    }

    #[test]
    fn test_plugin_result_with_missing_url_error() {
        let result = PluginResult {
            plugin_id: Uuid::new_v4(),
            plugin_name: "Bad Config Plugin".to_string(),
            success: false,
            duration_ms: 0,
            error: Some("Webhook plugin config missing 'url' field".to_string()),
        };

        assert!(!result.success);
        assert!(result.error.unwrap().contains("missing"));
    }

    #[test]
    fn test_hook_context_json_roundtrip() {
        let ctx = HookContext {
            event: "on_move".to_string(),
            message_uid: Some(777),
            folder: Some("Archive".to_string()),
            subject: Some("Important".to_string()),
            from: Some("ceo@company.com".to_string()),
        };

        let json_str = serde_json::to_string(&ctx).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["event"], "on_move");
        assert_eq!(parsed["message_uid"], 777);
        assert_eq!(parsed["folder"], "Archive");
    }
}
