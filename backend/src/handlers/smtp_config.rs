// Added: SMTP configuration management handlers for BYO-SMTP (TMAIL-48)
// PURPOSE: CRUD endpoints for managing user's external SMTP server configurations
// EXTERNAL: Uses smtp_tester service for connection testing

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::ai_config::{decrypt_api_key, derive_encryption_key, encrypt_api_key};
use crate::models::smtp_config::{
    CreateSmtpConfigRequest, SmtpConfiguration, SmtpConfigurationResponse, UpdateSmtpConfigRequest,
};
use crate::services::auth_service::Claims;
use crate::services::smtp_tester;
use crate::state::AppState;

/// PURPOSE: List all SMTP configs for the authenticated user (passwords masked)
/// GET /api/smtp-configs
pub async fn list_smtp_configs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<SmtpConfigurationResponse>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);
    let configs = SmtpConfiguration::find_by_user(&state.db, user_id).await?;
    let responses: Vec<SmtpConfigurationResponse> = configs
        .iter()
        .map(|c| c.to_response(&encryption_key))
        .collect();
    Ok(Json(responses))
}

/// PURPOSE: Create a new SMTP configuration
/// POST /api/smtp-configs
/// CONSTRAINTS: Password is encrypted before storage
pub async fn create_smtp_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateSmtpConfigRequest>,
) -> Result<(StatusCode, Json<SmtpConfigurationResponse>), AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    // Added: Validate required fields are not empty
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name is required".to_string()));
    }
    if body.host.trim().is_empty() {
        return Err(AppError::BadRequest("Host is required".to_string()));
    }
    if body.username.trim().is_empty() {
        return Err(AppError::BadRequest("Username is required".to_string()));
    }

    // Added: Encrypt the password before storing
    let encrypted_password = encrypt_api_key(&body.password, &encryption_key)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to encrypt password: {}", err)))?;

    let encryption_str = body
        .encryption
        .as_ref()
        .map(|e| e.as_str())
        .unwrap_or("starttls");

    let config = SmtpConfiguration::create(
        &state.db,
        user_id,
        &body.name,
        &body.host,
        body.port.unwrap_or(587),
        &body.username,
        &encrypted_password,
        encryption_str,
        body.from_address.as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(config.to_response(&encryption_key))))
}

/// PURPOSE: Get a single SMTP config (password masked)
/// GET /api/smtp-configs/:id
pub async fn get_smtp_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SmtpConfigurationResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    let config = SmtpConfiguration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("SMTP configuration not found".to_string()))?;

    Ok(Json(config.to_response(&encryption_key)))
}

/// PURPOSE: Update an existing SMTP config
/// PUT /api/smtp-configs/:id
pub async fn update_smtp_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateSmtpConfigRequest>,
) -> Result<Json<SmtpConfigurationResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    // Added: Encrypt new password if provided
    let encrypted_password = match &body.password {
        Some(pw) => Some(
            encrypt_api_key(pw, &encryption_key)
                .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to encrypt password: {}", err)))?,
        ),
        None => None,
    };

    let encryption_str = body.encryption.as_ref().map(|e| e.as_str());

    let config = SmtpConfiguration::update(
        &state.db,
        id,
        user_id,
        body.name.as_deref(),
        body.host.as_deref(),
        body.port,
        body.username.as_deref(),
        encrypted_password.as_deref(),
        encryption_str,
        body.from_address.as_ref().map(|a| Some(a.as_str())),
    )
    .await?
    .ok_or_else(|| AppError::NotFound("SMTP configuration not found".to_string()))?;

    Ok(Json(config.to_response(&encryption_key)))
}

/// PURPOSE: Delete an SMTP config
/// DELETE /api/smtp-configs/:id
pub async fn delete_smtp_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = SmtpConfiguration::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("SMTP configuration not found".to_string()))
    }
}

/// PURPOSE: Test SMTP connection by authenticating and sending a test email
/// POST /api/smtp-configs/:id/test
pub async fn test_smtp_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<smtp_tester::SmtpTestResult>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    let config = SmtpConfiguration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("SMTP configuration not found".to_string()))?;

    // Added: Decrypt the password for the test
    let password = decrypt_api_key(&config.encrypted_password, &encryption_key)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to decrypt password: {}", err)))?;

    let result = smtp_tester::test_smtp_connection(&config, &password).await;

    // Added: Update the verified status and last_tested_at timestamp
    let _ = SmtpConfiguration::update_test_result(&state.db, id, user_id, result.success).await;

    Ok(Json(result))
}

/// PURPOSE: Set an SMTP config as the default for sending
/// POST /api/smtp-configs/:id/default
pub async fn set_default_smtp(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SmtpConfigurationResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    let config = SmtpConfiguration::set_default(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("SMTP configuration not found".to_string()))?;

    Ok(Json(config.to_response(&encryption_key)))
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
    fn test_create_request_deserialization() {
        let json = serde_json::json!({
            "name": "Gmail",
            "host": "smtp.gmail.com",
            "port": 587,
            "username": "user@gmail.com",
            "password": "app-password",
            "encryption": "starttls",
            "from_address": "user@gmail.com"
        });

        let request: CreateSmtpConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Gmail");
        assert_eq!(request.host, "smtp.gmail.com");
    }

    #[test]
    fn test_update_request_deserialization() {
        let json = serde_json::json!({
            "host": "new-host.com",
            "port": 465
        });

        let request: UpdateSmtpConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.host.as_deref(), Some("new-host.com"));
        assert_eq!(request.port, Some(465));
        assert!(request.name.is_none());
        assert!(request.password.is_none());
    }
}
