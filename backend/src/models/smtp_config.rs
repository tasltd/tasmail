// Added: SMTP configuration model for BYO-SMTP integration (TMAIL-48)
// PURPOSE: Stores per-user external SMTP server configs with encrypted passwords
// CONSTRAINTS: Password encrypted with AES-256-GCM (same pattern as ai_config)
// EXTERNAL: Uses aes-gcm crate for encryption, lettre for SMTP transport

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// NOTE: Re-use encryption utilities from ai_config module
use crate::models::ai_config::{decrypt_api_key, encrypt_api_key};

/// PURPOSE: SMTP encryption mode for the external SMTP server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SmtpEncryption {
    None,
    Ssl,
    #[serde(rename = "starttls")]
    StartTls,
}

impl SmtpEncryption {
    /// PURPOSE: Convert to database string representation
    pub fn as_str(&self) -> &str {
        match self {
            SmtpEncryption::None => "none",
            SmtpEncryption::Ssl => "ssl",
            SmtpEncryption::StartTls => "starttls",
        }
    }

    /// PURPOSE: Parse from database string representation
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "none" => Ok(SmtpEncryption::None),
            "ssl" => Ok(SmtpEncryption::Ssl),
            "starttls" => Ok(SmtpEncryption::StartTls),
            other => Err(format!("Unknown SMTP encryption type: {}", other)),
        }
    }
}

/// PURPOSE: A user-configured external SMTP server
/// NOTE: RLS enforced at DB level via app.current_user_id session var
/// CONSTRAINTS: encrypted_password stored as base64(nonce + ciphertext) via AES-256-GCM
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SmtpConfiguration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub encrypted_password: String,
    pub encryption: String,
    pub from_address: Option<String>,
    pub is_default: bool,
    pub verified: bool,
    pub last_tested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Response struct that masks the password for safe client-side display
#[derive(Debug, Serialize)]
pub struct SmtpConfigurationResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password_masked: String,
    pub encryption: String,
    pub from_address: Option<String>,
    pub is_default: bool,
    pub verified: bool,
    pub last_tested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Request body for creating a new SMTP configuration
#[derive(Debug, Deserialize)]
pub struct CreateSmtpConfigRequest {
    pub name: String,
    pub host: String,
    pub port: Option<i32>,
    pub username: String,
    pub password: String,
    pub encryption: Option<SmtpEncryption>,
    pub from_address: Option<String>,
}

/// PURPOSE: Request body for updating an existing SMTP configuration
#[derive(Debug, Deserialize)]
pub struct UpdateSmtpConfigRequest {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub encryption: Option<SmtpEncryption>,
    pub from_address: Option<String>,
}

/// PURPOSE: Mask a password for display (show first 2 and last 2 chars)
/// NOTE: Passwords shorter than 6 chars are fully masked
pub fn mask_password(password: &str) -> String {
    if password.len() < 6 {
        return "*".repeat(password.len());
    }
    let prefix = &password[..2];
    let suffix = &password[password.len() - 2..];
    format!("{}...{}", prefix, suffix)
}

