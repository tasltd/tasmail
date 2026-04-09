use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Contact {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub company: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateContact {
    pub email: String,
    pub display_name: Option<String>,
    pub company: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContact {
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub company: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
}

impl Contact {
    pub async fn find_by_mailbox(pool: &PgPool, mailbox_id: Uuid) -> Result<Vec<Contact>, sqlx::Error> {
        sqlx::query_as::<_, Contact>(
            "SELECT * FROM contacts WHERE mailbox_id = $1 ORDER BY display_name ASC, email ASC"
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<Option<Contact>, sqlx::Error> {
        sqlx::query_as::<_, Contact>(
            "SELECT * FROM contacts WHERE id = $1 AND mailbox_id = $2"
        )
        .bind(id)
        .bind(mailbox_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn search(pool: &PgPool, mailbox_id: Uuid, query: &str) -> Result<Vec<Contact>, sqlx::Error> {
        let pattern = format!("%{}%", query);
        sqlx::query_as::<_, Contact>(
            "SELECT * FROM contacts WHERE mailbox_id = $1 AND (email ILIKE $2 OR display_name ILIKE $2 OR company ILIKE $2) ORDER BY display_name ASC LIMIT 50"
        )
        .bind(mailbox_id)
        .bind(&pattern)
        .fetch_all(pool)
        .await
    }

    pub async fn create(pool: &PgPool, mailbox_id: Uuid, input: &CreateContact) -> Result<Contact, sqlx::Error> {
        sqlx::query_as::<_, Contact>(
            "INSERT INTO contacts (mailbox_id, email, display_name, company, phone, notes) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
        )
        .bind(mailbox_id)
        .bind(&input.email)
        .bind(&input.display_name)
        .bind(&input.company)
        .bind(&input.phone)
        .bind(&input.notes)
        .fetch_one(pool)
        .await
    }

    pub async fn update(pool: &PgPool, id: Uuid, mailbox_id: Uuid, input: &UpdateContact) -> Result<Option<Contact>, sqlx::Error> {
        sqlx::query_as::<_, Contact>(
            "UPDATE contacts SET
                email = COALESCE($3, email),
                display_name = COALESCE($4, display_name),
                company = COALESCE($5, company),
                phone = COALESCE($6, phone),
                notes = COALESCE($7, notes),
                updated_at = NOW()
            WHERE id = $1 AND mailbox_id = $2 RETURNING *"
        )
        .bind(id)
        .bind(mailbox_id)
        .bind(&input.email)
        .bind(&input.display_name)
        .bind(&input.company)
        .bind(&input.phone)
        .bind(&input.notes)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM contacts WHERE id = $1 AND mailbox_id = $2")
            .bind(id)
            .bind(mailbox_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
