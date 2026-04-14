// Added: AI configuration model for BYOK AI integration (TMAIL-105)
// PURPOSE: Stores per-user AI provider configs with encrypted API keys for email summarization and smart replies
// CONSTRAINTS: Must match ai_provider ENUM in PostgreSQL (migration 032)
// EXTERNAL: Uses aes-gcm crate for AES-256-GCM encryption of API keys at rest

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, AeadCore, Nonce,
};

/// PURPOSE: Supported AI providers for BYOK integration
/// CONSTRAINTS: Must match the ai_provider ENUM in PostgreSQL (migration 032)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "ai_provider", rename_all = "snake_case")]
pub enum AiProvider {
    #[serde(rename = "openai")]
    Openai,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "google")]
    Google,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "custom")]
    Custom,
}

/// PURPOSE: A user-configured AI provider integration
/// NOTE: RLS enforced at DB level via app.current_user_id session var
/// CONSTRAINTS: api_key_encrypted is stored as base64(nonce + ciphertext)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiConfiguration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: AiProvider,
    pub api_key_encrypted: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub max_tokens: i32,
    pub temperature: f32,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Response struct that masks the API key for safe client-side display
#[derive(Debug, Serialize)]
pub struct AiConfigurationResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: AiProvider,
    pub api_key_masked: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub max_tokens: i32,
    pub temperature: f32,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAiConfigRequest {
    pub provider: AiProvider,
    pub api_key: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiConfigRequest {
    pub api_key: Option<String>,
    pub model_name: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub active: Option<bool>,
}

/// PURPOSE: Request body for the summarize endpoint
#[derive(Debug, Deserialize)]
pub struct SummarizeRequest {
    pub email_text: String,
}

// Added: Request body for thread/conversation summarization (TMAIL-103)
/// PURPOSE: Summarize an entire email thread by fetching multiple messages
/// CONSTRAINTS: uids must contain at least 2 message UIDs for a meaningful thread summary
#[derive(Debug, Deserialize)]
pub struct ThreadSummaryRequest {
    pub folder: String,
    pub uids: Vec<u32>,
}

// Added: Smart reply tone options for TMAIL-104
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SmartReplyTone {
    Brief,
    Detailed,
    Decline,
}

/// PURPOSE: Request body for the smart reply endpoint (TMAIL-104)
/// CONSTRAINTS: folder + uid identify the email via IMAP; tone controls reply style
#[derive(Debug, Deserialize)]
pub struct SmartReplyRequest {
    pub folder: String,
    pub uid: u32,
    pub tone: SmartReplyTone,
}

/// PURPOSE: Response body for the smart reply endpoint (TMAIL-104)
#[derive(Debug, Serialize)]
pub struct SmartReplyResponse {
    pub reply: String,
    pub tone: SmartReplyTone,
    pub provider: AiProvider,
    pub model: String,
}

// Added: Compose email request/response types for AI draft generation (TMAIL-134)
/// PURPOSE: Request body for AI compose (full draft generation) endpoint
/// CONSTRAINTS: prompt is required; tone/length have known valid values
#[derive(Debug, Deserialize)]
pub struct ComposeEmailRequest {
    pub prompt: String,
    pub context: Option<String>,
    pub tone: Option<String>,
    pub length: Option<String>,
}

/// PURPOSE: Response body for AI compose endpoint with generated subject and body
#[derive(Debug, Serialize)]
pub struct ComposeEmailResponse {
    pub subject: String,
    pub body: String,
    pub provider: AiProvider,
    pub model: String,
}

/// PURPOSE: Encrypt an API key using AES-256-GCM
/// CONSTRAINTS: encryption_key must be exactly 32 bytes
/// NOTE: Returns base64-encoded nonce (12 bytes) + ciphertext
pub fn encrypt_api_key(api_key: &str, encryption_key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new(encryption_key.into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, api_key.as_bytes())
        .map_err(|err| format!("Encryption failed: {}", err))?;

    // Added: Concatenate nonce + ciphertext and base64 encode
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &combined,
    ))
}

/// PURPOSE: Decrypt an API key from AES-256-GCM encrypted base64 string
/// CONSTRAINTS: encryption_key must be exactly 32 bytes
pub fn decrypt_api_key(encrypted: &str, encryption_key: &[u8; 32]) -> Result<String, String> {
    let combined = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        encrypted,
    )
    .map_err(|err| format!("Base64 decode failed: {}", err))?;

    if combined.len() < 12 {
        return Err("Encrypted data too short — expected at least 12 bytes for nonce".to_string());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(encryption_key.into());

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|err| format!("Decryption failed: {}", err))?;

    String::from_utf8(plaintext)
        .map_err(|err| format!("Decrypted key is not valid UTF-8: {}", err))
}

