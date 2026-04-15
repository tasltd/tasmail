// Added: CalDAV/CardDAV configuration model for TMAIL-117
// PURPOSE: Stores per-user DAV server configs with encrypted passwords for calendar/contact sync
// CONSTRAINTS: Password encrypted with AES-256-GCM (same pattern as ai_config and smtp_config)
// EXTERNAL: Uses aes-gcm crate via ai_config encryption helpers

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// NOTE: Re-use encryption utilities from ai_config module
use crate::models::ai_config::{decrypt_api_key, encrypt_api_key};

/// PURPOSE: DAV protocol type — CalDAV (calendars), CardDAV (contacts), or Both
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DavType {
    #[serde(rename = "caldav")]
    CalDav,
    #[serde(rename = "carddav")]
    CardDav,
    Both,
}

impl DavType {
    /// PURPOSE: Convert to database string representation
    pub fn as_str(&self) -> &str {
        match self {
            DavType::CalDav => "caldav",
            DavType::CardDav => "carddav",
            DavType::Both => "both",
        }
    }

    /// PURPOSE: Parse from database string representation
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "caldav" => Ok(DavType::CalDav),
            "carddav" => Ok(DavType::CardDav),
            "both" => Ok(DavType::Both),
            other => Err(format!("Unknown DAV type: {}", other)),
        }
    }
}

/// PURPOSE: Sync status for a DAV configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Idle,
    Syncing,
    Error,
}

impl SyncStatus {
    /// PURPOSE: Convert to database string representation
    pub fn as_str(&self) -> &str {
        match self {
            SyncStatus::Idle => "idle",
            SyncStatus::Syncing => "syncing",
            SyncStatus::Error => "error",
        }
    }

    /// PURPOSE: Parse from database string representation
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "idle" => Ok(SyncStatus::Idle),
            "syncing" => Ok(SyncStatus::Syncing),
            "error" => Ok(SyncStatus::Error),
            other => Err(format!("Unknown sync status: {}", other)),
        }
    }
}

/// PURPOSE: A user-configured CalDAV/CardDAV server connection
/// NOTE: RLS enforced at DB level via app.current_user_id session var
/// CONSTRAINTS: encrypted_password stored as base64(nonce + ciphertext) via AES-256-GCM
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DavConfiguration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub server_url: String,
    pub username: String,
    pub encrypted_password: String,
    pub dav_type: String,
    pub sync_interval_minutes: i32,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    pub sync_status: Option<String>,
    pub sync_error: Option<String>,
    pub enabled: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Response struct that masks the password for safe client-side display
#[derive(Debug, Serialize)]
pub struct DavConfigurationResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub server_url: String,
    pub username: String,
    pub password_masked: String,
    pub dav_type: String,
    pub sync_interval_minutes: i32,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    pub sync_status: Option<String>,
    pub sync_error: Option<String>,
    pub enabled: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Request body for creating a new DAV configuration
#[derive(Debug, Deserialize)]
pub struct CreateDavConfigRequest {
    pub name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub dav_type: DavType,
    pub sync_interval_minutes: Option<i32>,
    pub enabled: Option<bool>,
}

/// PURPOSE: Request body for updating an existing DAV configuration
#[derive(Debug, Deserialize)]
pub struct UpdateDavConfigRequest {
    pub name: Option<String>,
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub dav_type: Option<DavType>,
    pub sync_interval_minutes: Option<i32>,
    pub enabled: Option<bool>,
}

/// PURPOSE: Result of a DAV server connection test
#[derive(Debug, Serialize)]
pub struct DavTestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: u64,
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

