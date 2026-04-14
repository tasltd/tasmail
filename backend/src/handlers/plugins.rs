// Added: Plugin management handlers for TMAIL-132
// PURPOSE: CRUD endpoints for plugins plus execution log and test-fire

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::plugin::{
    CreatePluginRequest, Plugin, PluginExecution, UpdatePluginRequest,
};
use crate::services::auth_service::Claims;
use crate::services::plugin_executor::{build_test_context, execute_hook, PluginResult};
use crate::state::AppState;

/// PURPOSE: List all plugins for the authenticated user (plus system-wide)
/// GET /api/plugins
pub async fn list_plugins(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Plugin>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let plugins = Plugin::list_by_user(&state.db, user_id).await?;
    Ok(Json(plugins))
}

/// PURPOSE: Create a new plugin
/// POST /api/plugins
/// CONSTRAINTS: Name must be provided, hooks array must not be empty
pub async fn create_plugin(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreatePluginRequest>,
) -> Result<(StatusCode, Json<Plugin>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate that hooks list is not empty
    if body.hooks.is_empty() {
        return Err(AppError::BadRequest(
            "At least one hook must be specified".to_string(),
        ));
    }

    // Added: Validate webhook plugins have a URL in config
    if body.plugin_type == crate::models::plugin::PluginType::Webhook {
        if let Some(ref config) = body.config {
            if config.get("url").and_then(|v| v.as_str()).is_none() {
                return Err(AppError::BadRequest(
                    "Webhook plugins require a 'url' field in config".to_string(),
                ));
            }
        } else {
            return Err(AppError::BadRequest(
                "Webhook plugins require a config with 'url' field".to_string(),
            ));
        }
    }

    let plugin = Plugin::create(&state.db, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(plugin)))
}

/// PURPOSE: Get a single plugin by ID
/// GET /api/plugins/:id
pub async fn get_plugin(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Plugin>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let plugin = Plugin::get_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plugin not found".to_string()))?;
    Ok(Json(plugin))
}

/// PURPOSE: Update an existing plugin's configuration
/// PUT /api/plugins/:id
pub async fn update_plugin(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdatePluginRequest>,
) -> Result<Json<Plugin>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate hooks list if provided
    if let Some(ref hooks) = body.hooks {
        if hooks.is_empty() {
            return Err(AppError::BadRequest(
                "At least one hook must be specified".to_string(),
            ));
        }
    }

    let plugin = Plugin::update(&state.db, id, user_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Plugin not found".to_string()))?;
    Ok(Json(plugin))
}

/// PURPOSE: Delete a plugin and all associated execution records
/// DELETE /api/plugins/:id
pub async fn delete_plugin(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = Plugin::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Plugin not found".to_string()))
    }
}

/// PURPOSE: List recent execution log entries for a plugin
/// GET /api/plugins/:id/executions
pub async fn list_executions(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Vec<PluginExecution>>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Verify the plugin belongs to the user before listing executions
    Plugin::get_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plugin not found".to_string()))?;

    let executions = PluginExecution::list_by_plugin(&state.db, id).await?;
    Ok(Json(executions))
}

/// PURPOSE: Test-fire a plugin with a dummy context
/// POST /api/plugins/:id/test
#[derive(serde::Serialize)]
pub struct TestPluginResponse {
    pub success: bool,
    pub duration_ms: i32,
    pub error: Option<String>,
}

// Added: Convert PluginResult to API response
impl From<PluginResult> for TestPluginResponse {
    fn from(r: PluginResult) -> Self {
        TestPluginResponse {
            success: r.success,
            duration_ms: r.duration_ms,
            error: r.error,
        }
    }
}

pub async fn test_plugin(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<TestPluginResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;

    let plugin = Plugin::get_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plugin not found".to_string()))?;

    // Added: Use the first configured hook, or default to on_receive
    let hook_str = plugin
        .hooks
        .first()
        .cloned()
        .unwrap_or_else(|| "on_receive".to_string());

    let hook = crate::models::plugin::PluginHook::from_str(&hook_str)
        .unwrap_or(crate::models::plugin::PluginHook::OnReceive);

    let context = build_test_context(&hook_str);
    let results = execute_hook(&state.db, user_id, hook, &context).await;

    // Added: Find the result for this specific plugin
    let result = results
        .into_iter()
        .find(|r| r.plugin_id == id)
        .unwrap_or(PluginResult {
            plugin_id: id,
            plugin_name: plugin.name,
            success: false,
            duration_ms: 0,
            error: Some("Plugin was not executed".to_string()),
        });

    Ok(Json(result.into()))
}

fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }

    #[test]
    fn test_test_plugin_response_from_success() {
        let result = PluginResult {
            plugin_id: uuid::Uuid::new_v4(),
            plugin_name: "Test".to_string(),
            success: true,
            duration_ms: 100,
            error: None,
        };
        let response: TestPluginResponse = result.into();
        assert!(response.success);
        assert_eq!(response.duration_ms, 100);
        assert!(response.error.is_none());
    }

    #[test]
    fn test_test_plugin_response_from_failure() {
        let result = PluginResult {
            plugin_id: uuid::Uuid::new_v4(),
            plugin_name: "Broken".to_string(),
            success: false,
            duration_ms: 5000,
            error: Some("HTTP 500".to_string()),
        };
        let response: TestPluginResponse = result.into();
        assert!(!response.success);
        assert_eq!(response.error.unwrap(), "HTTP 500");
    }
}
