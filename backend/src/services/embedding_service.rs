// Added: Embedding generation and similarity search service for TMAIL-106
// PURPOSE: Generates text embeddings via AI provider APIs and searches for similar emails using pgvector
// EXTERNAL: Uses reqwest for HTTP calls to embedding APIs (OpenAI, Google, Ollama, custom)
// CONSTRAINTS: Embedding dimension must be 1536 for OpenAI text-embedding-3-small

use crate::models::ai_config::{AiConfiguration, AiProvider, decrypt_api_key, derive_encryption_key};
use crate::models::email_embedding::{EmailEmbedding, SemanticSearchResult};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Generate an embedding vector from text using the user's AI provider
/// CONSTRAINTS: Returns a Vec<f32> of 1536 dimensions for OpenAI; other providers may vary
/// EXTERNAL: Makes HTTP POST to the provider's embedding API endpoint
pub async fn generate_embedding(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    text: &str,
) -> Result<Vec<f32>, String> {
    let (url, body, auth_header) = build_embedding_request(provider, api_key, model, base_url, text);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {}", err))?;

    let mut request_builder = client
        .post(&url)
        .header("Content-Type", "application/json");

    // Added: Set provider-specific auth headers
    if let Some((header_name, header_value)) = auth_header {
        request_builder = request_builder.header(header_name, header_value);
    }

    let response = request_builder
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("Embedding API request failed: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Embedding API returned status {}: {}",
            status,
            error_body.chars().take(500).collect::<String>()
        ));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Failed to parse embedding API response: {}", err))?;

    extract_embedding_vector(provider, &response_json)
}

/// PURPOSE: Build the URL, request body, and auth header for the embedding API call
/// NOTE: Different providers have different embedding endpoints and request formats
fn build_embedding_request(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    text: &str,
) -> (String, serde_json::Value, Option<(&'static str, String)>) {
    match provider {
        AiProvider::Openai | AiProvider::Custom => {
            let base = base_url.unwrap_or("https://api.openai.com/v1");
            let url = format!("{}/embeddings", base);
            let body = serde_json::json!({
                "model": model,
                "input": text
            });
            (url, body, Some(("Authorization", format!("Bearer {}", api_key))))
        }
        AiProvider::Google => {
            let base = base_url.unwrap_or("https://generativelanguage.googleapis.com/v1beta");
            let url = format!("{}/models/{}:embedContent?key={}", base, model, api_key);
            let body = serde_json::json!({
                "content": {
                    "parts": [{ "text": text }]
                }
            });
            (url, body, None)
        }
        AiProvider::Ollama => {
            let base = base_url.unwrap_or("http://localhost:11434");
            let url = format!("{}/api/embeddings", base);
            let body = serde_json::json!({
                "model": model,
                "prompt": text
            });
            (url, body, None)
        }
        AiProvider::Anthropic => {
            // NOTE: Anthropic does not offer a native embedding API; fall back to OpenAI-compatible format
            let base = base_url.unwrap_or("https://api.openai.com/v1");
            let url = format!("{}/embeddings", base);
            let body = serde_json::json!({
                "model": "text-embedding-3-small",
                "input": text
            });
            (url, body, Some(("Authorization", format!("Bearer {}", api_key))))
        }
    }
}

/// PURPOSE: Extract the embedding vector from the provider-specific response JSON
fn extract_embedding_vector(
    provider: &AiProvider,
    response: &serde_json::Value,
) -> Result<Vec<f32>, String> {
    match provider {
        AiProvider::Openai | AiProvider::Custom | AiProvider::Anthropic => {
            // Added: OpenAI returns { data: [{ embedding: [...] }] }
            response["data"][0]["embedding"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect()
                })
                .ok_or_else(|| "Failed to extract embedding from OpenAI response — expected data[0].embedding array".to_string())
        }
        AiProvider::Google => {
            // Added: Google returns { embedding: { values: [...] } }
            response["embedding"]["values"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect()
                })
                .ok_or_else(|| "Failed to extract embedding from Google response — expected embedding.values array".to_string())
        }
        AiProvider::Ollama => {
            // Added: Ollama returns { embedding: [...] }
            response["embedding"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect()
                })
                .ok_or_else(|| "Failed to extract embedding from Ollama response — expected embedding array".to_string())
        }
    }
}

/// PURPOSE: Search for similar emails using an embedding query vector
/// CONSTRAINTS: Delegates to EmailEmbedding model's pgvector cosine similarity query
pub async fn search_similar(
    pool: &PgPool,
    user_id: Uuid,
    query_embedding: &[f32],
    limit: i64,
) -> Result<Vec<SemanticSearchResult>, sqlx::Error> {
    EmailEmbedding::search_similar(pool, user_id, query_embedding, limit).await
}

