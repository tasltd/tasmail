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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_serialization() {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let domain = Domain {
            id,
            name: "example.com".to_string(),
            active: true,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(&domain).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["name"], "example.com");
        assert_eq!(json["active"], true);
    }

    #[test]
    fn test_domain_serialization_inactive() {
        let domain = Domain {
            id: Uuid::new_v4(),
            name: "disabled.org".to_string(),
            active: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_value(&domain).unwrap();
        assert_eq!(json["name"], "disabled.org");
        assert_eq!(json["active"], false);
    }

    #[test]
    fn test_domain_roundtrip() {
        let domain = Domain {
            id: Uuid::new_v4(),
            name: "roundtrip.gh".to_string(),
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&domain).unwrap();
        let deserialized: Domain = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, domain.id);
        assert_eq!(deserialized.name, "roundtrip.gh");
        assert_eq!(deserialized.active, true);
    }

    #[test]
    fn test_create_domain_deserialization() {
        let json = serde_json::json!({
            "name": "newdomain.com"
        });

        let create: CreateDomain = serde_json::from_value(json).unwrap();
        assert_eq!(create.name, "newdomain.com");
    }

    #[test]
    fn test_create_domain_missing_name_fails() {
        let json = serde_json::json!({});
        let result = serde_json::from_value::<CreateDomain>(json);
        assert!(result.is_err());
    }
}
