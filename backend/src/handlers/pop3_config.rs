// Added: POP3 configuration management handlers for Dovecot POP3 access (TMAIL-133)
// PURPOSE: CRUD endpoints for managing user's POP3 access settings
// NOTE: Backend manages config; Dovecot handles actual POP3 protocol

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::pop3_config::{Pop3Configuration, Pop3Status, UpdatePop3ConfigRequest};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: Get current user's POP3 configuration
/// GET /api/pop3/config
pub async fn get_pop3_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Option<Pop3Configuration>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let config = Pop3Configuration::get_by_user(&state.db, user_id).await?;
    Ok(Json(config))
}

/// PURPOSE: Create or update POP3 configuration (upsert)
/// PUT /api/pop3/config
pub async fn update_pop3_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<UpdatePop3ConfigRequest>,
) -> Result<Json<Pop3Configuration>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate retention_days is positive if set
    if let Some(Some(days)) = body.retention_days {
        if days <= 0 {
            return Err(AppError::BadRequest(
                "Retention days must be a positive integer".to_string(),
            ));
        }
    }

    // Added: Fetch existing config for defaults, or use sensible defaults for new
    let existing = Pop3Configuration::get_by_user(&state.db, user_id).await?;
    let enabled = body.enabled.unwrap_or_else(|| {
        existing.as_ref().map(|c| c.enabled).unwrap_or(false)
    });
    let delete_after_download = body.delete_after_download.unwrap_or_else(|| {
        existing.as_ref().map(|c| c.delete_after_download).unwrap_or(false)
    });
    let retention_days = match body.retention_days {
        Some(val) => val,
        None => existing.as_ref().and_then(|c| c.retention_days),
    };

    let config = Pop3Configuration::create_or_update(
        &state.db,
        user_id,
        enabled,
        delete_after_download,
        retention_days,
    )
    .await?;

    Ok(Json(config))
}

/// PURPOSE: Delete POP3 configuration (disables POP3 access)
/// DELETE /api/pop3/config
pub async fn delete_pop3_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = Pop3Configuration::delete_by_user(&state.db, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("POP3 configuration not found".to_string()))
    }
}

/// PURPOSE: Get POP3 connection info for mail client setup
/// GET /api/pop3/status
/// NOTE: Returns server hostname, port, and encryption from the IMAP config
///       (Dovecot serves both IMAP and POP3 on the same host)
pub async fn get_pop3_status(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Pop3Status>, AppError> {
    let _user_id = parse_user_id(&claims)?;

    // Added: Build POP3 connection info from the server's IMAP config
    // NOTE: Dovecot typically serves POP3S on port 995, same host as IMAP
    let status = Pop3Status {
        server: state.config.imap.host.clone(),
        port: 995,
        encryption: "SSL/TLS".to_string(),
        username_format: format!("{}@{}", claims.username, state.config.imap.host),
    };

    Ok(Json(status))
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

    #[test]
    fn test_update_request_deserialization_full() {
        let json = serde_json::json!({
            "enabled": true,
            "delete_after_download": true,
            "retention_days": 30
        });

        let request: UpdatePop3ConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.enabled, Some(true));
        assert_eq!(request.delete_after_download, Some(true));
    }

    #[test]
    fn test_update_request_deserialization_partial() {
        let json = serde_json::json!({
            "enabled": false
        });

        let request: UpdatePop3ConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.enabled, Some(false));
        assert!(request.delete_after_download.is_none());
        assert!(request.retention_days.is_none());
    }

    #[test]
    fn test_update_request_empty() {
        let json = serde_json::json!({});
        let request: UpdatePop3ConfigRequest = serde_json::from_value(json).unwrap();
        assert!(request.enabled.is_none());
        assert!(request.delete_after_download.is_none());
        assert!(request.retention_days.is_none());
    }

    #[test]
    fn test_pop3_status_serialization() {
        let status = Pop3Status {
            server: "mail.example.com".to_string(),
            port: 995,
            encryption: "SSL/TLS".to_string(),
            username_format: "user@mail.example.com".to_string(),
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["server"], "mail.example.com");
        assert_eq!(json["port"], 995);
        assert_eq!(json["encryption"], "SSL/TLS");
    }
}
