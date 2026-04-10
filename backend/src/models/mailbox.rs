use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Mailbox {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub quota_bytes: i64,
    pub quota_warn_percent: i32,
    pub active: bool,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Safe representation without password hash
#[derive(Debug, Clone, Serialize)]
pub struct MailboxInfo {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub quota_bytes: i64,
    pub quota_warn_percent: i32,
    pub active: bool,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Mailbox> for MailboxInfo {
    fn from(m: Mailbox) -> Self {
        MailboxInfo {
            id: m.id,
            domain_id: m.domain_id,
            username: m.username,
            display_name: m.display_name,
            quota_bytes: m.quota_bytes,
            quota_warn_percent: m.quota_warn_percent,
            active: m.active,
            is_admin: m.is_admin,
            created_at: m.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateMailbox {
    pub username: String,
    pub password: String,
    pub domain_id: Uuid,
    pub display_name: Option<String>,
    pub quota_bytes: Option<i64>,
}

impl Mailbox {
    pub async fn find_by_username(
        pool: &sqlx::PgPool,
        username: &str,
    ) -> Result<Option<Mailbox>, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>("SELECT * FROM mailboxes WHERE username = $1 AND active = true")
            .bind(username)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_id(
        pool: &sqlx::PgPool,
        id: Uuid,
    ) -> Result<Option<Mailbox>, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>("SELECT * FROM mailboxes WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_domain(
        pool: &sqlx::PgPool,
        domain_id: Uuid,
    ) -> Result<Vec<Mailbox>, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>(
            "SELECT * FROM mailboxes WHERE domain_id = $1 ORDER BY username",
        )
        .bind(domain_id)
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &sqlx::PgPool,
        username: &str,
        password_hash: &str,
        domain_id: Uuid,
        display_name: Option<&str>,
        quota_bytes: i64,
    ) -> Result<Mailbox, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>(
            "INSERT INTO mailboxes (id, domain_id, username, password_hash, display_name, quota_bytes, active, is_admin, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, true, false, NOW(), NOW())
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(domain_id)
        .bind(username)
        .bind(password_hash)
        .bind(display_name)
        .bind(quota_bytes)
        .fetch_one(pool)
        .await
    }

    pub async fn update_password(
        pool: &sqlx::PgPool,
        id: Uuid,
        password_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE mailboxes SET password_hash = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(password_hash)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
