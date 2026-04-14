// Added: AI configuration management handlers for TMAIL-105
// PURPOSE: CRUD endpoints for managing BYOK AI provider configurations
// EXTERNAL: Uses ai_client service for test and summarize operations

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::ai_config::{
    AiConfiguration, AiConfigurationResponse, CreateAiConfigRequest, SummarizeRequest,
    UpdateAiConfigRequest, decrypt_api_key, derive_encryption_key, encrypt_api_key,
};
use crate::services::ai_client;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: List all AI configs for the authenticated user (keys masked)
/// GET /api/ai/config
pub async fn list_ai_configs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<AiConfigurationResponse>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);
    let configs = AiConfiguration::find_by_user(&state.db, user_id).await?;
    let responses: Vec<AiConfigurationResponse> = configs
        .iter()
        .map(|c| c.to_response(&encryption_key))
        .collect();
    Ok(Json(responses))
}

/// PURPOSE: Create a new AI provider configuration
/// POST /api/ai/config
/// CONSTRAINTS: API key is encrypted before storage
pub async fn create_ai_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateAiConfigRequest>,
) -> Result<(StatusCode, Json<AiConfigurationResponse>), AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    // Added: Validate model name is not empty
    if body.model_name.trim().is_empty() {
        return Err(AppError::BadRequest("Model name is required".to_string()));
    }

    // Added: Encrypt the API key before storing
    let api_key_encrypted = encrypt_api_key(&body.api_key, &encryption_key)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to encrypt API key: {}", err)))?;

    let config = AiConfiguration::create(
        &state.db,
        user_id,
        &body.provider,
        &api_key_encrypted,
        &body.model_name,
        body.base_url.as_deref(),
        body.max_tokens.unwrap_or(500),
        body.temperature.unwrap_or(0.7),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(config.to_response(&encryption_key))))
}

/// PURPOSE: Update an existing AI config
/// PUT /api/ai/config/:id
pub async fn update_ai_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateAiConfigRequest>,
) -> Result<Json<AiConfigurationResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    // Added: Encrypt new API key if provided
    let encrypted_key = match &body.api_key {
        Some(key) => Some(
            encrypt_api_key(key, &encryption_key)
                .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to encrypt API key: {}", err)))?,
        ),
        None => None,
    };

    let config = AiConfiguration::update(
        &state.db,
        id,
        user_id,
        encrypted_key.as_deref(),
        body.model_name.as_deref(),
        body.base_url.as_ref().map(|u| Some(u.as_str())),
        body.max_tokens,
        body.temperature,
        body.active,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("AI configuration not found".to_string()))?;

    Ok(Json(config.to_response(&encryption_key)))
}

/// PURPOSE: Delete an AI config
/// DELETE /api/ai/config/:id
pub async fn delete_ai_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = AiConfiguration::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("AI configuration not found".to_string()))
    }
}

/// PURPOSE: Test an AI config by making a simple completion request
/// POST /api/ai/config/:id/test
/// NOTE: Sends a short test prompt to verify the API key and model work
pub async fn test_ai_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    let config = AiConfiguration::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("AI configuration not found".to_string()))?;

    // Added: Decrypt the API key for the test request
    let api_key = decrypt_api_key(&config.api_key_encrypted, &encryption_key)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to decrypt API key: {}", err)))?;

    let result = ai_client::call_ai_provider(
        &config.provider,
        &api_key,
        &config.model_name,
        config.base_url.as_deref(),
        "You are a test assistant.",
        "Respond with exactly: Connection successful",
        50,
        0.0,
    )
    .await;

    match result {
        Ok(response_text) => Ok(Json(serde_json::json!({
            "success": true,
            "message": "API key verified successfully",
            "response": response_text
        }))),
        Err(error_message) => Ok(Json(serde_json::json!({
            "success": false,
            "message": error_message
        }))),
    }
}

/// PURPOSE: Summarize an email using the user's active AI configuration
/// POST /api/ai/summarize
/// CONSTRAINTS: Requires at least one active AI config
pub async fn summarize_email(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SummarizeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let encryption_key = derive_encryption_key(&state.config.jwt.secret);

    // Added: Find the user's active AI config
    let config = AiConfiguration::find_active(&state.db, user_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(
                "No active AI configuration found. Please configure an AI provider in Settings > AI Config."
                    .to_string(),
            )
        })?;

    if body.email_text.trim().is_empty() {
        return Err(AppError::BadRequest("Email text is required for summarization".to_string()));
    }

    let api_key = decrypt_api_key(&config.api_key_encrypted, &encryption_key)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("Failed to decrypt API key: {}", err)))?;

    let summary = ai_client::summarize_email(
        &config.provider,
        &api_key,
        &config.model_name,
        config.base_url.as_deref(),
        config.max_tokens,
        config.temperature,
        &body.email_text,
    )
    .await
    .map_err(|err| AppError::BadRequest(format!("AI summarization failed: {}", err)))?;

    Ok(Json(serde_json::json!({
        "summary": summary,
        "provider": config.provider,
        "model": config.model_name
    })))
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
}
