// Added: Webhook management handlers for TMAIL-131

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::webhook::{
    CreateWebhookRequest, UpdateWebhookRequest, Webhook, WebhookDelivery,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: List all webhooks for the authenticated user
/// GET /api/webhooks
pub async fn list_webhooks(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Webhook>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let webhooks = Webhook::find_by_user(&state.db, user_id).await?;
    Ok(Json(webhooks))
}

/// PURPOSE: Create a new webhook endpoint
/// POST /api/webhooks
/// CONSTRAINTS: URL must be provided, events array must not be empty
pub async fn create_webhook(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<Webhook>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate that events list is not empty
    if body.events.is_empty() {
        return Err(AppError::BadRequest(
            "At least one event type must be specified".to_string(),
        ));
    }

    // Added: Validate URL format
    if !body.url.starts_with("https://") && !body.url.starts_with("http://") {
        return Err(AppError::BadRequest(
            "Webhook URL must start with http:// or https://".to_string(),
        ));
    }

    let webhook = Webhook::create(&state.db, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(webhook)))
}

/// PURPOSE: Get a single webhook by ID
/// GET /api/webhooks/:id
pub async fn get_webhook(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Webhook>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let webhook = Webhook::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Webhook not found".to_string()))?;
    Ok(Json(webhook))
}

/// PURPOSE: Update an existing webhook's configuration
/// PUT /api/webhooks/:id
pub async fn update_webhook(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateWebhookRequest>,
) -> Result<Json<Webhook>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate events list if provided
    if let Some(ref events) = body.events {
        if events.is_empty() {
            return Err(AppError::BadRequest(
                "At least one event type must be specified".to_string(),
            ));
        }
    }

    // Added: Validate URL if provided
    if let Some(ref url) = body.url {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(AppError::BadRequest(
                "Webhook URL must start with http:// or https://".to_string(),
            ));
        }
    }

    let webhook = Webhook::update(&state.db, id, user_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Webhook not found".to_string()))?;
    Ok(Json(webhook))
}

/// PURPOSE: Delete a webhook and all associated delivery records
/// DELETE /api/webhooks/:id
pub async fn delete_webhook(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = Webhook::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Webhook not found".to_string()))
    }
}

/// PURPOSE: List recent delivery attempts for a webhook
/// GET /api/webhooks/:id/deliveries
pub async fn list_deliveries(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Vec<WebhookDelivery>>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Verify the webhook belongs to the user before listing deliveries
    Webhook::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Webhook not found".to_string()))?;

    let deliveries = WebhookDelivery::find_by_webhook(&state.db, id).await?;
    Ok(Json(deliveries))
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
}
