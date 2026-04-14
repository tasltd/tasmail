// Added: POP3 configuration model for Dovecot POP3 access (TMAIL-133)
// PURPOSE: Stores per-user POP3 access settings managed via the web UI
// CONSTRAINTS: One config per user (user_id UNIQUE), RLS enforced at DB level

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: A user's POP3 access configuration for Dovecot
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Pop3Configuration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub enabled: bool,
    pub delete_after_download: bool,
    pub retention_days: Option<i32>,
    pub last_pop3_login: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Request body for creating or updating POP3 configuration
#[derive(Debug, Deserialize)]
pub struct UpdatePop3ConfigRequest {
    pub enabled: Option<bool>,
    pub delete_after_download: Option<bool>,
    pub retention_days: Option<Option<i32>>,
}

/// PURPOSE: POP3 connection status info for mail client setup
#[derive(Debug, Serialize)]
pub struct Pop3Status {
    pub server: String,
    pub port: u16,
    pub encryption: String,
    pub username_format: String,
}

impl Pop3Configuration {
    /// PURPOSE: Get POP3 config for a specific user
    pub async fn get_by_user(pool: &PgPool, user_id: Uuid) -> Result<Option<Pop3Configuration>, sqlx::Error> {
        sqlx::query_as::<_, Pop3Configuration>(
            "SELECT * FROM pop3_configurations WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create or update POP3 config (upsert on user_id)
    pub async fn create_or_update(
        pool: &PgPool,
        user_id: Uuid,
        enabled: bool,
        delete_after_download: bool,
        retention_days: Option<i32>,
    ) -> Result<Pop3Configuration, sqlx::Error> {
        sqlx::query_as::<_, Pop3Configuration>(
            "INSERT INTO pop3_configurations (user_id, enabled, delete_after_download, retention_days) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id) DO UPDATE SET \
                enabled = EXCLUDED.enabled, \
                delete_after_download = EXCLUDED.delete_after_download, \
                retention_days = EXCLUDED.retention_days, \
                updated_at = NOW() \
             RETURNING *",
        )
        .bind(user_id)
        .bind(enabled)
        .bind(delete_after_download)
        .bind(retention_days)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Delete POP3 config for a user
    pub async fn delete_by_user(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM pop3_configurations WHERE user_id = $1")
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
    fn test_update_request_full_deserialization() {
        let json = serde_json::json!({
            "enabled": true,
            "delete_after_download": true,
            "retention_days": 30
        });

        let request: UpdatePop3ConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.enabled, Some(true));
        assert_eq!(request.delete_after_download, Some(true));
        // NOTE: retention_days is Option<Option<i32>>; outer Some means field present
        assert!(request.retention_days.is_some());
    }

    #[test]
    fn test_update_request_minimal_deserialization() {
        let json = serde_json::json!({});

        let request: UpdatePop3ConfigRequest = serde_json::from_value(json).unwrap();
        assert!(request.enabled.is_none());
        assert!(request.delete_after_download.is_none());
        assert!(request.retention_days.is_none());
    }

    #[test]
    fn test_update_request_enabled_only() {
        let json = serde_json::json!({
            "enabled": false
        });

        let request: UpdatePop3ConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.enabled, Some(false));
        assert!(request.delete_after_download.is_none());
        assert!(request.retention_days.is_none());
    }

    #[test]
    fn test_update_request_retention_null() {
        // NOTE: Explicit null in JSON deserializes to None for Option<Option<i32>>
        // To distinguish "absent" from "null", use serde's double_option or custom deser
        let json = serde_json::json!({
            "retention_days": null
        });

        let request: UpdatePop3ConfigRequest = serde_json::from_value(json).unwrap();
        // NOTE: serde treats JSON null as None for Option<Option<T>> by default
        assert!(request.retention_days.is_none());
    }

    #[test]
    fn test_pop3_configuration_serialization() {
        let config = Pop3Configuration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            enabled: true,
            delete_after_download: false,
            retention_days: Some(30),
            last_pop3_login: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["delete_after_download"], false);
        assert_eq!(json["retention_days"], 30);
        assert!(json["last_pop3_login"].is_null());
    }

    #[test]
    fn test_pop3_configuration_serialization_no_retention() {
        let config = Pop3Configuration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            enabled: false,
            delete_after_download: false,
            retention_days: None,
            last_pop3_login: None,
            created_at: None,
            updated_at: None,
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], false);
        assert!(json["retention_days"].is_null());
        assert!(json["created_at"].is_null());
    }

    #[test]
    fn test_pop3_status_serialization() {
        let status = Pop3Status {
            server: "mail.example.com".to_string(),
            port: 995,
            encryption: "SSL/TLS".to_string(),
            username_format: "user@example.com".to_string(),
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["server"], "mail.example.com");
        assert_eq!(json["port"], 995);
        assert_eq!(json["encryption"], "SSL/TLS");
        assert_eq!(json["username_format"], "user@example.com");
    }

    #[test]
    fn test_pop3_configuration_deserialization() {
        let json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "user_id": "00000000-0000-0000-0000-000000000002",
            "enabled": true,
            "delete_after_download": true,
            "retention_days": 7,
            "last_pop3_login": null,
            "created_at": "2026-04-14T00:00:00Z",
            "updated_at": "2026-04-14T00:00:00Z"
        });

        let config: Pop3Configuration = serde_json::from_value(json).unwrap();
        assert!(config.enabled);
        assert!(config.delete_after_download);
        assert_eq!(config.retention_days, Some(7));
        assert!(config.last_pop3_login.is_none());
    }
}
