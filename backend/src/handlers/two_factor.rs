use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::totp_service;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct EnrollResponse {
    pub secret: String,
    pub otpauth_url: String,
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct TwoFactorStatus {
    pub enabled: bool,
    pub verified_at: Option<String>,
    pub backup_codes_remaining: i64,
}

/// POST /api/2fa/enroll — Start TOTP enrollment (generates secret + backup codes)
pub async fn enroll(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<(StatusCode, Json<EnrollResponse>), AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    // Check if already enrolled
    let row: Option<(Option<bool>,)> = sqlx::query_as(
        "SELECT totp_enabled FROM mailboxes WHERE id = $1"
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?;

    if let Some((Some(true),)) = row {
        return Err(AppError::Conflict("2FA already enabled. Disable first to re-enroll.".to_string()));
    }

    // Generate TOTP secret
    let (secret, otpauth_url) = totp_service::generate_totp(&claims.username, "TASMail")?;

    // Store secret (not yet verified)
    sqlx::query("UPDATE mailboxes SET totp_secret = $1, totp_enabled = false WHERE id = $2")
        .bind(&secret)
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    // Generate and store backup codes
    let backup_codes = totp_service::generate_backup_codes();
    // Delete old codes first
    sqlx::query("DELETE FROM backup_codes WHERE mailbox_id = $1")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    for code in &backup_codes {
        let hash = totp_service::hash_backup_code(code);
        sqlx::query("INSERT INTO backup_codes (mailbox_id, code_hash) VALUES ($1, $2)")
            .bind(mailbox_id)
            .bind(&hash)
            .execute(&state.db)
            .await?;
    }

    Ok((
        StatusCode::OK,
        Json(EnrollResponse {
            secret,
            otpauth_url,
            backup_codes,
        }),
    ))
}

/// POST /api/2fa/verify — Verify TOTP code to complete enrollment
pub async fn verify(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<VerifyRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    // Get stored secret
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT totp_secret FROM mailboxes WHERE id = $1"
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?;

    let secret = row
        .and_then(|r| r.0)
        .ok_or_else(|| AppError::BadRequest("No TOTP enrollment in progress".to_string()))?;

    // Verify the code
    if !totp_service::verify_totp(&secret, &body.code)? {
        return Err(AppError::Unauthorized("Invalid TOTP code".to_string()));
    }

    // Enable 2FA
    sqlx::query(
        "UPDATE mailboxes SET totp_enabled = true, totp_verified_at = NOW() WHERE id = $1"
    )
    .bind(mailbox_id)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::OK)
}

/// DELETE /api/2fa — Disable 2FA
pub async fn disable(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<VerifyRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    // Require current TOTP code to disable
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT totp_secret FROM mailboxes WHERE id = $1 AND totp_enabled = true"
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?;

    let secret = row
        .and_then(|r| r.0)
        .ok_or_else(|| AppError::BadRequest("2FA not enabled".to_string()))?;

    if !totp_service::verify_totp(&secret, &body.code)? {
        return Err(AppError::Unauthorized("Invalid TOTP code".to_string()));
    }

    // Disable 2FA and clear secret
    sqlx::query(
        "UPDATE mailboxes SET totp_enabled = false, totp_secret = NULL, totp_verified_at = NULL WHERE id = $1"
    )
    .bind(mailbox_id)
    .execute(&state.db)
    .await?;

    // Delete backup codes
    sqlx::query("DELETE FROM backup_codes WHERE mailbox_id = $1")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/2fa/status — Check 2FA status
pub async fn status(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<TwoFactorStatus>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let (enabled, verified_at): (bool, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT totp_enabled, totp_verified_at FROM mailboxes WHERE id = $1"
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))?;

    let remaining: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM backup_codes WHERE mailbox_id = $1 AND used = false"
    )
    .bind(mailbox_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(TwoFactorStatus {
        enabled,
        verified_at: verified_at.map(|v| v.to_rfc3339()),
        backup_codes_remaining: remaining.0,
    }))
}