/// PURPOSE: Mask an API key for display (show first 4 and last 4 chars)
/// NOTE: Keys shorter than 10 chars are fully masked
pub fn mask_api_key(api_key: &str) -> String {
    if api_key.len() < 10 {
        return "*".repeat(api_key.len());
    }
    let prefix = &api_key[..4];
    let suffix = &api_key[api_key.len() - 4..];
    format!("{}...{}", prefix, suffix)
}

/// PURPOSE: Derive a 32-byte encryption key from the JWT secret
/// CONSTRAINTS: Uses SHA-256 hash of the secret as the key
pub fn derive_encryption_key(jwt_secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(jwt_secret.as_bytes());
    hasher.finalize().into()
}

impl AiConfiguration {
    /// PURPOSE: Convert to response struct with masked API key
    pub fn to_response(&self, encryption_key: &[u8; 32]) -> AiConfigurationResponse {
        // Added: Attempt to decrypt for masking; fallback to generic mask
        let api_key_masked = match decrypt_api_key(&self.api_key_encrypted, encryption_key) {
            Ok(key) => mask_api_key(&key),
            Err(_) => "****".to_string(),
        };

        AiConfigurationResponse {
            id: self.id,
            user_id: self.user_id,
            provider: self.provider.clone(),
            api_key_masked,
            model_name: self.model_name.clone(),
            base_url: self.base_url.clone(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            active: self.active,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// PURPOSE: List all AI configs for a user
    pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<AiConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, AiConfiguration>(
            "SELECT * FROM ai_configurations WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single AI config by ID and user
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<AiConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, AiConfiguration>(
            "SELECT * FROM ai_configurations WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Find the active AI config for a user (first active one)
    pub async fn find_active(pool: &PgPool, user_id: Uuid) -> Result<Option<AiConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, AiConfiguration>(
            "SELECT * FROM ai_configurations WHERE user_id = $1 AND active = true ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create a new AI config
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        provider: &AiProvider,
        api_key_encrypted: &str,
        model_name: &str,
        base_url: Option<&str>,
        max_tokens: i32,
        temperature: f32,
    ) -> Result<AiConfiguration, sqlx::Error> {
        sqlx::query_as::<_, AiConfiguration>(
            "INSERT INTO ai_configurations (user_id, provider, api_key_encrypted, model_name, base_url, max_tokens, temperature) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING *",
        )
        .bind(user_id)
        .bind(provider)
        .bind(api_key_encrypted)
        .bind(model_name)
        .bind(base_url)
        .bind(max_tokens)
        .bind(temperature)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing AI config
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        api_key_encrypted: Option<&str>,
        model_name: Option<&str>,
        base_url: Option<Option<&str>>,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        active: Option<bool>,
    ) -> Result<Option<AiConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, AiConfiguration>(
            "UPDATE ai_configurations SET \
                api_key_encrypted = COALESCE($3, api_key_encrypted), \
                model_name = COALESCE($4, model_name), \
                base_url = COALESCE($5, base_url), \
                max_tokens = COALESCE($6, max_tokens), \
                temperature = COALESCE($7, temperature), \
                active = COALESCE($8, active), \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(api_key_encrypted)
        .bind(model_name)
        .bind(base_url.flatten())
        .bind(max_tokens)
        .bind(temperature)
        .bind(active)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete an AI config
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM ai_configurations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_provider_serialization() {
        // NOTE: Verify enum values match the PostgreSQL ENUM names
        let provider = AiProvider::Openai;
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json, "openai");

        let provider = AiProvider::Anthropic;
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json, "anthropic");

        let provider = AiProvider::Google;
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json, "google");

        let provider = AiProvider::Ollama;
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json, "ollama");

        let provider = AiProvider::Custom;
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json, "custom");
    }

    #[test]
    fn test_ai_provider_deserialization() {
        let provider: AiProvider = serde_json::from_str("\"openai\"").unwrap();
        assert_eq!(provider, AiProvider::Openai);

        let provider: AiProvider = serde_json::from_str("\"anthropic\"").unwrap();
        assert_eq!(provider, AiProvider::Anthropic);

        let provider: AiProvider = serde_json::from_str("\"google\"").unwrap();
        assert_eq!(provider, AiProvider::Google);

        let provider: AiProvider = serde_json::from_str("\"ollama\"").unwrap();
        assert_eq!(provider, AiProvider::Ollama);

        let provider: AiProvider = serde_json::from_str("\"custom\"").unwrap();
        assert_eq!(provider, AiProvider::Custom);
    }

    #[test]
    fn test_ai_provider_roundtrip() {
        let providers = vec![
            AiProvider::Openai,
            AiProvider::Anthropic,
            AiProvider::Google,
            AiProvider::Ollama,
            AiProvider::Custom,
        ];
        let json = serde_json::to_string(&providers).unwrap();
        let deserialized: Vec<AiProvider> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, providers);
    }

