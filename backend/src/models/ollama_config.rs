// Added: Ollama configuration and model cache models for TMAIL-102
// PURPOSE: Stores admin-level Ollama server settings and cached model metadata
// CONSTRAINTS: ollama_config is a single-row table; ollama_model_cache has UNIQUE model_name

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Ollama server configuration (single-row, admin-managed)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OllamaConfig {
    pub id: Uuid,
    pub base_url: String,
    pub enabled: bool,
    pub default_model: Option<String>,
    pub max_context_length: Option<i32>,
    pub gpu_layers: Option<i32>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Cached metadata about a model available in the Ollama instance
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OllamaModelCache {
    pub id: Uuid,
    pub model_name: String,
    pub size_bytes: Option<i64>,
    pub parameter_count: Option<String>,
    pub quantization: Option<String>,
    pub last_pulled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Request body for updating the Ollama config
#[derive(Debug, Deserialize)]
pub struct UpdateOllamaConfigRequest {
    pub base_url: Option<String>,
    pub enabled: Option<bool>,
    pub default_model: Option<String>,
    pub max_context_length: Option<i32>,
    pub gpu_layers: Option<i32>,
}

/// PURPOSE: Request body for pulling a model
#[derive(Debug, Deserialize)]
pub struct PullModelRequest {
    pub model: String,
}

/// PURPOSE: Ollama server health and status information returned to the frontend
#[derive(Debug, Serialize)]
pub struct OllamaStatus {
    pub running: bool,
    pub version: Option<String>,
    pub models: Vec<OllamaModelInfo>,
}

/// PURPOSE: Model info returned from Ollama /api/tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: Option<u64>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
    pub modified_at: Option<String>,
}

impl OllamaConfig {
    /// PURPOSE: Get the current Ollama config (always returns the single row)
    pub async fn get(pool: &PgPool) -> Result<OllamaConfig, sqlx::Error> {
        sqlx::query_as::<_, OllamaConfig>(
            "SELECT * FROM ollama_config ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Upsert the Ollama config (update the single row)
    pub async fn upsert(
        pool: &PgPool,
        base_url: Option<&str>,
        enabled: Option<bool>,
        default_model: Option<&str>,
        max_context_length: Option<i32>,
        gpu_layers: Option<i32>,
    ) -> Result<OllamaConfig, sqlx::Error> {
        // NOTE: Update the first row; there should always be exactly one
        sqlx::query_as::<_, OllamaConfig>(
            "UPDATE ollama_config SET \
                base_url = COALESCE($1, base_url), \
                enabled = COALESCE($2, enabled), \
                default_model = COALESCE($3, default_model), \
                max_context_length = COALESCE($4, max_context_length), \
                gpu_layers = COALESCE($5, gpu_layers), \
                updated_at = NOW() \
             WHERE id = (SELECT id FROM ollama_config LIMIT 1) \
             RETURNING *",
        )
        .bind(base_url)
        .bind(enabled)
        .bind(default_model)
        .bind(max_context_length)
        .bind(gpu_layers)
        .fetch_one(pool)
        .await
    }
}

impl OllamaModelCache {
    /// PURPOSE: List all cached models ordered by name
    pub async fn list(pool: &PgPool) -> Result<Vec<OllamaModelCache>, sqlx::Error> {
        sqlx::query_as::<_, OllamaModelCache>(
            "SELECT * FROM ollama_model_cache ORDER BY model_name ASC",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Upsert a model into the cache (insert or update on conflict)
    pub async fn upsert(
        pool: &PgPool,
        model_name: &str,
        size_bytes: Option<i64>,
        parameter_count: Option<&str>,
        quantization: Option<&str>,
    ) -> Result<OllamaModelCache, sqlx::Error> {
        sqlx::query_as::<_, OllamaModelCache>(
            "INSERT INTO ollama_model_cache (model_name, size_bytes, parameter_count, quantization, last_pulled_at) \
             VALUES ($1, $2, $3, $4, NOW()) \
             ON CONFLICT (model_name) DO UPDATE SET \
                size_bytes = COALESCE($2, ollama_model_cache.size_bytes), \
                parameter_count = COALESCE($3, ollama_model_cache.parameter_count), \
                quantization = COALESCE($4, ollama_model_cache.quantization), \
                last_pulled_at = NOW() \
             RETURNING *",
        )
        .bind(model_name)
        .bind(size_bytes)
        .bind(parameter_count)
        .bind(quantization)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Delete a model from the cache by name
    pub async fn delete_by_name(pool: &PgPool, model_name: &str) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM ollama_model_cache WHERE model_name = $1")
                .bind(model_name)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config_serialization() {
        let config = OllamaConfig {
            id: Uuid::new_v4(),
            base_url: "http://localhost:11434".to_string(),
            enabled: true,
            default_model: Some("llama3.2".to_string()),
            max_context_length: Some(4096),
            gpu_layers: Some(-1),
            updated_at: Some(chrono::Utc::now()),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["base_url"], "http://localhost:11434");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["default_model"], "llama3.2");
        assert_eq!(json["max_context_length"], 4096);
        assert_eq!(json["gpu_layers"], -1);
    }

    #[test]
    fn test_ollama_config_deserialization() {
        let json = serde_json::json!({
            "id": Uuid::new_v4(),
            "base_url": "http://gpu-server:11434",
            "enabled": false,
            "default_model": "mistral",
            "max_context_length": 8192,
            "gpu_layers": 35,
            "updated_at": null
        });
        let config: OllamaConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.base_url, "http://gpu-server:11434");
        assert!(!config.enabled);
        assert_eq!(config.default_model.as_deref(), Some("mistral"));
        assert_eq!(config.max_context_length, Some(8192));
        assert_eq!(config.gpu_layers, Some(35));
    }

    #[test]
    fn test_update_ollama_config_request_full() {
        let json = serde_json::json!({
            "base_url": "http://192.168.1.100:11434",
            "enabled": true,
            "default_model": "codellama",
            "max_context_length": 16384,
            "gpu_layers": 40
        });
        let req: UpdateOllamaConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.base_url.as_deref(), Some("http://192.168.1.100:11434"));
        assert_eq!(req.enabled, Some(true));
        assert_eq!(req.default_model.as_deref(), Some("codellama"));
        assert_eq!(req.max_context_length, Some(16384));
        assert_eq!(req.gpu_layers, Some(40));
    }

    #[test]
    fn test_update_ollama_config_request_partial() {
        let json = serde_json::json!({
            "enabled": false
        });
        let req: UpdateOllamaConfigRequest = serde_json::from_value(json).unwrap();
        assert!(req.base_url.is_none());
        assert_eq!(req.enabled, Some(false));
        assert!(req.default_model.is_none());
    }

    #[test]
    fn test_update_ollama_config_request_empty() {
        let json = serde_json::json!({});
        let req: UpdateOllamaConfigRequest = serde_json::from_value(json).unwrap();
        assert!(req.base_url.is_none());
        assert!(req.enabled.is_none());
        assert!(req.default_model.is_none());
        assert!(req.max_context_length.is_none());
        assert!(req.gpu_layers.is_none());
    }

    #[test]
    fn test_pull_model_request_deserialization() {
        let json = serde_json::json!({ "model": "llama3.2" });
        let req: PullModelRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.model, "llama3.2");
    }

    #[test]
    fn test_pull_model_request_missing_model_fails() {
        let json = serde_json::json!({});
        let result = serde_json::from_value::<PullModelRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_ollama_model_cache_serialization() {
        let model = OllamaModelCache {
            id: Uuid::new_v4(),
            model_name: "llama3.2".to_string(),
            size_bytes: Some(4_100_000_000),
            parameter_count: Some("8B".to_string()),
            quantization: Some("Q4_0".to_string()),
            last_pulled_at: Some(chrono::Utc::now()),
            created_at: Some(chrono::Utc::now()),
        };
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["model_name"], "llama3.2");
        assert_eq!(json["size_bytes"], 4_100_000_000_i64);
        assert_eq!(json["parameter_count"], "8B");
        assert_eq!(json["quantization"], "Q4_0");
    }

    #[test]
    fn test_ollama_model_info_serialization() {
        let info = OllamaModelInfo {
            name: "codellama:13b".to_string(),
            size: Some(7_400_000_000),
            parameter_size: Some("13B".to_string()),
            quantization_level: Some("Q4_0".to_string()),
            modified_at: Some("2024-01-15T10:30:00Z".to_string()),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["name"], "codellama:13b");
        assert_eq!(json["size"], 7_400_000_000_u64);
        assert_eq!(json["parameter_size"], "13B");
    }

    #[test]
    fn test_ollama_status_serialization() {
        let status = OllamaStatus {
            running: true,
            version: Some("0.3.14".to_string()),
            models: vec![
                OllamaModelInfo {
                    name: "llama3.2".to_string(),
                    size: Some(4_100_000_000),
                    parameter_size: Some("8B".to_string()),
                    quantization_level: Some("Q4_0".to_string()),
                    modified_at: None,
                },
            ],
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["running"], true);
        assert_eq!(json["version"], "0.3.14");
        let models = json["models"].as_array().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["name"], "llama3.2");
    }

    #[test]
    fn test_ollama_status_not_running() {
        let status = OllamaStatus {
            running: false,
            version: None,
            models: vec![],
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["running"], false);
        assert!(json["version"].is_null());
        assert_eq!(json["models"].as_array().unwrap().len(), 0);
    }
}
