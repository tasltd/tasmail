// Added: Ollama management handlers for TMAIL-102
// PURPOSE: Admin endpoints for Ollama config, status, and model management
// EXTERNAL: Uses ollama_client service for Ollama API calls

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::ollama_config::{
    OllamaConfig, OllamaModelCache, OllamaStatus, PullModelRequest, UpdateOllamaConfigRequest,
};
use crate::services::auth_service::{self, Claims};
use crate::services::ollama_client;
use crate::state::AppState;

/// PURPOSE: Get the current Ollama configuration
/// GET /api/admin/ollama/config
pub async fn get_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<OllamaConfig>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let config = OllamaConfig::get(&state.db).await?;
    Ok(Json(config))
}

/// PURPOSE: Update the Ollama configuration
/// PUT /api/admin/ollama/config
pub async fn update_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<UpdateOllamaConfigRequest>,
) -> Result<Json<OllamaConfig>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate base_url is not empty if provided
    if let Some(ref url) = body.base_url {
        if url.trim().is_empty() {
            return Err(AppError::BadRequest("Base URL cannot be empty".to_string()));
        }
    }

    let config = OllamaConfig::upsert(
        &state.db,
        body.base_url.as_deref(),
        body.enabled,
        body.default_model.as_deref(),
        body.max_context_length,
        body.gpu_layers,
    )
    .await?;

    Ok(Json(config))
}

/// PURPOSE: Check Ollama server health and list available models
/// GET /api/admin/ollama/status
pub async fn get_status(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<OllamaStatus>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let config = OllamaConfig::get(&state.db).await?;

    // Added: Ping the Ollama server for health and version
    let health = ollama_client::check_health(&config.base_url).await;

    let models = if health.running {
        // Added: Fetch model list from the running server
        ollama_client::list_models(&config.base_url)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok(Json(OllamaStatus {
        running: health.running,
        version: health.version,
        models,
    }))
}

/// PURPOSE: Pull (download) a model on the Ollama server
/// POST /api/admin/ollama/models/pull
pub async fn pull_model(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<PullModelRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    if body.model.trim().is_empty() {
        return Err(AppError::BadRequest("Model name is required".to_string()));
    }

    let config = OllamaConfig::get(&state.db).await?;

    let result = ollama_client::pull_model(&config.base_url, &body.model).await;

    if result.success {
        // Added: Refresh model list and update cache after successful pull
        if let Ok(models) = ollama_client::list_models(&config.base_url).await {
            for m in &models {
                let _ = OllamaModelCache::upsert(
                    &state.db,
                    &m.name,
                    m.size.map(|s| s as i64),
                    m.parameter_size.as_deref(),
                    m.quantization_level.as_deref(),
                )
                .await;
            }
        }
    }

    Ok(Json(serde_json::json!({
        "success": result.success,
        "message": result.message
    })))
}

/// PURPOSE: Delete a model from the Ollama server and cache
/// DELETE /api/admin/ollama/models/{name}
pub async fn delete_model(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("Model name is required".to_string()));
    }

    let config = OllamaConfig::get(&state.db).await?;

    ollama_client::delete_model(&config.base_url, &name)
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to delete model: {}", e)))?;

    // Added: Remove from local cache
    let _ = OllamaModelCache::delete_by_name(&state.db, &name).await;

    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: List all cached models from the database
/// GET /api/admin/ollama/models
pub async fn list_cached_models(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<OllamaModelCache>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let models = OllamaModelCache::list(&state.db).await?;
    Ok(Json(models))
}

#[cfg(test)]
mod tests {
    use crate::models::ollama_config::{PullModelRequest, UpdateOllamaConfigRequest};

    #[test]
    fn test_update_config_request_deserialization() {
        let json = serde_json::json!({
            "base_url": "http://gpu-server:11434",
            "enabled": true,
            "default_model": "codellama",
            "max_context_length": 8192,
            "gpu_layers": 40
        });
        let req: UpdateOllamaConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.base_url.as_deref(), Some("http://gpu-server:11434"));
        assert_eq!(req.enabled, Some(true));
    }

    #[test]
    fn test_pull_model_request_deserialization() {
        let json = serde_json::json!({ "model": "llama3.2:latest" });
        let req: PullModelRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.model, "llama3.2:latest");
    }
}
