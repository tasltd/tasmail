use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IMAP error: {0}")]
    Imap(String),

    #[error("SMTP error: {0}")]
    Smtp(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    // Added: Used when a feature is wired but not yet provisioned (e.g., payment provider missing config row).
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    // Added (TMAIL-102): Per-user/per-route quota was exceeded (e.g., AI inference rate limit).
    // Maps to HTTP 429 so SPAs can surface a retry-friendly message.
    #[error("Too many requests: {0}")]
    TooManyRequests(String),

    // Added (TMAIL-273): Per-account brute-force lockout was triggered or is
    // still in effect. Maps to HTTP 423 Locked. The body intentionally stays
    // generic — no remaining attempts, no countdown, no enumeration.
    #[error("Account locked: {0}")]
    AccountLocked(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Imap(msg) => {
                tracing::error!("IMAP error: {}", msg);
                (StatusCode::BAD_GATEWAY, format!("Mail server error: {msg}"))
            }
            AppError::Smtp(msg) => {
                tracing::error!("SMTP error: {}", msg);
                (StatusCode::BAD_GATEWAY, format!("Mail sending error: {msg}"))
            }
            AppError::Internal(e) => {
                tracing::error!("Internal error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::ServiceUnavailable(msg) => {
                tracing::warn!("Service unavailable: {}", msg);
                (StatusCode::SERVICE_UNAVAILABLE, msg.clone())
            }
            // Added (TMAIL-102): AI rate-limit (10/min/user) and other per-user quotas
            AppError::TooManyRequests(msg) => {
                tracing::warn!("Rate limit exceeded: {}", msg);
                (StatusCode::TOO_MANY_REQUESTS, msg.clone())
            }
            // Added (TMAIL-273): Per-account brute-force lockout. Returns 423
            // with a generic message — no remaining attempts or countdown so
            // attackers can't fingerprint how close they were to the threshold.
            AppError::AccountLocked(msg) => {
                tracing::warn!("Account lockout enforced: {}", msg);
                (StatusCode::LOCKED, msg.clone())
            }
        };

        let body = json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn test_error_display() {
        let err = AppError::Unauthorized("bad token".to_string());
        assert_eq!(err.to_string(), "Authentication failed: bad token");

        let err = AppError::NotFound("user".to_string());
        assert_eq!(err.to_string(), "Not found: user");

        let err = AppError::BadRequest("missing field".to_string());
        assert_eq!(err.to_string(), "Bad request: missing field");
    }

    #[test]
    fn test_error_status_codes() {
        let cases = vec![
            (AppError::Unauthorized("".to_string()), StatusCode::UNAUTHORIZED),
            (AppError::Forbidden("".to_string()), StatusCode::FORBIDDEN),
            (AppError::NotFound("".to_string()), StatusCode::NOT_FOUND),
            (AppError::BadRequest("".to_string()), StatusCode::BAD_REQUEST),
            (AppError::Conflict("".to_string()), StatusCode::CONFLICT),
            (AppError::Imap("".to_string()), StatusCode::BAD_GATEWAY),
            (AppError::Smtp("".to_string()), StatusCode::BAD_GATEWAY),
            // Added (TMAIL-102): AI rate-limit maps to 429
            (
                AppError::TooManyRequests("".to_string()),
                StatusCode::TOO_MANY_REQUESTS,
            ),
            (
                AppError::ServiceUnavailable("".to_string()),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            // Added (TMAIL-273): AccountLocked → 423 Locked
            (
                AppError::AccountLocked("".to_string()),
                StatusCode::LOCKED,
            ),
        ];

        for (error, expected_status) in cases {
            let response = error.into_response();
            assert_eq!(response.status(), expected_status);
        }
    }

    #[test]
    fn test_internal_error_from_anyhow() {
        let err: AppError = anyhow::anyhow!("something broke").into();
        assert!(matches!(err, AppError::Internal(_)));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
