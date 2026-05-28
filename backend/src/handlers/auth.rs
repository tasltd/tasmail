use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::services::auth_service;
use crate::state::AppState;
// Added: Input validation for login requests (TMAIL-37)
use crate::validation;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<(StatusCode, Json<auth_service::TokenPair>), AppError> {
    // Added: Validate input lengths to prevent abuse (TMAIL-37)
    validation::validate_username(&body.username)?;

    // Added (TMAIL-273): Thread the lockout policy through so authenticate()
    // can enforce per-account brute-force limits in addition to the per-IP
    // rate limiter that runs upstream of this handler.
    let tokens = auth_service::authenticate(
        &state.db,
        &state.config.jwt,
        &state.config.lockout,
        &body.username,
        &body.password,
        None,
        None,
    )
    .await?;

    // Added: Audit log for successful login (fire-and-forget)
    let _ = AuditLog::record(
        &state.db,
        None,
        "auth.login",
        Some("session"),
        None,
        Some(serde_json::json!({ "username": body.username })),
        None,
        None,
    )
    .await;

    Ok((StatusCode::OK, Json(tokens)))
}

/// Added: BYOK signup payload — TASMail account creation only. The user attaches
/// their own IMAP/SMTP credentials in the onboarding wizard after this.
#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// POST /api/auth/signup
/// Public endpoint. Creates a TASMail mailbox row attached to the synthetic
/// "byok.tasmail" domain (see migration 056) and returns a JWT token pair so the
/// frontend can immediately route the new user into the onboarding wizard.
pub async fn signup(
    State(state): State<AppState>,
    Json(body): Json<SignupRequest>,
) -> Result<(StatusCode, Json<auth_service::TokenPair>), AppError> {
    use crate::models::mailbox::Mailbox;

    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("Valid email is required".into()));
    }
    if body.password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters".into()));
    }
    validation::validate_username(&email)?;

    // Reject duplicates up-front for a clean 409 instead of a unique-violation 500
    if Mailbox::find_by_username(&state.db, &email).await?.is_some() {
        return Err(AppError::Conflict("An account with this email already exists".into()));
    }

    // Look up the synthetic byok.tasmail domain inserted by migration 056
    let domain_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM domains WHERE name = 'byok.tasmail' LIMIT 1")
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("byok.tasmail domain row missing — re-run migration 056")))?;

    let password_hash = auth_service::hash_password(&body.password)?;
    // 1 GiB default quota — TASMail itself doesn't store mail (it's a webmail UI),
    // but quota_bytes is NOT NULL in the schema. Generous default.
    let mailbox = Mailbox::create(
        &state.db,
        &email,
        &password_hash,
        domain_id,
        body.display_name.as_deref(),
        1_073_741_824,
    )
    .await?;

    // Issue tokens immediately so the frontend can move the user into the wizard.
    let tokens = auth_service::issue_token_pair_for_mailbox(&state.db, &state.config.jwt, &mailbox).await?;

    let mailbox_id_str = mailbox.id.to_string();
    let _ = AuditLog::record(
        &state.db,
        Some(mailbox.id),
        "auth.signup",
        Some("mailbox"),
        Some(mailbox_id_str.as_str()),
        None,
        None,
        None,
    )
    .await;

    Ok((StatusCode::CREATED, Json(tokens)))
}

/// POST /api/auth/refresh
pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<auth_service::TokenPair>, AppError> {
    let tokens =
        auth_service::refresh_tokens(&state.db, &state.config.jwt, &body.refresh_token).await?;

    Ok(Json(tokens))
}

/// POST /api/auth/logout
pub async fn logout(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<auth_service::Claims>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in token")))?;

    // Delete all sessions for this user
    crate::models::session::Session::delete_all_for_mailbox(&state.db, mailbox_id).await?;

    // Added: Audit log for logout
    let _ = AuditLog::record(
        &state.db,
        Some(mailbox_id),
        "auth.logout",
        Some("session"),
        None,
        None,
        None,
        None,
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_request_deserialization() {
        let json = r#"{"username": "alice@example.com", "password": "secret123"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "alice@example.com");
        assert_eq!(req.password, "secret123");
    }

    #[test]
    fn test_login_request_missing_username_fails() {
        let json = r#"{"password": "secret123"}"#;
        let result = serde_json::from_str::<LoginRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_login_request_missing_password_fails() {
        let json = r#"{"username": "alice@example.com"}"#;
        let result = serde_json::from_str::<LoginRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_login_request_empty_body_fails() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<LoginRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_login_request_extra_fields_ignored() {
        let json = r#"{"username": "bob@test.com", "password": "pw", "extra": "ignored"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "bob@test.com");
        assert_eq!(req.password, "pw");
    }

    #[test]
    fn test_refresh_request_deserialization() {
        let json = r#"{"refresh_token": "abc-def-123"}"#;
        let req: RefreshRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.refresh_token, "abc-def-123");
    }

    #[test]
    fn test_refresh_request_missing_token_fails() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<RefreshRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_token_pair_serialization() {
        let pair = crate::services::auth_service::TokenPair {
            access_token: "access_abc".to_string(),
            refresh_token: "refresh_xyz".to_string(),
            expires_in: 3600,
        };
        let json = serde_json::to_string(&pair).unwrap();
        assert!(json.contains("access_abc"));
        assert!(json.contains("refresh_xyz"));
        assert!(json.contains("3600"));
    }

    #[test]
    fn test_token_pair_serialization_fields() {
        let pair = crate::services::auth_service::TokenPair {
            access_token: "tok".to_string(),
            refresh_token: "ref".to_string(),
            expires_in: 900,
        };
        let json = serde_json::to_string(&pair).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["access_token"], "tok");
        assert_eq!(value["refresh_token"], "ref");
        assert_eq!(value["expires_in"], 900);
    }
}
