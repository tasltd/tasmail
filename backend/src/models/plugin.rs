// Added: Plugin model for extensible plugin/extension architecture (TMAIL-132)
// PURPOSE: Defines Plugin and PluginExecution structs with CRUD operations

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Types of plugins that can be registered
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Webhook,
    Script,
    Filter,
}

// Added: Display impl for database storage as text
impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginType::Webhook => write!(f, "webhook"),
            PluginType::Script => write!(f, "script"),
            PluginType::Filter => write!(f, "filter"),
        }
    }
}

impl PluginType {
    /// PURPOSE: Parse plugin type from database text column
    pub fn from_str(s: &str) -> Option<PluginType> {
        match s {
            "webhook" => Some(PluginType::Webhook),
            "script" => Some(PluginType::Script),
            "filter" => Some(PluginType::Filter),
            _ => None,
        }
    }
}

/// PURPOSE: Hook events that plugins can subscribe to
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PluginHook {
    OnReceive,
    OnSend,
    OnDelete,
    OnMove,
    OnFlag,
    OnRead,
}

impl std::fmt::Display for PluginHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginHook::OnReceive => write!(f, "on_receive"),
            PluginHook::OnSend => write!(f, "on_send"),
            PluginHook::OnDelete => write!(f, "on_delete"),
            PluginHook::OnMove => write!(f, "on_move"),
            PluginHook::OnFlag => write!(f, "on_flag"),
            PluginHook::OnRead => write!(f, "on_read"),
        }
    }
}

impl PluginHook {
    /// PURPOSE: Parse hook name from database text array element
    pub fn from_str(s: &str) -> Option<PluginHook> {
        match s {
            "on_receive" => Some(PluginHook::OnReceive),
            "on_send" => Some(PluginHook::OnSend),
            "on_delete" => Some(PluginHook::OnDelete),
            "on_move" => Some(PluginHook::OnMove),
            "on_flag" => Some(PluginHook::OnFlag),
            "on_read" => Some(PluginHook::OnRead),
            _ => None,
        }
    }
}

