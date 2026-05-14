// Added: Admin CRUD for payment_provider_config (DB-backed credentials, mirrors PayPro pattern).
// All endpoints require ROOT/admin auth — credentials are written encrypted at rest.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::payment_provider_config::{PaymentProviderConfig, PlaintextProviderConfig};
use crate::services::auth_service::{self, Claims};
use crate::state::AppState;

/// PURPOSE: Public-facing summary — masks all sensitive ciphertext fields.
#[derive(Debug, Serialize)]
pub struct ProviderSummary {
    pub id: Uuid,
    pub provider: String,
    pub tenant_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub base_url: Option<String>,
    pub callback_url: Option<String>,
    pub currency: Option<String>,
    pub environment: Option<String>,
    pub enabled: bool,
    pub archived: bool,
    pub has_secret_key: bool,
    pub has_public_key: bool,
    pub has_webhook_secret: bool,
    pub has_merchant_id: bool,
    pub has_api_password: bool,
    pub has_key_id: bool,
    pub has_shared_secret_key: bool,
    pub bank_details: Option<serde_json::Value>,
    pub split_code: Option<String>,
}

impl From<PaymentProviderConfig> for ProviderSummary {
    fn from(c: PaymentProviderConfig) -> Self {
        Self {
            id: c.id,
            provider: c.provider,
            tenant_id: c.tenant_id,
            name: c.name,
            description: c.description,
            base_url: c.base_url,
            callback_url: c.callback_url,
            currency: c.currency,
            environment: c.environment,
            enabled: c.enabled,
            archived: c.archived,
            has_secret_key: c.secret_key.as_deref().is_some_and(|s| !s.is_empty()),
            has_public_key: c.public_key.as_deref().is_some_and(|s| !s.is_empty()),
            has_webhook_secret: c.webhook_secret.as_deref().is_some_and(|s| !s.is_empty()),
            has_merchant_id: c.merchant_id.as_deref().is_some_and(|s| !s.is_empty()),
            has_api_password: c.api_password.as_deref().is_some_and(|s| !s.is_empty()),
            has_key_id: c.key_id.as_deref().is_some_and(|s| !s.is_empty()),
            has_shared_secret_key: c.shared_secret_key.as_deref().is_some_and(|s| !s.is_empty()),
            bank_details: c.bank_details,
            split_code: c.split_code,
        }
    }
}

/// PURPOSE: GET /api/admin/payment-providers — list all configs with sensitive fields masked
pub async fn list_providers(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ProviderSummary>>, AppError> {
    // Fix: TMAIL-210 — admin-only.
    auth_service::require_admin(&claims)?;
    let rows = PaymentProviderConfig::list_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(ProviderSummary::from).collect()))
}

/// PURPOSE: Plaintext credential body for create/update.
#[derive(Debug, Deserialize)]
pub struct UpsertProviderRequest {
    pub provider: String, // PAYSTACK | MASTERCARD | CYBERSOURCE | BANK_TRANSFER
    pub tenant_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,

    pub secret_key: Option<String>,
    pub public_key: Option<String>,
    pub webhook_secret: Option<String>,
    pub merchant_id: Option<String>,
    pub api_password: Option<String>,
    pub key_id: Option<String>,
    pub shared_secret_key: Option<String>,
    pub key_file_path: Option<String>,

    pub base_url: Option<String>,
    pub callback_url: Option<String>,
    pub currency: Option<String>,
    pub environment: Option<String>,
    pub bank_details: Option<serde_json::Value>,
    pub split_code: Option<String>,
    pub notes: Option<String>,
}

/// PURPOSE: POST /api/admin/payment-providers — create a new config row.
/// Sensitive fields are encrypted before storage; the response is masked.
pub async fn create_provider(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<UpsertProviderRequest>,
) -> Result<(StatusCode, Json<ProviderSummary>), AppError> {
    // Fix: TMAIL-210 — admin-only.
    auth_service::require_admin(&claims)?;
    let allowed = ["PAYSTACK", "MASTERCARD", "CYBERSOURCE", "BANK_TRANSFER"];
    if !allowed.contains(&body.provider.as_str()) {
        return Err(AppError::BadRequest(format!(
            "provider must be one of: {}",
            allowed.join(", ")
        )));
    }

    let plaintext = PlaintextProviderConfig {
        description: body.description.as_deref(),
        secret_key: body.secret_key.as_deref(),
        public_key: body.public_key.as_deref(),
        webhook_secret: body.webhook_secret.as_deref(),
        merchant_id: body.merchant_id.as_deref(),
        api_password: body.api_password.as_deref(),
        key_id: body.key_id.as_deref(),
        shared_secret_key: body.shared_secret_key.as_deref(),
        key_file_path: body.key_file_path.as_deref(),
        base_url: body.base_url.as_deref(),
        callback_url: body.callback_url.as_deref(),
        currency: body.currency.as_deref(),
        environment: body.environment.as_deref(),
        bank_details: body.bank_details,
        split_code: body.split_code.as_deref(),
        notes: body.notes.as_deref(),
    };

    let row = PaymentProviderConfig::insert(
        &state.db,
        &state.encryption,
        &body.provider,
        body.tenant_id,
        body.name.as_deref(),
        plaintext,
    )
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create provider config: {}", e)))?;

    Ok((StatusCode::CREATED, Json(ProviderSummary::from(row))))
}

/// PURPOSE: DELETE /api/admin/payment-providers/{id} — soft-delete (archived = true)
pub async fn archive_provider(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    // Fix: TMAIL-210 — admin-only.
    auth_service::require_admin(&claims)?;
    let result = sqlx::query("UPDATE payment_provider_config SET archived = true, enabled = false WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("payment_provider_config {}", id)));
    }
    Ok(StatusCode::NO_CONTENT)
}
