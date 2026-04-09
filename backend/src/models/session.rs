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

    /// Clean up expired sessions
    pub async fn cleanup_expired(pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}