impl DavConfiguration {
    /// PURPOSE: Convert to response struct with masked password
    pub fn to_response(&self, encryption_key: &[u8; 32]) -> DavConfigurationResponse {
        // Added: Attempt to decrypt for masking; fallback to generic mask
        let password_masked = match decrypt_api_key(&self.encrypted_password, encryption_key) {
            Ok(pw) => mask_password(&pw),
            Err(_) => "****".to_string(),
        };

        DavConfigurationResponse {
            id: self.id,
            user_id: self.user_id,
            name: self.name.clone(),
            server_url: self.server_url.clone(),
            username: self.username.clone(),
            password_masked,
            dav_type: self.dav_type.clone(),
            sync_interval_minutes: self.sync_interval_minutes,
            last_sync_at: self.last_sync_at,
            sync_status: self.sync_status.clone(),
            sync_error: self.sync_error.clone(),
            enabled: self.enabled,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// PURPOSE: List all DAV configs for a user
    pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<DavConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, DavConfiguration>(
            "SELECT * FROM dav_configurations WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single DAV config by ID and user
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<DavConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, DavConfiguration>(
            "SELECT * FROM dav_configurations WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create a new DAV config
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        name: &str,
        server_url: &str,
        username: &str,
        encrypted_password: &str,
        dav_type: &str,
        sync_interval_minutes: i32,
        enabled: bool,
    ) -> Result<DavConfiguration, sqlx::Error> {
        sqlx::query_as::<_, DavConfiguration>(
            "INSERT INTO dav_configurations (user_id, name, server_url, username, encrypted_password, dav_type, sync_interval_minutes, enabled) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             RETURNING *",
        )
        .bind(user_id)
        .bind(name)
        .bind(server_url)
        .bind(username)
        .bind(encrypted_password)
        .bind(dav_type)
        .bind(sync_interval_minutes)
        .bind(enabled)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing DAV config
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        name: Option<&str>,
        server_url: Option<&str>,
        username: Option<&str>,
        encrypted_password: Option<&str>,
        dav_type: Option<&str>,
        sync_interval_minutes: Option<i32>,
        enabled: Option<bool>,
    ) -> Result<Option<DavConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, DavConfiguration>(
            "UPDATE dav_configurations SET \
                name = COALESCE($3, name), \
                server_url = COALESCE($4, server_url), \
                username = COALESCE($5, username), \
                encrypted_password = COALESCE($6, encrypted_password), \
                dav_type = COALESCE($7, dav_type), \
                sync_interval_minutes = COALESCE($8, sync_interval_minutes), \
                enabled = COALESCE($9, enabled), \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(name)
        .bind(server_url)
        .bind(username)
        .bind(encrypted_password)
        .bind(dav_type)
        .bind(sync_interval_minutes)
        .bind(enabled)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete a DAV config
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM dav_configurations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Update sync status and error message after a sync attempt
    pub async fn update_sync_status(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<Option<DavConfiguration>, sqlx::Error> {
        sqlx::query_as::<_, DavConfiguration>(
            "UPDATE dav_configurations SET \
                sync_status = $3, \
                sync_error = $4, \
                last_sync_at = CASE WHEN $3 = 'idle' THEN NOW() ELSE last_sync_at END, \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(status)
        .bind(error)
        .fetch_optional(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ai_config::{derive_encryption_key, encrypt_api_key};

    #[test]
    fn test_dav_type_serialization() {
        let dav = DavType::CalDav;
        let json = serde_json::to_value(&dav).unwrap();
        assert_eq!(json, "caldav");

        let dav = DavType::CardDav;
        let json = serde_json::to_value(&dav).unwrap();
        assert_eq!(json, "carddav");

        let dav = DavType::Both;
        let json = serde_json::to_value(&dav).unwrap();
        assert_eq!(json, "both");
    }

    #[test]
    fn test_dav_type_deserialization() {
        let dav: DavType = serde_json::from_str("\"caldav\"").unwrap();
        assert_eq!(dav, DavType::CalDav);

        let dav: DavType = serde_json::from_str("\"carddav\"").unwrap();
        assert_eq!(dav, DavType::CardDav);

        let dav: DavType = serde_json::from_str("\"both\"").unwrap();
        assert_eq!(dav, DavType::Both);
    }

    #[test]
    fn test_dav_type_invalid_deserialization() {
        let result = serde_json::from_str::<DavType>("\"webdav\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_dav_type_as_str() {
        assert_eq!(DavType::CalDav.as_str(), "caldav");
        assert_eq!(DavType::CardDav.as_str(), "carddav");
        assert_eq!(DavType::Both.as_str(), "both");
    }

    #[test]
    fn test_dav_type_from_str() {
        assert_eq!(DavType::from_str("caldav").unwrap(), DavType::CalDav);
        assert_eq!(DavType::from_str("carddav").unwrap(), DavType::CardDav);
        assert_eq!(DavType::from_str("both").unwrap(), DavType::Both);
        assert!(DavType::from_str("invalid").is_err());
    }

    #[test]
    fn test_sync_status_serialization() {
        let status = SyncStatus::Idle;
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json, "idle");

        let status = SyncStatus::Syncing;
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json, "syncing");

        let status = SyncStatus::Error;
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json, "error");
    }

    #[test]
    fn test_sync_status_from_str() {
        assert_eq!(SyncStatus::from_str("idle").unwrap(), SyncStatus::Idle);
        assert_eq!(SyncStatus::from_str("syncing").unwrap(), SyncStatus::Syncing);
        assert_eq!(SyncStatus::from_str("error").unwrap(), SyncStatus::Error);
        assert!(SyncStatus::from_str("unknown").is_err());
    }

    #[test]
    fn test_mask_password_long() {
        assert_eq!(mask_password("my-dav-password"), "my...rd");
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
            "name": "Radicale Server",
            "server_url": "https://radicale.example.com",
            "username": "user@example.com",
            "password": "my-dav-password",
            "dav_type": "both",
            "sync_interval_minutes": 30,
            "enabled": true
        });

        let request: CreateDavConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Radicale Server");
        assert_eq!(request.server_url, "https://radicale.example.com");
        assert_eq!(request.username, "user@example.com");
        assert_eq!(request.password, "my-dav-password");
        assert_eq!(request.dav_type, DavType::Both);
        assert_eq!(request.sync_interval_minutes, Some(30));
        assert_eq!(request.enabled, Some(true));
    }

    #[test]
    fn test_create_request_deserialization_minimal() {
        let json = serde_json::json!({
            "name": "My CalDAV",
            "server_url": "https://cal.example.com",
            "username": "user",
            "password": "secret",
            "dav_type": "caldav"
        });

        let request: CreateDavConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "My CalDAV");
        assert_eq!(request.dav_type, DavType::CalDav);
        assert!(request.sync_interval_minutes.is_none());
        assert!(request.enabled.is_none());
    }

    #[test]
    fn test_create_request_missing_required_field_fails() {
        let json = serde_json::json!({
            "name": "Incomplete",
            "server_url": "https://cal.example.com"
        });
        let result = serde_json::from_value::<CreateDavConfigRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_request_partial() {
        let json = serde_json::json!({
            "server_url": "https://new-server.example.com",
            "dav_type": "carddav",
            "sync_interval_minutes": 120
        });

        let update: UpdateDavConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(update.server_url.as_deref(), Some("https://new-server.example.com"));
        assert_eq!(update.dav_type, Some(DavType::CardDav));
        assert_eq!(update.sync_interval_minutes, Some(120));
        assert!(update.name.is_none());
        assert!(update.username.is_none());
        assert!(update.password.is_none());
    }

    #[test]
    fn test_update_request_empty() {
        let json = serde_json::json!({});

        let update: UpdateDavConfigRequest = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert!(update.server_url.is_none());
        assert!(update.username.is_none());
        assert!(update.password.is_none());
        assert!(update.dav_type.is_none());
        assert!(update.sync_interval_minutes.is_none());
        assert!(update.enabled.is_none());
    }

    #[test]
    fn test_dav_config_response_serialization() {
        let response = DavConfigurationResponse {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Radicale".to_string(),
            server_url: "https://radicale.example.com".to_string(),
            username: "user@example.com".to_string(),
            password_masked: "my...rd".to_string(),
            dav_type: "both".to_string(),
            sync_interval_minutes: 60,
            last_sync_at: Some(chrono::Utc::now()),
            sync_status: Some("idle".to_string()),
            sync_error: None,
            enabled: true,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["name"], "Radicale");
        assert_eq!(json["server_url"], "https://radicale.example.com");
        assert_eq!(json["password_masked"], "my...rd");
        assert_eq!(json["dav_type"], "both");
        assert_eq!(json["sync_interval_minutes"], 60);
        assert_eq!(json["enabled"], true);
        // NOTE: encrypted_password should never appear in the response
        assert!(json.get("encrypted_password").is_none());
    }

    #[test]
    fn test_to_response_with_valid_encryption() {
        let key = derive_encryption_key("test-secret-for-dav");
        let password = "my-dav-password";
        let encrypted = encrypt_api_key(password, &key).unwrap();

        let config = DavConfiguration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Test DAV".to_string(),
            server_url: "https://dav.test.com".to_string(),
            username: "test@test.com".to_string(),
            encrypted_password: encrypted,
            dav_type: "caldav".to_string(),
            sync_interval_minutes: 60,
            last_sync_at: None,
            sync_status: Some("idle".to_string()),
            sync_error: None,
            enabled: true,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let response = config.to_response(&key);
        assert_eq!(response.password_masked, "my...rd");
        assert_eq!(response.name, "Test DAV");
    }

    #[test]
    fn test_to_response_with_invalid_encryption_key() {
        // Added: Wrong key should produce generic mask
        let key_a = derive_encryption_key("key-a");
        let key_b = derive_encryption_key("key-b");
        let encrypted = encrypt_api_key("secret", &key_a).unwrap();

        let config = DavConfiguration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Bad Key Test".to_string(),
            server_url: "https://dav.test.com".to_string(),
            username: "test@test.com".to_string(),
            encrypted_password: encrypted,
            dav_type: "both".to_string(),
            sync_interval_minutes: 60,
            last_sync_at: None,
            sync_status: Some("idle".to_string()),
            sync_error: None,
            enabled: true,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let response = config.to_response(&key_b);
        assert_eq!(response.password_masked, "****");
    }

    #[test]
    fn test_dav_test_result_serialization() {
        let result = DavTestResult {
            success: true,
            message: "Connection successful".to_string(),
            latency_ms: 150,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["message"], "Connection successful");
        assert_eq!(json["latency_ms"], 150);
    }

    #[test]
    fn test_dav_type_roundtrip() {
        let values = vec![DavType::CalDav, DavType::CardDav, DavType::Both];
        let json = serde_json::to_string(&values).unwrap();
        let deserialized: Vec<DavType> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, values);
    }
}
