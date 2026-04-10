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
    let tokens = auth_service::authenticate(
        &state.db,
        &state.config.jwt,
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