/// PURPOSE: Get the user's active AI configuration and decrypt its API key
/// CONSTRAINTS: Requires at least one active AI config with a valid encrypted key
pub async fn get_active_ai_config(
    pool: &PgPool,
    user_id: Uuid,
    jwt_secret: &str,
) -> Result<(AiConfiguration, String), String> {
    let config = AiConfiguration::find_active(pool, user_id)
        .await
        .map_err(|err| format!("Database error finding AI config: {}", err))?
        .ok_or_else(|| {
            "No active AI configuration found. Please configure an AI provider in Settings > AI Config.".to_string()
        })?;

    let encryption_key = derive_encryption_key(jwt_secret);
    let api_key = decrypt_api_key(&config.api_key_encrypted, &encryption_key)
        .map_err(|err| format!("Failed to decrypt API key: {}", err))?;

    Ok((config, api_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_embedding_request_openai() {
        let (url, body, auth) = build_embedding_request(
            &AiProvider::Openai,
            "sk-test-key",
            "text-embedding-3-small",
            None,
            "Hello world",
        );
        assert_eq!(url, "https://api.openai.com/v1/embeddings");
        assert_eq!(body["model"], "text-embedding-3-small");
        assert_eq!(body["input"], "Hello world");
        let (header_name, header_value) = auth.unwrap();
        assert_eq!(header_name, "Authorization");
        assert!(header_value.contains("sk-test-key"));
    }

    #[test]
    fn test_build_embedding_request_openai_custom_base() {
        let (url, _, _) = build_embedding_request(
            &AiProvider::Openai,
            "sk-test",
            "text-embedding-3-small",
            Some("https://my-proxy.example.com/v1"),
            "Test",
        );
        assert_eq!(url, "https://my-proxy.example.com/v1/embeddings");
    }

    #[test]
    fn test_build_embedding_request_google() {
        let (url, body, auth) = build_embedding_request(
            &AiProvider::Google,
            "my-api-key",
            "text-embedding-004",
            None,
            "Hello world",
        );
        assert!(url.contains("text-embedding-004"));
        assert!(url.contains("embedContent"));
        assert!(url.contains("my-api-key"));
        assert!(body["content"]["parts"][0]["text"].as_str().unwrap().contains("Hello world"));
        assert!(auth.is_none());
    }

    #[test]
    fn test_build_embedding_request_ollama() {
        let (url, body, auth) = build_embedding_request(
            &AiProvider::Ollama,
            "",
            "nomic-embed-text",
            None,
            "Test text",
        );
        assert_eq!(url, "http://localhost:11434/api/embeddings");
        assert_eq!(body["model"], "nomic-embed-text");
        assert_eq!(body["prompt"], "Test text");
        assert!(auth.is_none());
    }

    #[test]
    fn test_build_embedding_request_ollama_custom_host() {
        let (url, _, _) = build_embedding_request(
            &AiProvider::Ollama,
            "",
            "nomic-embed-text",
            Some("http://gpu-server:11434"),
            "Test",
        );
        assert_eq!(url, "http://gpu-server:11434/api/embeddings");
    }

    #[test]
    fn test_build_embedding_request_custom_provider() {
        let (url, body, auth) = build_embedding_request(
            &AiProvider::Custom,
            "custom-key",
            "my-embed-model",
            Some("https://my-llm.example.com/v1"),
            "Text to embed",
        );
        assert_eq!(url, "https://my-llm.example.com/v1/embeddings");
        assert_eq!(body["model"], "my-embed-model");
        let (_, header_value) = auth.unwrap();
        assert!(header_value.contains("custom-key"));
    }

    #[test]
    fn test_build_embedding_request_anthropic_fallback() {
        // NOTE: Anthropic doesn't have native embeddings, falls back to OpenAI-compatible
        let (url, body, auth) = build_embedding_request(
            &AiProvider::Anthropic,
            "anthropic-key",
            "claude-sonnet-4-20250514",
            None,
            "Test text",
        );
        assert_eq!(url, "https://api.openai.com/v1/embeddings");
        assert_eq!(body["model"], "text-embedding-3-small");
        assert!(auth.is_some());
    }

    #[test]
    fn test_extract_embedding_vector_openai() {
        let response = serde_json::json!({
            "data": [{
                "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
            }]
        });
        let vec = extract_embedding_vector(&AiProvider::Openai, &response).unwrap();
        assert_eq!(vec.len(), 5);
        assert!((vec[0] - 0.1).abs() < 0.001);
        assert!((vec[4] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_extract_embedding_vector_google() {
        let response = serde_json::json!({
            "embedding": {
                "values": [0.1, 0.2, 0.3]
            }
        });
        let vec = extract_embedding_vector(&AiProvider::Google, &response).unwrap();
        assert_eq!(vec.len(), 3);
    }

    #[test]
    fn test_extract_embedding_vector_ollama() {
        let response = serde_json::json!({
            "embedding": [0.5, 0.6, 0.7]
        });
        let vec = extract_embedding_vector(&AiProvider::Ollama, &response).unwrap();
        assert_eq!(vec.len(), 3);
        assert!((vec[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_extract_embedding_vector_openai_missing_data() {
        let response = serde_json::json!({ "data": [] });
        let result = extract_embedding_vector(&AiProvider::Openai, &response);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OpenAI"));
    }

    #[test]
    fn test_extract_embedding_vector_google_missing_values() {
        let response = serde_json::json!({ "embedding": {} });
        let result = extract_embedding_vector(&AiProvider::Google, &response);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Google"));
    }

    #[test]
    fn test_extract_embedding_vector_ollama_missing_embedding() {
        let response = serde_json::json!({ "done": true });
        let result = extract_embedding_vector(&AiProvider::Ollama, &response);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ollama"));
    }

    #[test]
    fn test_extract_embedding_vector_custom_uses_openai_format() {
        let response = serde_json::json!({
            "data": [{
                "embedding": [0.9, 0.8]
            }]
        });
        let vec = extract_embedding_vector(&AiProvider::Custom, &response).unwrap();
        assert_eq!(vec.len(), 2);
        assert!((vec[0] - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_extract_embedding_vector_anthropic_uses_openai_format() {
        // NOTE: Anthropic provider falls back to OpenAI embedding format
        let response = serde_json::json!({
            "data": [{
                "embedding": [0.1, 0.2]
            }]
        });
        let vec = extract_embedding_vector(&AiProvider::Anthropic, &response).unwrap();
        assert_eq!(vec.len(), 2);
    }
}
