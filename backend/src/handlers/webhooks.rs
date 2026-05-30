// Added: Webhook management handlers for TMAIL-131
// Added (TMAIL-313): redeliver + rotate_secret handlers for webhook lifecycle ops

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::models::webhook::{
    CreateWebhookRequest, UpdateWebhookRequest, Webhook, WebhookDelivery,
};
use crate::services::auth_service::Claims;
use crate::services::webhook_dispatcher;
use crate::state::AppState;

/// Added (TMAIL-313): One-time response shape for POST /api/webhooks/:id/rotate-secret.
/// The plaintext secret is returned exactly once at rotation time — subsequent
/// reads of the webhook do NOT expose the new secret (it stays in the DB but the
/// API consumer is expected to capture it now).
#[derive(Debug, Serialize)]
pub struct RotateSecretResponse {
    pub id: uuid::Uuid,
    pub secret: String,
    pub rotated_at: chrono::DateTime<chrono::Utc>,
}

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

/// PURPOSE (TMAIL-313): Manually replay a previous delivery for a webhook
/// POST /api/webhooks/:id/deliveries/:delivery_id/redeliver
/// CONSTRAINTS: Both the webhook and the original delivery must belong to the
///              authenticated user. The replay re-uses the original payload but
///              recomputes the signature with the webhook's *current* secret —
///              important after a rotate-secret call.
/// RETURNS: 201 Created with the new WebhookDelivery row (success may be false
///          if the receiver was unavailable; the row still appears in the
///          deliveries log either way).
pub async fn redeliver(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((id, delivery_id)): Path<(uuid::Uuid, uuid::Uuid)>,
) -> Result<(StatusCode, Json<WebhookDelivery>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Verify webhook ownership before touching the delivery — otherwise an
    // attacker who knew a delivery_id could probe for cross-tenant webhooks via
    // 404 vs 200 timing differences. find_by_id is user-scoped.
    let webhook = Webhook::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Webhook not found".to_string()))?;

    let delivery = WebhookDelivery::find_by_id_and_webhook(&state.db, delivery_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Delivery not found".to_string()))?;

    let new_delivery =
        webhook_dispatcher::redeliver_webhook(&state.db, &webhook, &delivery).await?;

    // NOTE: Audit log is fire-and-forget — a failed insert MUST NOT mask the
    // already-successful redelivery. AuditLog::record itself logs at warn! on
    // failure (see TMAIL-307 / TMAIL-198).
    let _ = AuditLog::record(
        &state.db,
        Some(user_id),
        "webhook.redeliver",
        Some("webhook"),
        Some(&id.to_string()),
        Some(serde_json::json!({
            "original_delivery_id": delivery_id.to_string(),
            "new_delivery_id": new_delivery.id.to_string(),
            "success": new_delivery.success,
            "response_status": new_delivery.response_status,
        })),
        None,
        None,
    )
    .await;

    Ok((StatusCode::CREATED, Json(new_delivery)))
}

/// PURPOSE (TMAIL-313): Generate a new HMAC signing secret for a webhook
/// POST /api/webhooks/:id/rotate-secret
/// CONSTRAINTS: New secret is 32 bytes of cryptographic randomness, hex-encoded
///              (64 chars). It is returned ONCE in the response body — subsequent
///              GETs do not expose it. Existing in-flight deliveries continue to
///              use the *old* secret until the dispatcher's next read of the
///              webhook row (the dispatcher always reads fresh on each event).
/// RETURNS: 200 OK with { id, secret, rotated_at }.
pub async fn rotate_secret(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<RotateSecretResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;

    let new_secret = generate_webhook_secret();
    let webhook = Webhook::rotate_secret(&state.db, id, user_id, &new_secret)
        .await?
        .ok_or_else(|| AppError::NotFound("Webhook not found".to_string()))?;

    // NOTE: Never log the secret value itself — only that a rotation happened.
    let _ = AuditLog::record(
        &state.db,
        Some(user_id),
        "webhook.rotate_secret",
        Some("webhook"),
        Some(&id.to_string()),
        Some(serde_json::json!({
            "rotated_at": webhook.updated_at.to_rfc3339(),
        })),
        None,
        None,
    )
    .await;

    Ok(Json(RotateSecretResponse {
        id: webhook.id,
        secret: new_secret,
        rotated_at: webhook.updated_at,
    }))
}

/// PURPOSE: Generate a cryptographically-random HMAC signing secret
/// CONSTRAINTS: 32 bytes of entropy (256 bits), hex-encoded → 64 ASCII chars.
///              Matches the size convention used elsewhere (WebAuthn challenge).
fn generate_webhook_secret() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);
    hex::encode(bytes)
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

    // Added (TMAIL-313): secret generator contract — 32 bytes, hex-encoded
    #[test]
    fn test_generate_webhook_secret_length_and_charset() {
        let s = generate_webhook_secret();
        assert_eq!(s.len(), 64, "expected 32 bytes hex-encoded = 64 chars");
        assert!(
            s.chars().all(|c| c.is_ascii_hexdigit()),
            "secret must be hex-only, got {}",
            s
        );
    }

    #[test]
    fn test_generate_webhook_secret_is_random() {
        // NOTE: Two consecutive calls must not collide. Probability of a real
        // collision in 256 bits is negligible — a hit here means the RNG broke.
        let a = generate_webhook_secret();
        let b = generate_webhook_secret();
        assert_ne!(a, b);
    }

    #[test]
    fn test_rotate_secret_response_serializes_to_expected_shape() {
        let resp = RotateSecretResponse {
            id: uuid::Uuid::nil(),
            secret: "abc123".to_string(),
            rotated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("id").is_some());
        assert_eq!(json["secret"], "abc123");
        assert!(json.get("rotated_at").is_some());
    }
}
