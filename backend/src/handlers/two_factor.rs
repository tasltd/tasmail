use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::db_session;
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
    // Fix: TMAIL-282 — backup_codes has FORCE RLS and the policy's `WITH CHECK`
    // clause rejects every INSERT unless `app.mailbox_id` is set on the
    // connection running the statement. Same shape as TMAIL-209's sms_otp_codes
    // fix and TMAIL-198's audit-log fix. Without the pinned connection the
    // INSERT raised "new row violates row-level security policy" and the whole
    // enroll endpoint returned 500.
    {
        let mut conn = db_session::acquire_with_rls(&state, &claims).await?;
        sqlx::query("DELETE FROM backup_codes WHERE mailbox_id = $1")
            .bind(mailbox_id)
            .execute(&mut *conn)
            .await?;

        for code in &backup_codes {
            let hash = totp_service::hash_backup_code(code);
            sqlx::query("INSERT INTO backup_codes (mailbox_id, code_hash) VALUES ($1, $2)")
                .bind(mailbox_id)
                .bind(&hash)
                .execute(&mut *conn)
                .await?;
        }
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

    // Fix: TMAIL-282 — backup_codes is RLS-protected; the DELETE policy's USING
    // clause also evaluates against `app.mailbox_id`. Without the pinned
    // connection no rows match and the codes leak past disable.
    {
        let mut conn = db_session::acquire_with_rls(&state, &claims).await?;
        sqlx::query("DELETE FROM backup_codes WHERE mailbox_id = $1")
            .bind(mailbox_id)
            .execute(&mut *conn)
            .await?;
    }

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

    // Fix: TMAIL-282 — RLS hides backup_codes rows from any connection that
    // doesn't have `app.mailbox_id` set, so the status endpoint was always
    // reporting `backup_codes_remaining: 0` even after a successful enroll.
    let mut conn = db_session::acquire_with_rls(&state, &claims).await?;
    let remaining: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM backup_codes WHERE mailbox_id = $1 AND used = false"
    )
    .bind(mailbox_id)
    .fetch_one(&mut *conn)
    .await?;

    Ok(Json(TwoFactorStatus {
        enabled,
        verified_at: verified_at.map(|v| v.to_rfc3339()),
        backup_codes_remaining: remaining.0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_request_deserialization() {
        let json = r#"{"code": "123456"}"#;
        let req: VerifyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.code, "123456");
    }

    #[test]
    fn test_verify_request_rejects_missing_code() {
        let json = r#"{}"#;
        assert!(serde_json::from_str::<VerifyRequest>(json).is_err());
    }

    #[test]
    fn test_enroll_response_serialization() {
        let resp = EnrollResponse {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            otpauth_url: "otpauth://totp/TASMail:user@example.com?secret=JBSWY3DPEHPK3PXP".to_string(),
            backup_codes: vec!["abc123".to_string(), "def456".to_string()],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["secret"], "JBSWY3DPEHPK3PXP");
        assert!(json["otpauth_url"].as_str().unwrap().starts_with("otpauth://"));
        assert_eq!(json["backup_codes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_two_factor_status_serialization() {
        let status = TwoFactorStatus {
            enabled: true,
            verified_at: Some("2026-01-15T10:30:00+00:00".to_string()),
            backup_codes_remaining: 8,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["backup_codes_remaining"], 8);
        assert!(json["verified_at"].is_string());
    }

    #[test]
    fn test_two_factor_status_disabled() {
        let status = TwoFactorStatus {
            enabled: false,
            verified_at: None,
            backup_codes_remaining: 0,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["enabled"], false);
        assert!(json["verified_at"].is_null());
        assert_eq!(json["backup_codes_remaining"], 0);
    }
}
