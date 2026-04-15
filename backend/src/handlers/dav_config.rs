// Added: CalDAV/CardDAV configuration management handlers for TMAIL-117
// PURPOSE: CRUD endpoints for managing user's DAV server configurations
// EXTERNAL: Uses ai_config encryption helpers for password encryption

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::ai_config::{decrypt_api_key, derive_encryption_key, encrypt_api_key};
use crate::models::dav_config::{
    CreateDavConfigRequest, DavConfiguration, DavConfigurationResponse, DavTestResult,
    UpdateDavConfigRequest,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: List all DAV configs for the authenticated user (passwords masked)
/// GET /api/dav/configs
pub async fn list_dav_configs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<DavConfigurationResponse>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);
    let configs = DavConfiguration::find_by_user(&state.db, user_id).await?;
    let responses: Vec<DavConfigurationResponse> = configs
        .iter()
        .map(|c| c.to_response(&encryption_key))
        .collect();
    Ok(Json(responses))
}

/// PURPOSE: Create a new DAV configuration
/// POST /api/dav/configs
/// CONSTRAINTS: Password is encrypted before storage
pub async fn create_dav_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateDavConfigRequest>,
) -> Result<(StatusCode, Json<DavConfigurationResponse>), AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    // Added: Validate required fields are not empty
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name is required".to_string()));
    }
    if body.server_url.trim().is_empty() {
        return Err(AppError::BadRequest("Server URL is required".to_string()));
    }
    if body.username.trim().is_empty() {
        return Err(AppError::BadRequest("Username is required".to_string()));
    }

    // Added: Encrypt the password before storing
    let encrypted_password = encrypt_api_key(&body.password, &encryption_key)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to encrypt password: {}", err)))?;

    let config = DavConfiguration::create(
        &state.db,
        user_id,
        &body.name,
        &body.server_url,
        &body.username,
        &encrypted_password,
        body.dav_type.as_str(),
        body.sync_interval_minutes.unwrap_or(60),
        body.enabled.unwrap_or(true),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(config.to_response(&encryption_key))))
}

/// PURPOSE: Get a single DAV config (password masked)
/// GET /api/dav/configs/:id
pub async fn get_dav_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<DavConfigurationResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    let config = DavConfiguration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("DAV configuration not found".to_string()))?;

    Ok(Json(config.to_response(&encryption_key)))
}

/// PURPOSE: Update an existing DAV config
/// PUT /api/dav/configs/:id
pub async fn update_dav_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateDavConfigRequest>,
) -> Result<Json<DavConfigurationResponse>, AppError> {
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

    let dav_type_str = body.dav_type.as_ref().map(|t| t.as_str());

    let config = DavConfiguration::update(
        &state.db,
        id,
        user_id,
        body.name.as_deref(),
        body.server_url.as_deref(),
        body.username.as_deref(),
        encrypted_password.as_deref(),
        dav_type_str,
        body.sync_interval_minutes,
        body.enabled,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("DAV configuration not found".to_string()))?;

    Ok(Json(config.to_response(&encryption_key)))
}

/// PURPOSE: Delete a DAV config
/// DELETE /api/dav/configs/:id
pub async fn delete_dav_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = DavConfiguration::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("DAV configuration not found".to_string()))
    }
}

/// PURPOSE: Trigger a manual sync for a DAV configuration
/// POST /api/dav/configs/:id/sync
/// NOTE: This sets status to syncing; actual sync would be handled by a background service
pub async fn sync_dav_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<DavConfigurationResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    // Added: Verify config exists and belongs to user
    let _config = DavConfiguration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("DAV configuration not found".to_string()))?;

    // Added: Set status to syncing; background worker would pick this up
    let updated = DavConfiguration::update_sync_status(&state.db, id, user_id, "syncing", None)
        .await?
        .ok_or_else(|| AppError::NotFound("DAV configuration not found".to_string()))?;

    Ok(Json(updated.to_response(&encryption_key)))
}

/// PURPOSE: Test connection to a CalDAV/CardDAV server
/// POST /api/dav/configs/:id/test
/// NOTE: Attempts an HTTP OPTIONS or PROPFIND request to validate the server
pub async fn test_dav_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<DavTestResult>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    let config = DavConfiguration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("DAV configuration not found".to_string()))?;

    // Added: Decrypt the password for the test
    let password = decrypt_api_key(&config.encrypted_password, &encryption_key)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to decrypt password: {}", err)))?;

    // Added: Test the DAV connection using an HTTP OPTIONS request
    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to build HTTP client: {}", e)))?;

    let result = client
        .request(reqwest::Method::OPTIONS, &config.server_url)
        .basic_auth(&config.username, Some(&password))
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    let test_result = match result {
        Ok(response) => {
            if response.status().is_success() || response.status().as_u16() == 401 {
                // NOTE: 401 means server is reachable but credentials may be wrong
                if response.status().as_u16() == 401 {
                    DavTestResult {
                        success: false,
                        message: "Server reachable but authentication failed. Check credentials.".to_string(),
                        latency_ms,
                    }
                } else {
                    DavTestResult {
                        success: true,
                        message: format!("Connection successful (HTTP {})", response.status().as_u16()),
                        latency_ms,
                    }
                }
            } else {
                DavTestResult {
                    success: false,
                    message: format!("Server returned HTTP {}", response.status().as_u16()),
                    latency_ms,
                }
            }
        }
        Err(e) => DavTestResult {
            success: false,
            message: format!("Connection failed: {}", e),
            latency_ms,
        },
    };

    // Added: Update sync status based on test result
    let sync_status = if test_result.success { "idle" } else { "error" };
    let sync_error = if test_result.success {
        None
    } else {
        Some(test_result.message.as_str())
    };
    let _ = DavConfiguration::update_sync_status(&state.db, id, user_id, sync_status, sync_error).await;

    Ok(Json(test_result))
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
            "name": "Radicale",
            "server_url": "https://radicale.example.com",
            "username": "user@example.com",
            "password": "dav-password",
            "dav_type": "both",
            "sync_interval_minutes": 30
        });

        let request: CreateDavConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Radicale");
        assert_eq!(request.server_url, "https://radicale.example.com");
        assert_eq!(request.dav_type, crate::models::dav_config::DavType::Both);
    }

    #[test]
    fn test_update_request_deserialization() {
        let json = serde_json::json!({
            "server_url": "https://new-dav.example.com",
            "sync_interval_minutes": 120
        });

        let request: UpdateDavConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.server_url.as_deref(), Some("https://new-dav.example.com"));
        assert_eq!(request.sync_interval_minutes, Some(120));
        assert!(request.name.is_none());
        assert!(request.password.is_none());
    }
}
