// Added: WebAuthn credential model for TMAIL-83 passkey support
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// PURPOSE: Represents a stored WebAuthn/FIDO2 credential for passkey authentication
/// CONSTRAINTS: credential_id must be unique across all users (enforced by DB)
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebauthnCredential {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub credential_id: String,
    pub public_key: Vec<u8>,
    pub sign_count: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// PURPOSE: Serializable credential info returned to the frontend (excludes raw public_key)
#[derive(Debug, Serialize)]
pub struct WebauthnCredentialInfo {
    pub id: Uuid,
    pub credential_id: String,
    pub name: String,
    pub sign_count: i64,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl From<WebauthnCredential> for WebauthnCredentialInfo {
    fn from(cred: WebauthnCredential) -> Self {
        Self {
            id: cred.id,
            credential_id: cred.credential_id,
            name: cred.name,
            sign_count: cred.sign_count,
            created_at: cred.created_at,
            last_used_at: cred.last_used_at,
        }
    }
}

impl WebauthnCredential {
    /// PURPOSE: Store a new WebAuthn credential after successful registration
    pub async fn create(
        pool: &PgPool,
        mailbox_id: Uuid,
        credential_id: &str,
        public_key: &[u8],
        name: &str,
    ) -> Result<Self, AppError> {
        let row = sqlx::query_as::<_, Self>(
            r#"INSERT INTO webauthn_credentials (mailbox_id, credential_id, public_key, name)
               VALUES ($1, $2, $3, $4)
               RETURNING *"#,
        )
        .bind(mailbox_id)
        .bind(credential_id)
        .bind(public_key)
        .bind(name)
        .fetch_one(pool)
        .await?;

        Ok(row)
    }

    /// PURPOSE: List all credentials for a given mailbox
    pub async fn list_by_mailbox(
        pool: &PgPool,
        mailbox_id: Uuid,
    ) -> Result<Vec<Self>, AppError> {
        let rows = sqlx::query_as::<_, Self>(
            "SELECT * FROM webauthn_credentials WHERE mailbox_id = $1 ORDER BY created_at DESC",
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// PURPOSE: Look up a credential by its WebAuthn credential_id (used during authentication)
    pub async fn find_by_credential_id(
        pool: &PgPool,
        credential_id: &str,
    ) -> Result<Option<Self>, AppError> {
        let row = sqlx::query_as::<_, Self>(
            "SELECT * FROM webauthn_credentials WHERE credential_id = $1",
        )
        .bind(credential_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// PURPOSE: Increment sign_count and update last_used_at after successful authentication
    pub async fn update_sign_count(
        pool: &PgPool,
        id: Uuid,
        new_count: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE webauthn_credentials SET sign_count = $1, last_used_at = NOW() WHERE id = $2",
        )
        .bind(new_count)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// PURPOSE: Delete a credential (user removing a passkey)
    /// CONSTRAINTS: Requires both id and mailbox_id to prevent cross-user deletion
    pub async fn delete(
        pool: &PgPool,
        id: Uuid,
        mailbox_id: Uuid,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "DELETE FROM webauthn_credentials WHERE id = $1 AND mailbox_id = $2",
        )
        .bind(id)
        .bind(mailbox_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_info_from_credential() {
        let cred = WebauthnCredential {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            credential_id: "test-cred-id-abc123".to_string(),
            public_key: vec![1, 2, 3, 4],
            sign_count: 5,
            name: "MacBook fingerprint".to_string(),
            created_at: Utc::now(),
            last_used_at: Some(Utc::now()),
        };

        let info: WebauthnCredentialInfo = cred.into();
        assert_eq!(info.credential_id, "test-cred-id-abc123");
        assert_eq!(info.name, "MacBook fingerprint");
        assert_eq!(info.sign_count, 5);
        assert!(info.last_used_at.is_some());
    }

    #[test]
    fn test_credential_info_serialization() {
        let info = WebauthnCredentialInfo {
            id: Uuid::new_v4(),
            credential_id: "cred-xyz".to_string(),
            name: "YubiKey".to_string(),
            sign_count: 0,
            created_at: Utc::now(),
            last_used_at: None,
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["credential_id"], "cred-xyz");
        assert_eq!(json["name"], "YubiKey");
        assert_eq!(json["sign_count"], 0);
        assert!(json["last_used_at"].is_null());
    }

    #[test]
    fn test_credential_info_excludes_public_key() {
        // NOTE: WebauthnCredentialInfo should not expose raw public_key bytes
        let info = WebauthnCredentialInfo {
            id: Uuid::new_v4(),
            credential_id: "cred-test".to_string(),
            name: "Test Key".to_string(),
            sign_count: 10,
            created_at: Utc::now(),
            last_used_at: None,
        };

        let json = serde_json::to_value(&info).unwrap();
        assert!(json.get("public_key").is_none());
    }
}
