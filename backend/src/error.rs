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
