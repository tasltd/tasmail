use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Domain {
    pub id: Uuid,
    pub name: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDomain {
    pub name: String,
}

impl Domain {
    pub async fn find_all(pool: &sqlx::PgPool) -> Result<Vec<Domain>, sqlx::Error> {
        sqlx::query_as::<_, Domain>("SELECT * FROM domains ORDER BY name")
            .fetch_all(pool)
            .await
    }

    pub async fn find_by_id(pool: &sqlx::PgPool, id: Uuid) -> Result<Option<Domain>, sqlx::Error> {
        sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_name(
        pool: &sqlx::PgPool,
        name: &str,
    ) -> Result<Option<Domain>, sqlx::Error> {
        sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE name = $1")
            .bind(name)
            .fetch_optional(pool)
            .await
    }

    pub async fn create(pool: &sqlx::PgPool, name: &str) -> Result<Domain, sqlx::Error> {
        sqlx::query_as::<_, Domain>(
            "INSERT INTO domains (id, name, active, created_at, updated_at)
             VALUES ($1, $2, true, NOW(), NOW())
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM domains WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