    #[test]
    fn test_ai_provider_invalid_deserialization() {
        let result = serde_json::from_str::<AiProvider>("\"unknown_provider\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_encryption_key("test-secret-key-for-testing");
        let original = "sk-1234567890abcdef";

        let encrypted = encrypt_api_key(original, &key).unwrap();
        // Added: Encrypted string should differ from original
        assert_ne!(encrypted, original);

        let decrypted = decrypt_api_key(&encrypted, &key).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertexts() {
        // Added: Each encryption should use a random nonce, producing different output
        let key = derive_encryption_key("test-secret");
        let api_key = "sk-test-key-12345";

        let encrypted_a = encrypt_api_key(api_key, &key).unwrap();
        let encrypted_b = encrypt_api_key(api_key, &key).unwrap();
        assert_ne!(encrypted_a, encrypted_b);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let key_a = derive_encryption_key("secret-a");
        let key_b = derive_encryption_key("secret-b");
        let api_key = "sk-test-key";

        let encrypted = encrypt_api_key(api_key, &key_a).unwrap();
        let result = decrypt_api_key(&encrypted, &key_b);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_with_short_data_fails() {
        let key = derive_encryption_key("test-secret");
        let result = decrypt_api_key("c2hvcnQ=", &key); // base64("short") = 5 bytes
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too short"));
    }

    #[test]
    fn test_decrypt_with_invalid_base64_fails() {
        let key = derive_encryption_key("test-secret");
        let result = decrypt_api_key("not-valid-base64!!!", &key);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Base64 decode failed"));
    }

    #[test]
    fn test_mask_api_key_long() {
        assert_eq!(mask_api_key("sk-1234567890abcdef"), "sk-1...cdef");
    }

    #[test]
    fn test_mask_api_key_short() {
        // NOTE: Keys shorter than 10 chars are fully masked
        assert_eq!(mask_api_key("short"), "*****");
        assert_eq!(mask_api_key("123456789"), "*********");
    }

    #[test]
    fn test_mask_api_key_exactly_10() {
        assert_eq!(mask_api_key("1234567890"), "1234...7890");
    }

    #[test]
    fn test_derive_encryption_key_deterministic() {
        let key_a = derive_encryption_key("my-secret");
        let key_b = derive_encryption_key("my-secret");
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn test_derive_encryption_key_different_secrets() {
        let key_a = derive_encryption_key("secret-one");
        let key_b = derive_encryption_key("secret-two");
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn test_create_request_deserialization() {
        let json = serde_json::json!({
            "provider": "openai",
            "api_key": "sk-abc123",
            "model_name": "gpt-4o",
            "base_url": null,
            "max_tokens": 1000,
            "temperature": 0.5
        });

        let request: CreateAiConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.provider, AiProvider::Openai);
        assert_eq!(request.api_key, "sk-abc123");
        assert_eq!(request.model_name, "gpt-4o");
        assert!(request.base_url.is_none());
        assert_eq!(request.max_tokens, Some(1000));
        assert_eq!(request.temperature, Some(0.5));
    }

    #[test]
    fn test_create_request_minimal() {
        let json = serde_json::json!({
            "provider": "ollama",
            "api_key": "",
            "model_name": "llama3"
        });

        let request: CreateAiConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.provider, AiProvider::Ollama);
        assert_eq!(request.model_name, "llama3");
        assert!(request.base_url.is_none());
        assert!(request.max_tokens.is_none());
    }

    #[test]
    fn test_create_request_missing_required_field_fails() {
        let json = serde_json::json!({
            "provider": "openai"
        });
        let result = serde_json::from_value::<CreateAiConfigRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_request_partial() {
        let json = serde_json::json!({
            "active": false
        });

        let update: UpdateAiConfigRequest = serde_json::from_value(json).unwrap();
        assert!(update.api_key.is_none());
        assert!(update.model_name.is_none());
        assert_eq!(update.active, Some(false));
    }

    #[test]
    fn test_update_request_empty() {
        let json = serde_json::json!({});

        let update: UpdateAiConfigRequest = serde_json::from_value(json).unwrap();
        assert!(update.api_key.is_none());
        assert!(update.model_name.is_none());
        assert!(update.active.is_none());
        assert!(update.temperature.is_none());
    }

    #[test]
    fn test_summarize_request_deserialization() {
        let json = serde_json::json!({
            "email_text": "Hello, this is a test email about the quarterly report."
        });

        let request: SummarizeRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.email_text, "Hello, this is a test email about the quarterly report.");
    }

    // Added: Smart reply request/response tests for TMAIL-104
    #[test]
    fn test_smart_reply_tone_serialization() {
        let brief = SmartReplyTone::Brief;
        let json = serde_json::to_value(&brief).unwrap();
        assert_eq!(json, "brief");

        let detailed = SmartReplyTone::Detailed;
        let json = serde_json::to_value(&detailed).unwrap();
        assert_eq!(json, "detailed");

        let decline = SmartReplyTone::Decline;
        let json = serde_json::to_value(&decline).unwrap();
        assert_eq!(json, "decline");
    }

    #[test]
    fn test_smart_reply_tone_deserialization() {
        let tone: SmartReplyTone = serde_json::from_str("\"brief\"").unwrap();
        assert_eq!(tone, SmartReplyTone::Brief);

        let tone: SmartReplyTone = serde_json::from_str("\"detailed\"").unwrap();
        assert_eq!(tone, SmartReplyTone::Detailed);

        let tone: SmartReplyTone = serde_json::from_str("\"decline\"").unwrap();
        assert_eq!(tone, SmartReplyTone::Decline);
    }

    #[test]
    fn test_smart_reply_tone_invalid_deserialization() {
        let result = serde_json::from_str::<SmartReplyTone>("\"casual\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_smart_reply_request_deserialization() {
        let json = serde_json::json!({
            "folder": "INBOX",
            "uid": 42,
            "tone": "brief"
        });
        let request: SmartReplyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.folder, "INBOX");
        assert_eq!(request.uid, 42);
        assert_eq!(request.tone, SmartReplyTone::Brief);
    }

    #[test]
    fn test_smart_reply_request_missing_tone_fails() {
        let json = serde_json::json!({
            "folder": "INBOX",
            "uid": 42
        });
        let result = serde_json::from_value::<SmartReplyRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_smart_reply_response_serialization() {
        let response = SmartReplyResponse {
            reply: "Thank you for your email.".to_string(),
            tone: SmartReplyTone::Brief,
            provider: AiProvider::Openai,
            model: "gpt-4o".to_string(),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["reply"], "Thank you for your email.");
        assert_eq!(json["tone"], "brief");
        assert_eq!(json["provider"], "openai");
        assert_eq!(json["model"], "gpt-4o");
    }

    // Added: ComposeEmailRequest deserialization tests for TMAIL-134
    #[test]
    fn test_compose_email_request_full_deserialization() {
        let json = serde_json::json!({
            "prompt": "Write a follow-up email about the project deadline",
            "context": "We discussed moving the deadline to next Friday",
            "tone": "professional",
            "length": "medium"
        });
        let request: ComposeEmailRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.prompt, "Write a follow-up email about the project deadline");
        assert_eq!(request.context.as_deref(), Some("We discussed moving the deadline to next Friday"));
        assert_eq!(request.tone.as_deref(), Some("professional"));
        assert_eq!(request.length.as_deref(), Some("medium"));
    }

    #[test]
    fn test_compose_email_request_minimal_deserialization() {
        let json = serde_json::json!({
            "prompt": "Ask about meeting time"
        });
        let request: ComposeEmailRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.prompt, "Ask about meeting time");
        assert!(request.context.is_none());
        assert!(request.tone.is_none());
        assert!(request.length.is_none());
    }

    #[test]
    fn test_compose_email_request_missing_prompt_fails() {
        let json = serde_json::json!({
            "tone": "casual"
        });
        let result = serde_json::from_value::<ComposeEmailRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_compose_email_response_serialization() {
        let response = ComposeEmailResponse {
            subject: "Meeting Follow-Up".to_string(),
            body: "Hi team,\n\nJust following up on our discussion.".to_string(),
            provider: AiProvider::Openai,
            model: "gpt-4o".to_string(),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["subject"], "Meeting Follow-Up");
        assert_eq!(json["body"], "Hi team,\n\nJust following up on our discussion.");
        assert_eq!(json["provider"], "openai");
        assert_eq!(json["model"], "gpt-4o");
    }

    #[test]
    fn test_ai_configuration_response_serialization() {
        let response = AiConfigurationResponse {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            provider: AiProvider::Openai,
            api_key_masked: "sk-1...cdef".to_string(),
            model_name: "gpt-4o".to_string(),
            base_url: None,
            max_tokens: 500,
            temperature: 0.7,
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["provider"], "openai");
        assert_eq!(json["api_key_masked"], "sk-1...cdef");
        assert_eq!(json["model_name"], "gpt-4o");
        assert_eq!(json["max_tokens"], 500);
        assert_eq!(json["active"], true);
        // NOTE: Encrypted key should never appear in the response
        assert!(json.get("api_key_encrypted").is_none());
    }
}
