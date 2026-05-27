// Added: Chat integration management handlers for TMAIL-129
// PURPOSE: CRUD endpoints for managing team chat webhook integrations
// EXTERNAL: Uses chat_notifier service for test message delivery

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::chat_integration::{
    ChatIntegration, CreateChatIntegrationRequest, UpdateChatIntegrationRequest,
};
use crate::services::auth_service::Claims;
use crate::services::chat_notifier;
use crate::state::AppState;

/// PURPOSE: List all chat integrations for the authenticated user
/// GET /api/chat-integrations
pub async fn list_chat_integrations(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ChatIntegration>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let integrations = ChatIntegration::find_by_user(&state.db, user_id).await?;
    Ok(Json(integrations))
}

/// PURPOSE: Create a new chat integration
/// POST /api/chat-integrations
/// CONSTRAINTS: webhook_url must be a valid HTTP(S) URL
pub async fn create_chat_integration(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateChatIntegrationRequest>,
) -> Result<(StatusCode, Json<ChatIntegration>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate URL format
    if !body.webhook_url.starts_with("https://") && !body.webhook_url.starts_with("http://") {
        return Err(AppError::BadRequest(
            "Webhook URL must start with http:// or https://".to_string(),
        ));
    }

    let integration = ChatIntegration::create(&state.db, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(integration)))
}

/// PURPOSE: Get a single chat integration by ID
/// GET /api/chat-integrations/:id
pub async fn get_chat_integration(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ChatIntegration>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let integration = ChatIntegration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Chat integration not found".to_string()))?;
    Ok(Json(integration))
}

/// PURPOSE: Update an existing chat integration's configuration
/// PUT /api/chat-integrations/:id
pub async fn update_chat_integration(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateChatIntegrationRequest>,
) -> Result<Json<ChatIntegration>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate URL if provided
    if let Some(ref url) = body.webhook_url {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(AppError::BadRequest(
                "Webhook URL must start with http:// or https://".to_string(),
            ));
        }
    }

    let integration = ChatIntegration::update(&state.db, id, user_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Chat integration not found".to_string()))?;
    Ok(Json(integration))
}

/// PURPOSE: Delete a chat integration
/// DELETE /api/chat-integrations/:id
pub async fn delete_chat_integration(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = ChatIntegration::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Chat integration not found".to_string()))
    }
}

/// PURPOSE: Send a test notification to the chat integration's webhook
/// POST /api/chat-integrations/:id/test
/// NOTE: Uses the chat_notifier service to format and send a test message
pub async fn test_chat_integration(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let integration = ChatIntegration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Chat integration not found".to_string()))?;

    // Added: Send test notification via the chat notifier service
    let result = chat_notifier::send_test_notification(
        &integration.platform,
        &integration.webhook_url,
    )
    .await;

    match result {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "message": "Test notification sent successfully"
        }))),
        Err(error_message) => Ok(Json(serde_json::json!({
            "success": false,
            "message": error_message
        }))),
    }
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
            is_compliance_officer: false,
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
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }
}