/// PURPOSE: A user-registered or system-wide plugin
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Plugin {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub plugin_type: String,
    pub config: serde_json::Value,
    pub hooks: Vec<String>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: A record of a single plugin execution attempt
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PluginExecution {
    pub id: Uuid,
    pub plugin_id: Uuid,
    pub event: String,
    pub status: String,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Request payload for creating a new plugin
#[derive(Debug, Deserialize)]
pub struct CreatePluginRequest {
    pub name: String,
    pub description: Option<String>,
    pub plugin_type: PluginType,
    pub config: Option<serde_json::Value>,
    pub hooks: Vec<PluginHook>,
    pub enabled: Option<bool>,
}

/// PURPOSE: Request payload for updating an existing plugin
#[derive(Debug, Deserialize)]
pub struct UpdatePluginRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub plugin_type: Option<PluginType>,
    pub config: Option<serde_json::Value>,
    pub hooks: Option<Vec<PluginHook>>,
    pub enabled: Option<bool>,
}

impl Plugin {
    /// PURPOSE: List all plugins belonging to a user (plus system-wide plugins)
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Plugin>, sqlx::Error> {
        sqlx::query_as::<_, Plugin>(
            "SELECT * FROM plugins WHERE user_id = $1 OR user_id IS NULL ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single plugin by ID
    pub async fn get_by_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Plugin>, sqlx::Error> {
        sqlx::query_as::<_, Plugin>(
            "SELECT * FROM plugins WHERE id = $1 AND (user_id = $2 OR user_id IS NULL)",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create a new plugin
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        input: &CreatePluginRequest,
    ) -> Result<Plugin, sqlx::Error> {
        // Added: Convert hook enums to string array for database storage
        let hook_strings: Vec<String> = input.hooks.iter().map(|h| h.to_string()).collect();
        let config = input.config.clone().unwrap_or(serde_json::json!({}));
        let enabled = input.enabled.unwrap_or(true);

        sqlx::query_as::<_, Plugin>(
            "INSERT INTO plugins (user_id, name, description, plugin_type, config, hooks, enabled) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
        )
        .bind(user_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.plugin_type.to_string())
        .bind(&config)
        .bind(&hook_strings)
        .bind(enabled)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing plugin's configuration
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        input: &UpdatePluginRequest,
    ) -> Result<Option<Plugin>, sqlx::Error> {
        // Added: Convert optional hook enums to string array
        let hook_strings: Option<Vec<String>> = input
            .hooks
            .as_ref()
            .map(|hooks| hooks.iter().map(|h| h.to_string()).collect());
        let plugin_type_str: Option<String> = input.plugin_type.as_ref().map(|pt| pt.to_string());

        sqlx::query_as::<_, Plugin>(
            "UPDATE plugins SET \
                name = COALESCE($3, name), \
                description = COALESCE($4, description), \
                plugin_type = COALESCE($5, plugin_type), \
                config = COALESCE($6, config), \
                hooks = COALESCE($7, hooks), \
                enabled = COALESCE($8, enabled), \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&plugin_type_str)
        .bind(&input.config)
        .bind(&hook_strings)
        .bind(input.enabled)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete a plugin and all its execution records (cascade)
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM plugins WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Find all enabled plugins subscribed to a specific hook for a user
    /// NOTE: Used by the plugin executor to determine which plugins to fire
    pub async fn list_enabled_for_hook(
        pool: &PgPool,
        user_id: Uuid,
        hook: &str,
    ) -> Result<Vec<Plugin>, sqlx::Error> {
        sqlx::query_as::<_, Plugin>(
            "SELECT * FROM plugins \
             WHERE (user_id = $1 OR user_id IS NULL) \
               AND enabled = true \
               AND $2 = ANY(hooks) \
             ORDER BY created_at ASC",
        )
        .bind(user_id)
        .bind(hook)
        .fetch_all(pool)
        .await
    }
}

impl PluginExecution {
    /// PURPOSE: List recent executions for a plugin, most recent first
    pub async fn list_by_plugin(
        pool: &PgPool,
        plugin_id: Uuid,
    ) -> Result<Vec<PluginExecution>, sqlx::Error> {
        sqlx::query_as::<_, PluginExecution>(
            "SELECT * FROM plugin_executions WHERE plugin_id = $1 \
             ORDER BY executed_at DESC LIMIT 50",
        )
        .bind(plugin_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Record a plugin execution attempt
    pub async fn create(
        pool: &PgPool,
        plugin_id: Uuid,
        event: &str,
        status: &str,
        duration_ms: Option<i32>,
        error_message: Option<String>,
    ) -> Result<PluginExecution, sqlx::Error> {
        sqlx::query_as::<_, PluginExecution>(
            "INSERT INTO plugin_executions (plugin_id, event, status, duration_ms, error_message) \
             VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(plugin_id)
        .bind(event)
        .bind(status)
        .bind(duration_ms)
        .bind(error_message)
        .fetch_one(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_type_serialization() {
        // NOTE: Verify enum values match expected JSON representations
        let pt = PluginType::Webhook;
        let json = serde_json::to_value(&pt).unwrap();
        assert_eq!(json, "webhook");

        let pt = PluginType::Script;
        let json = serde_json::to_value(&pt).unwrap();
        assert_eq!(json, "script");

        let pt = PluginType::Filter;
        let json = serde_json::to_value(&pt).unwrap();
        assert_eq!(json, "filter");
    }

    #[test]
    fn test_plugin_type_deserialization() {
        let pt: PluginType = serde_json::from_str("\"webhook\"").unwrap();
        assert_eq!(pt, PluginType::Webhook);

        let pt: PluginType = serde_json::from_str("\"script\"").unwrap();
        assert_eq!(pt, PluginType::Script);

        let pt: PluginType = serde_json::from_str("\"filter\"").unwrap();
        assert_eq!(pt, PluginType::Filter);
    }

    #[test]
    fn test_plugin_type_invalid_deserialization() {
        let result = serde_json::from_str::<PluginType>("\"unknown\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_type_display() {
        assert_eq!(PluginType::Webhook.to_string(), "webhook");
        assert_eq!(PluginType::Script.to_string(), "script");
        assert_eq!(PluginType::Filter.to_string(), "filter");
    }

    #[test]
    fn test_plugin_type_from_str() {
        assert_eq!(PluginType::from_str("webhook"), Some(PluginType::Webhook));
        assert_eq!(PluginType::from_str("script"), Some(PluginType::Script));
        assert_eq!(PluginType::from_str("filter"), Some(PluginType::Filter));
        assert_eq!(PluginType::from_str("invalid"), None);
    }

    #[test]
    fn test_plugin_hook_serialization() {
        let hook = PluginHook::OnReceive;
        let json = serde_json::to_value(&hook).unwrap();
        assert_eq!(json, "on_receive");

        let hook = PluginHook::OnSend;
        let json = serde_json::to_value(&hook).unwrap();
        assert_eq!(json, "on_send");

        let hook = PluginHook::OnDelete;
        let json = serde_json::to_value(&hook).unwrap();
        assert_eq!(json, "on_delete");

        let hook = PluginHook::OnMove;
        let json = serde_json::to_value(&hook).unwrap();
        assert_eq!(json, "on_move");

        let hook = PluginHook::OnFlag;
        let json = serde_json::to_value(&hook).unwrap();
        assert_eq!(json, "on_flag");

        let hook = PluginHook::OnRead;
        let json = serde_json::to_value(&hook).unwrap();
        assert_eq!(json, "on_read");
    }

    #[test]
    fn test_plugin_hook_deserialization() {
        let hook: PluginHook = serde_json::from_str("\"on_receive\"").unwrap();
        assert_eq!(hook, PluginHook::OnReceive);

        let hook: PluginHook = serde_json::from_str("\"on_read\"").unwrap();
        assert_eq!(hook, PluginHook::OnRead);
    }

    #[test]
    fn test_plugin_hook_from_str() {
        assert_eq!(PluginHook::from_str("on_receive"), Some(PluginHook::OnReceive));
        assert_eq!(PluginHook::from_str("on_send"), Some(PluginHook::OnSend));
        assert_eq!(PluginHook::from_str("on_delete"), Some(PluginHook::OnDelete));
        assert_eq!(PluginHook::from_str("on_move"), Some(PluginHook::OnMove));
        assert_eq!(PluginHook::from_str("on_flag"), Some(PluginHook::OnFlag));
        assert_eq!(PluginHook::from_str("on_read"), Some(PluginHook::OnRead));
        assert_eq!(PluginHook::from_str("invalid"), None);
    }

    #[test]
    fn test_plugin_hook_display() {
        assert_eq!(PluginHook::OnReceive.to_string(), "on_receive");
        assert_eq!(PluginHook::OnSend.to_string(), "on_send");
        assert_eq!(PluginHook::OnDelete.to_string(), "on_delete");
        assert_eq!(PluginHook::OnMove.to_string(), "on_move");
        assert_eq!(PluginHook::OnFlag.to_string(), "on_flag");
        assert_eq!(PluginHook::OnRead.to_string(), "on_read");
    }

    #[test]
    fn test_plugin_hook_roundtrip() {
        let hooks = vec![
            PluginHook::OnReceive,
            PluginHook::OnSend,
            PluginHook::OnDelete,
            PluginHook::OnMove,
            PluginHook::OnFlag,
            PluginHook::OnRead,
        ];
        let json = serde_json::to_string(&hooks).unwrap();
        let deserialized: Vec<PluginHook> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, hooks);
    }

    #[test]
    fn test_plugin_serialization() {
        let plugin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let plugin = Plugin {
            id: plugin_id,
            user_id: Some(user_id),
            name: "My Webhook Plugin".to_string(),
            description: Some("Notifies on new email".to_string()),
            plugin_type: "webhook".to_string(),
            config: serde_json::json!({"url": "https://example.com/hook"}),
            hooks: vec!["on_receive".to_string(), "on_send".to_string()],
            enabled: true,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(&plugin).unwrap();
        assert_eq!(json["id"], plugin_id.to_string());
        assert_eq!(json["name"], "My Webhook Plugin");
        assert_eq!(json["plugin_type"], "webhook");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["hooks"].as_array().unwrap().len(), 2);
        assert_eq!(json["config"]["url"], "https://example.com/hook");
    }

    #[test]
    fn test_plugin_system_wide_serialization() {
        // NOTE: System-wide plugins have null user_id
        let plugin = Plugin {
            id: Uuid::new_v4(),
            user_id: None,
            name: "System Filter".to_string(),
            description: None,
            plugin_type: "filter".to_string(),
            config: serde_json::json!({"rules": []}),
            hooks: vec!["on_receive".to_string()],
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&plugin).unwrap();
        assert!(json["user_id"].is_null());
        assert_eq!(json["name"], "System Filter");
    }

    #[test]
    fn test_create_plugin_request_deserialization() {
        let json = serde_json::json!({
            "name": "Slack Notifier",
            "description": "Posts to Slack on new email",
            "plugin_type": "webhook",
            "config": {"url": "https://hooks.slack.com/...", "channel": "#email"},
            "hooks": ["on_receive", "on_send"],
            "enabled": true
        });

        let request: CreatePluginRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Slack Notifier");
        assert_eq!(request.plugin_type, PluginType::Webhook);
        assert_eq!(request.hooks.len(), 2);
        assert_eq!(request.hooks[0], PluginHook::OnReceive);
        assert_eq!(request.enabled, Some(true));
    }

    #[test]
    fn test_create_plugin_request_minimal() {
        let json = serde_json::json!({
            "name": "Basic Filter",
            "plugin_type": "filter",
            "hooks": ["on_receive"]
        });

        let request: CreatePluginRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Basic Filter");
        assert!(request.description.is_none());
        assert!(request.config.is_none());
        assert!(request.enabled.is_none());
    }

    #[test]
    fn test_create_plugin_request_missing_required_fails() {
        let json = serde_json::json!({
            "name": "Incomplete"
        });
        let result = serde_json::from_value::<CreatePluginRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_plugin_request_partial() {
        let json = serde_json::json!({
            "enabled": false
        });

        let update: UpdatePluginRequest = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert!(update.description.is_none());
        assert!(update.plugin_type.is_none());
        assert!(update.config.is_none());
        assert!(update.hooks.is_none());
        assert_eq!(update.enabled, Some(false));
    }

    #[test]
    fn test_update_plugin_request_empty() {
        let json = serde_json::json!({});
        let update: UpdatePluginRequest = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert!(update.enabled.is_none());
    }

    #[test]
    fn test_plugin_execution_serialization() {
        let execution = PluginExecution {
            id: Uuid::new_v4(),
            plugin_id: Uuid::new_v4(),
            event: "on_receive".to_string(),
            status: "success".to_string(),
            duration_ms: Some(42),
            error_message: None,
            executed_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&execution).unwrap();
        assert_eq!(json["event"], "on_receive");
        assert_eq!(json["status"], "success");
        assert_eq!(json["duration_ms"], 42);
        assert!(json["error_message"].is_null());
    }

    #[test]
    fn test_plugin_execution_error_serialization() {
        let execution = PluginExecution {
            id: Uuid::new_v4(),
            plugin_id: Uuid::new_v4(),
            event: "on_send".to_string(),
            status: "error".to_string(),
            duration_ms: Some(5000),
            error_message: Some("Connection refused".to_string()),
            executed_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&execution).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_message"], "Connection refused");
        assert_eq!(json["duration_ms"], 5000);
    }

    #[test]
    fn test_plugin_execution_timeout_serialization() {
        let execution = PluginExecution {
            id: Uuid::new_v4(),
            plugin_id: Uuid::new_v4(),
            event: "on_delete".to_string(),
            status: "timeout".to_string(),
            duration_ms: Some(10000),
            error_message: Some("Plugin execution timed out after 10s".to_string()),
            executed_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&execution).unwrap();
        assert_eq!(json["status"], "timeout");
    }

    #[test]
    fn test_plugin_hook_invalid_deserialization() {
        let result = serde_json::from_str::<PluginHook>("\"on_unknown\"");
        assert!(result.is_err());
    }
}
