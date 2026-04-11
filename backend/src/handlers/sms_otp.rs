use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::sms_service;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct EnrollSmsRequest {
    pub phone_number: String,
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VerifySmsOtpRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct SmsOtpStatus {
    pub enabled: bool,
    pub phone_number: Option<String>,
    pub provider: Option<String>,
}

fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}

/// POST /api/sms-otp/enroll — Start SMS OTP enrollment (sends code to phone)
pub async fn enroll(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<EnrollSmsRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Validate phone number format (Ghana: +233...)
    if !body.phone_number.starts_with('+') || body.phone_number.len() < 10 {
        return Err(AppError::BadRequest("Invalid phone number format. Use international format (+233...)".to_string()));
    }

    let provider = body.provider.as_deref().unwrap_or("hubtel");
    if provider != "hubtel" && provider != "africastalking" {
        return Err(AppError::BadRequest("Provider must be 'hubtel' or 'africastalking'".to_string()));
    }

    // Store phone number
    sqlx::query("UPDATE mailboxes SET phone_number = $1, sms_provider = $2 WHERE id = $3")
        .bind(&body.phone_number)
        .bind(provider)
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    // Generate and store OTP
    let code = sms_service::generate_otp();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);

    // Invalidate old codes
    sqlx::query("UPDATE sms_otp_codes SET used = true WHERE mailbox_id = $1 AND used = false")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    sqlx::query(
        "INSERT INTO sms_otp_codes (mailbox_id, code, phone_number, expires_at) VALUES ($1, $2, $3, $4)"
    )
    .bind(mailbox_id)
    .bind(&code)
    .bind(&body.phone_number)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    // Send OTP via SMS
    let sms_config = sms_service::SmsConfig::default();
    sms_service::send_otp(&sms_config, provider, &body.phone_number, &code)
        .await
        .map_err(|e| AppError::Internal(e))?;

    Ok(StatusCode::OK)
}

/// POST /api/sms-otp/verify — Verify SMS OTP to complete enrollment
pub async fn verify(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<VerifySmsOtpRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Find valid (unused, not expired) OTP code
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM sms_otp_codes WHERE mailbox_id = $1 AND code = $2 AND used = false AND expires_at > NOW()"
    )
    .bind(mailbox_id)
    .bind(&body.code)
    .fetch_optional(&state.db)
    .await?;

    let otp_id = row
        .map(|r| r.0)
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired OTP code".to_string()))?;

    // Mark code as used
    sqlx::query("UPDATE sms_otp_codes SET used = true WHERE id = $1")
        .bind(otp_id)
        .execute(&state.db)
        .await?;

    // Enable SMS OTP
    sqlx::query("UPDATE mailboxes SET sms_otp_enabled = true WHERE id = $1")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::OK)
}

/// DELETE /api/sms-otp — Disable SMS OTP
pub async fn disable(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    sqlx::query("UPDATE mailboxes SET sms_otp_enabled = false WHERE id = $1")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/sms-otp/status — Get SMS OTP status
pub async fn status(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<SmsOtpStatus>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let row: Option<(bool, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT sms_otp_enabled, phone_number, sms_provider FROM mailboxes WHERE id = $1"
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?;

    let (enabled, phone, provider) = row
        .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))?;

    Ok(Json(SmsOtpStatus {
        enabled,
        phone_number: phone.map(|p| mask_phone(&p)),
        provider,
    }))
}

// Added: Phone masking helper extracted for testability
fn mask_phone(p: &str) -> String {
    if p.len() > 6 {
        format!("{}***{}", &p[..4], &p[p.len()-3..])
    } else {
        "***".to_string()
    }
}

/// POST /api/sms-otp/resend — Resend OTP code
pub async fn resend(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Get phone number and provider
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT phone_number, sms_provider FROM mailboxes WHERE id = $1"
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?;

    let (phone, provider) = row
        .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))?;

    let phone = phone.ok_or_else(|| AppError::BadRequest("No phone number registered".to_string()))?;
    let provider = provider.unwrap_or_else(|| "hubtel".to_string());

    // Generate new code
    let code = sms_service::generate_otp();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);

    // Invalidate old codes
    sqlx::query("UPDATE sms_otp_codes SET used = true WHERE mailbox_id = $1 AND used = false")
        .bind(mailbox_id)
        .execute(&state.db)
        .await?;

    sqlx::query(
        "INSERT INTO sms_otp_codes (mailbox_id, code, phone_number, expires_at) VALUES ($1, $2, $3, $4)"
    )
    .bind(mailbox_id)
    .bind(&code)
    .bind(&phone)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    let sms_config = sms_service::SmsConfig::default();
    sms_service::send_otp(&sms_config, &provider, &phone, &code)
        .await
        .map_err(|e| AppError::Internal(e))?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_phone_normal() {
        assert_eq!(mask_phone("+233241234789"), "+233***789");
    }

    #[test]
    fn test_mask_phone_short() {
        assert_eq!(mask_phone("+2332"), "***");
    }

    #[test]
    fn test_mask_phone_exactly_7_chars() {
        assert_eq!(mask_phone("+233241"), "+233***241");
    }

    #[test]
    fn test_enroll_request_deserialization() {
        let json = r#"{"phone_number": "+233241234567", "provider": "hubtel"}"#;
        let req: EnrollSmsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.phone_number, "+233241234567");
        assert_eq!(req.provider, Some("hubtel".to_string()));
    }

    #[test]
    fn test_enroll_request_without_provider() {
        let json = r#"{"phone_number": "+233241234567"}"#;
        let req: EnrollSmsRequest = serde_json::from_str(json).unwrap();
        assert!(req.provider.is_none());
    }

    #[test]
    fn test_verify_request_deserialization() {
        let json = r#"{"code": "123456"}"#;
        let req: VerifySmsOtpRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.code, "123456");
    }

    #[test]
    fn test_sms_otp_status_serialization() {
        let status = SmsOtpStatus {
            enabled: true,
            phone_number: Some("+233***789".to_string()),
            provider: Some("hubtel".to_string()),
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["phone_number"], "+233***789");
        assert_eq!(json["provider"], "hubtel");
    }

    #[test]
    fn test_sms_otp_status_disabled() {
        let status = SmsOtpStatus {
            enabled: false,
            phone_number: None,
            provider: None,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["enabled"], false);
        assert!(json["phone_number"].is_null());
    }
}
