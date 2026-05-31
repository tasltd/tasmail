use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub refresh_token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl Session {
    pub async fn create(
        pool: &sqlx::PgPool,
        mailbox_id: Uuid,
        refresh_token_hash: &str,
        expires_at: DateTime<Utc>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<Session, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            "INSERT INTO sessions (id, mailbox_id, refresh_token_hash, expires_at, created_at, ip_address, user_agent)
             VALUES ($1, $2, $3, $4, NOW(), $5, $6)
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(mailbox_id)
        .bind(refresh_token_hash)
        .bind(expires_at)
        .bind(ip_address)
        .bind(user_agent)
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_token_hash(
        pool: &sqlx::PgPool,
        token_hash: &str,
    ) -> Result<Option<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE refresh_token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_all_for_mailbox(
        pool: &sqlx::PgPool,
        mailbox_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE mailbox_id = $1")
            .bind(mailbox_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Added (TMAIL-377): list every active (unexpired) refresh-token row for
    /// `mailbox_id`, newest-first by `created_at`. Used by the Classic UI
    /// sessions page (P1 #23) to render "SPA / mobile refresh tokens" rows
    /// next to the live classic browsers. Filtering by `expires_at > NOW()`
    /// hides rows the cleanup sweep hasn't reaped yet so the user only sees
    /// what could currently authenticate as them.
    pub async fn list_active_for_mailbox(
        pool: &sqlx::PgPool,
        mailbox_id: Uuid,
    ) -> Result<Vec<Session>, sqlx::Error> {
        sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions
              WHERE mailbox_id = $1 AND expires_at > NOW()
              ORDER BY created_at DESC",
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    /// Added (TMAIL-377): delete a single refresh-token row scoped to the
    /// owning mailbox. Used by the per-row "Revoke this session" button on
    /// `/classic/settings/sessions`. The `mailbox_id` filter makes the
    /// query safe with attacker-supplied ids — a hostile user can't revoke
    /// another mailbox's row because the WHERE clause silently no-ops on a
    /// mismatched owner.
    ///
    /// Returns true when a row was deleted, false otherwise (already
    /// expired / wrong owner / never existed).
    pub async fn delete_for_mailbox(
        pool: &sqlx::PgPool,
        mailbox_id: Uuid,
        session_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM sessions WHERE mailbox_id = $1 AND id = $2",
        )
        .bind(mailbox_id)
        .bind(session_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_serialization() {
        let id = Uuid::new_v4();
        let mailbox_id = Uuid::new_v4();
        let now = Utc::now();
        let expires = now + chrono::Duration::hours(24);

        let session = Session {
            id,
            mailbox_id,
            refresh_token_hash: "sha256:abc123def456".to_string(),
            expires_at: expires,
            created_at: now,
            ip_address: Some("10.0.0.1".to_string()),
            user_agent: Some("TASMail/1.0".to_string()),
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["mailbox_id"], mailbox_id.to_string());
        assert_eq!(json["refresh_token_hash"], "sha256:abc123def456");
        assert_eq!(json["ip_address"], "10.0.0.1");
        assert_eq!(json["user_agent"], "TASMail/1.0");
    }

    #[test]
    fn test_session_serialization_with_nulls() {
        let session = Session {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            refresh_token_hash: "sha256:xyz".to_string(),
            expires_at: Utc::now(),
            created_at: Utc::now(),
            ip_address: None,
            user_agent: None,
        };

        let json = serde_json::to_value(&session).unwrap();
        assert!(json["ip_address"].is_null());
        assert!(json["user_agent"].is_null());
    }

    #[test]
    fn test_session_roundtrip() {
        let session = Session {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            refresh_token_hash: "sha256:roundtrip".to_string(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            created_at: Utc::now(),
            ip_address: Some("192.168.1.100".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, session.id);
        assert_eq!(deserialized.mailbox_id, session.mailbox_id);
        assert_eq!(deserialized.refresh_token_hash, "sha256:roundtrip");
        assert_eq!(deserialized.ip_address.unwrap(), "192.168.1.100");
    }
}
