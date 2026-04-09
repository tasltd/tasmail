use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Signature {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub name: String,
    pub html_body: String,
    pub text_body: String,
    pub is_default: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSignature {
    pub name: String,
    pub html_body: String,
    pub text_body: String,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSignature {
    pub name: Option<String>,
    pub html_body: Option<String>,
    pub text_body: Option<String>,
    pub is_default: Option<bool>,
}

impl Signature {
    pub async fn find_by_mailbox(pool: &PgPool, mailbox_id: Uuid) -> Result<Vec<Signature>, sqlx::Error> {
        sqlx::query_as::<_, Signature>(
            "SELECT * FROM signatures WHERE mailbox_id = $1 ORDER BY is_default DESC, name ASC"
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<Option<Signature>, sqlx::Error> {
        sqlx::query_as::<_, Signature>(
            "SELECT * FROM signatures WHERE id = $1 AND mailbox_id = $2"
        )
        .bind(id)
        .bind(mailbox_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_default(pool: &PgPool, mailbox_id: Uuid) -> Result<Option<Signature>, sqlx::Error> {
        sqlx::query_as::<_, Signature>(
            "SELECT * FROM signatures WHERE mailbox_id = $1 AND is_default = true LIMIT 1"
        )
        .bind(mailbox_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn create(pool: &PgPool, mailbox_id: Uuid, input: &CreateSignature) -> Result<Signature, sqlx::Error> {
        let is_default = input.is_default.unwrap_or(false);

        // If setting as default, unset other defaults first
        if is_default {
            sqlx::query("UPDATE signatures SET is_default = false WHERE mailbox_id = $1")
                .bind(mailbox_id)
                .execute(pool)
                .await?;
        }

        sqlx::query_as::<_, Signature>(
            "INSERT INTO signatures (mailbox_id, name, html_body, text_body, is_default) VALUES ($1, $2, $3, $4, $5) RETURNING *"
        )
        .bind(mailbox_id)
        .bind(&input.name)
        .bind(&input.html_body)
        .bind(&input.text_body)
        .bind(is_default)
        .fetch_one(pool)
        .await
    }

    pub async fn update(pool: &PgPool, id: Uuid, mailbox_id: Uuid, input: &UpdateSignature) -> Result<Option<Signature>, sqlx::Error> {
        // If setting as default, unset other defaults first
        if input.is_default == Some(true) {
            sqlx::query("UPDATE signatures SET is_default = false WHERE mailbox_id = $1 AND id != $2")
                .bind(mailbox_id)
                .bind(id)
                .execute(pool)
                .await?;
        }

        sqlx::query_as::<_, Signature>(
            "UPDATE signatures SET
                name = COALESCE($3, name),
                html_body = COALESCE($4, html_body),
                text_body = COALESCE($5, text_body),
                is_default = COALESCE($6, is_default),
                updated_at = NOW()
            WHERE id = $1 AND mailbox_id = $2 RETURNING *"
        )
        .bind(id)
        .bind(mailbox_id)
        .bind(&input.name)
        .bind(&input.html_body)
        .bind(&input.text_body)
        .bind(input.is_default)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM signatures WHERE id = $1 AND mailbox_id = $2")
            .bind(id)
            .bind(mailbox_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