impl SmtpConfiguration {
    /// PURPOSE: Convert to response struct with masked password
    pub fn to_response(&self, encryption_key: &[u8; 32]) -> SmtpConfigurationResponse {
        // Added: Attempt to decrypt for masking; fallback to generic mask
        let password_masked = match decrypt_api_key(&self.encrypted_password, encryption_key) {
            Ok(pw) => mask_password(&pw),
            Err(_) => "****".to_string(),
        };

        SmtpConfigurationResponse {
            id: self.id,
            user_id: self.user_id,
            name: self.name.clone(),
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            password_masked,
            encryption: self.encryption.clone(),
            from_address: self.from_address.clone(),
            is_default: self.is_default,
            verified: self.verified,
            last_tested_at: self.last_tested_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// PURPOSE: List all SMTP configs for a user
    pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<SmtpConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, SmtpConfiguration>(
            "SELECT * FROM smtp_configurations WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single SMTP config by ID and user
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<SmtpConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, SmtpConfiguration>(
            "SELECT * FROM smtp_configurations WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Find the default SMTP config for a user
    pub async fn find_default(pool: &PgPool, user_id: Uuid) -> Result<Option<SmtpConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, SmtpConfiguration>(
            "SELECT * FROM smtp_configurations WHERE user_id = $1 AND is_default = true LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// Added: Decrypt the stored SMTP password using the same AES-256-GCM key shared with ai_config.
    /// Returns the plaintext password ready to be passed to lettre's `Credentials`.
    pub fn decrypted_password(&self, key: &[u8; 32]) -> Result<String, String> {
        decrypt_api_key(&self.encrypted_password, key)
    }

    /// PURPOSE: Create a new SMTP config
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        name: &str,
        host: &str,
        port: i32,
        username: &str,
        encrypted_password: &str,
        encryption: &str,
        from_address: Option<&str>,
    ) -> Result<SmtpConfiguration, sqlx::Error> {
        sqlx::query_as::<_, SmtpConfiguration>(
            "INSERT INTO smtp_configurations (user_id, name, host, port, username, encrypted_password, encryption, from_address) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             RETURNING *",
        )
        .bind(user_id)
        .bind(name)
        .bind(host)
        .bind(port)
        .bind(username)
        .bind(encrypted_password)
        .bind(encryption)
        .bind(from_address)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing SMTP config
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        name: Option<&str>,
        host: Option<&str>,
        port: Option<i32>,
        username: Option<&str>,
        encrypted_password: Option<&str>,
        encryption: Option<&str>,
        from_address: Option<Option<&str>>,
    ) -> Result<Option<SmtpConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, SmtpConfiguration>(
            "UPDATE smtp_configurations SET \
                name = COALESCE($3, name), \
                host = COALESCE($4, host), \
                port = COALESCE($5, port), \
                username = COALESCE($6, username), \
                encrypted_password = COALESCE($7, encrypted_password), \
                encryption = COALESCE($8, encryption), \
                from_address = COALESCE($9, from_address), \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(name)
        .bind(host)
        .bind(port)
        .bind(username)
        .bind(encrypted_password)
        .bind(encryption)
        .bind(from_address.flatten())
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete an SMTP config
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM smtp_configurations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Set a config as the default (unsets any current default first)
    pub async fn set_default(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<Option<SmtpConfiguration>, sqlx::Error> {
        // Added: Unset all other defaults for this user first
        sqlx::query("UPDATE smtp_configurations SET is_default = false WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;

        // Added: Set the specified config as default
        sqlx::query_as::<_, SmtpConfiguration>(
            "UPDATE smtp_configurations SET is_default = true, updated_at = NOW() WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Update last_tested_at and verified status after a test
    pub async fn update_test_result(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        verified: bool,
    ) -> Result<Option<SmtpConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, SmtpConfiguration>(
            "UPDATE smtp_configurations SET verified = $3, last_tested_at = NOW(), updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(verified)
        .fetch_optional(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ai_config::derive_encryption_key;

    #[test]
    fn test_smtp_encryption_serialization() {
        let enc = SmtpEncryption::None;
        let json = serde_json::to_value(&enc).unwrap();
        assert_eq!(json, "none");

        let enc = SmtpEncryption::Ssl;
        let json = serde_json::to_value(&enc).unwrap();
        assert_eq!(json, "ssl");

        let enc = SmtpEncryption::StartTls;
        let json = serde_json::to_value(&enc).unwrap();
        assert_eq!(json, "starttls");
    }

    #[test]
    fn test_smtp_encryption_deserialization() {
        let enc: SmtpEncryption = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(enc, SmtpEncryption::None);

        let enc: SmtpEncryption = serde_json::from_str("\"ssl\"").unwrap();
        assert_eq!(enc, SmtpEncryption::Ssl);

        let enc: SmtpEncryption = serde_json::from_str("\"starttls\"").unwrap();
        assert_eq!(enc, SmtpEncryption::StartTls);
    }

    #[test]
    fn test_smtp_encryption_invalid_deserialization() {
        let result = serde_json::from_str::<SmtpEncryption>("\"plain\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_smtp_encryption_roundtrip() {
        let values = vec![SmtpEncryption::None, SmtpEncryption::Ssl, SmtpEncryption::StartTls];
        let json = serde_json::to_string(&values).unwrap();
        let deserialized: Vec<SmtpEncryption> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, values);
    }

    #[test]
    fn test_smtp_encryption_as_str() {
        assert_eq!(SmtpEncryption::None.as_str(), "none");
        assert_eq!(SmtpEncryption::Ssl.as_str(), "ssl");
        assert_eq!(SmtpEncryption::StartTls.as_str(), "starttls");
    }

    #[test]
    fn test_smtp_encryption_from_str() {
        assert_eq!(SmtpEncryption::from_str("none").unwrap(), SmtpEncryption::None);
        assert_eq!(SmtpEncryption::from_str("ssl").unwrap(), SmtpEncryption::Ssl);
        assert_eq!(SmtpEncryption::from_str("starttls").unwrap(), SmtpEncryption::StartTls);
        assert!(SmtpEncryption::from_str("invalid").is_err());
    }

    #[test]
    fn test_mask_password_long() {
        assert_eq!(mask_password("my-secret-password"), "my...rd");
    }

    #[test]
    fn test_mask_password_short() {
        // NOTE: Passwords shorter than 6 chars are fully masked
        assert_eq!(mask_password("pass"), "****");
        assert_eq!(mask_password("12345"), "*****");
    }

    #[test]
    fn test_mask_password_exactly_6() {
        assert_eq!(mask_password("abcdef"), "ab...ef");
    }

    #[test]
    fn test_create_request_deserialization_full() {
        let json = serde_json::json!({
            "name": "Gmail SMTP",
            "host": "smtp.gmail.com",
            "port": 465,
            "username": "user@gmail.com",
            "password": "app-password-123",
            "encryption": "ssl",
            "from_address": "user@gmail.com"
        });

        let request: CreateSmtpConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Gmail SMTP");
        assert_eq!(request.host, "smtp.gmail.com");
        assert_eq!(request.port, Some(465));
        assert_eq!(request.username, "user@gmail.com");
        assert_eq!(request.password, "app-password-123");
        assert_eq!(request.encryption, Some(SmtpEncryption::Ssl));
        assert_eq!(request.from_address.as_deref(), Some("user@gmail.com"));
    }

    #[test]
    fn test_create_request_deserialization_minimal() {
        let json = serde_json::json!({
            "name": "SendGrid",
            "host": "smtp.sendgrid.net",
            "username": "apikey",
            "password": "SG.xxxxx"
        });

        let request: CreateSmtpConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "SendGrid");
        assert_eq!(request.host, "smtp.sendgrid.net");
        assert!(request.port.is_none());
        assert!(request.encryption.is_none());
        assert!(request.from_address.is_none());
    }

    #[test]
    fn test_create_request_missing_required_field_fails() {
        let json = serde_json::json!({
            "name": "Incomplete",
            "host": "smtp.test.com"
        });
        let result = serde_json::from_value::<CreateSmtpConfigRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_request_partial() {
        let json = serde_json::json!({
            "host": "new-smtp.example.com",
            "port": 465,
            "encryption": "ssl"
        });

        let update: UpdateSmtpConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(update.host.as_deref(), Some("new-smtp.example.com"));
        assert_eq!(update.port, Some(465));
        assert_eq!(update.encryption, Some(SmtpEncryption::Ssl));
        assert!(update.name.is_none());
        assert!(update.username.is_none());
        assert!(update.password.is_none());
    }

    #[test]
    fn test_update_request_empty() {
        let json = serde_json::json!({});

        let update: UpdateSmtpConfigRequest = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert!(update.host.is_none());
        assert!(update.port.is_none());
        assert!(update.username.is_none());
        assert!(update.password.is_none());
        assert!(update.encryption.is_none());
        assert!(update.from_address.is_none());
    }

    #[test]
    fn test_smtp_config_response_serialization() {
        let response = SmtpConfigurationResponse {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Gmail".to_string(),
            host: "smtp.gmail.com".to_string(),
            port: 587,
            username: "user@gmail.com".to_string(),
            password_masked: "ap...rd".to_string(),
            encryption: "starttls".to_string(),
            from_address: Some("user@gmail.com".to_string()),
            is_default: true,
            verified: true,
            last_tested_at: Some(chrono::Utc::now()),
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["name"], "Gmail");
        assert_eq!(json["host"], "smtp.gmail.com");
        assert_eq!(json["port"], 587);
        assert_eq!(json["password_masked"], "ap...rd");
        assert_eq!(json["encryption"], "starttls");
        assert_eq!(json["is_default"], true);
        assert_eq!(json["verified"], true);
        // NOTE: encrypted_password should never appear in the response
        assert!(json.get("encrypted_password").is_none());
    }

    #[test]
    fn test_to_response_with_valid_encryption() {
        let key = derive_encryption_key("test-secret-for-smtp");
        let password = "my-smtp-password";
        let encrypted = encrypt_api_key(password, &key).unwrap();

        let config = SmtpConfiguration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Test SMTP".to_string(),
            host: "smtp.test.com".to_string(),
            port: 587,
            username: "test@test.com".to_string(),
            encrypted_password: encrypted,
            encryption: "starttls".to_string(),
            from_address: None,
            is_default: false,
            verified: false,
            last_tested_at: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let response = config.to_response(&key);
        assert_eq!(response.password_masked, "my...rd");
        assert_eq!(response.name, "Test SMTP");
    }

    #[test]
    fn test_to_response_with_invalid_encryption_key() {
        // Added: Wrong key should produce generic mask
        let key_a = derive_encryption_key("key-a");
        let key_b = derive_encryption_key("key-b");
        let encrypted = encrypt_api_key("secret", &key_a).unwrap();

        let config = SmtpConfiguration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Bad Key Test".to_string(),
            host: "smtp.test.com".to_string(),
            port: 587,
            username: "test@test.com".to_string(),
            encrypted_password: encrypted,
            encryption: "starttls".to_string(),
            from_address: None,
            is_default: false,
            verified: false,
            last_tested_at: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let response = config.to_response(&key_b);
        assert_eq!(response.password_masked, "****");
    }
}
