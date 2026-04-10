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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contact_serialization() {
        let id = Uuid::new_v4();
        let mailbox_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let contact = Contact {
            id,
            mailbox_id,
            email: "alice@example.com".to_string(),
            display_name: Some("Alice".to_string()),
            company: Some("Acme Corp".to_string()),
            phone: Some("+233201234567".to_string()),
            notes: Some("VIP client".to_string()),
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(&contact).unwrap();
        assert_eq!(json["email"], "alice@example.com");
        assert_eq!(json["display_name"], "Alice");
        assert_eq!(json["company"], "Acme Corp");
        assert_eq!(json["phone"], "+233201234567");
        assert_eq!(json["notes"], "VIP client");
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["mailbox_id"], mailbox_id.to_string());
    }

    #[test]
    fn test_contact_serialization_with_nulls() {
        let contact = Contact {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            email: "bob@example.com".to_string(),
            display_name: None,
            company: None,
            phone: None,
            notes: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&contact).unwrap();
        assert_eq!(json["email"], "bob@example.com");
        assert!(json["display_name"].is_null());
        assert!(json["company"].is_null());
        assert!(json["phone"].is_null());
        assert!(json["notes"].is_null());
    }

    #[test]
    fn test_create_contact_deserialization() {
        let json = serde_json::json!({
            "email": "charlie@example.com",
            "display_name": "Charlie",
            "company": "TechCo",
            "phone": "+233551234567",
            "notes": "New lead"
        });

        let create: CreateContact = serde_json::from_value(json).unwrap();
        assert_eq!(create.email, "charlie@example.com");
        assert_eq!(create.display_name.unwrap(), "Charlie");
        assert_eq!(create.company.unwrap(), "TechCo");
        assert_eq!(create.phone.unwrap(), "+233551234567");
        assert_eq!(create.notes.unwrap(), "New lead");
    }

    #[test]
    fn test_create_contact_deserialization_minimal() {
        let json = serde_json::json!({
            "email": "minimal@example.com"
        });

        let create: CreateContact = serde_json::from_value(json).unwrap();
        assert_eq!(create.email, "minimal@example.com");
        assert!(create.display_name.is_none());
        assert!(create.company.is_none());
        assert!(create.phone.is_none());
        assert!(create.notes.is_none());
    }

    #[test]
    fn test_update_contact_deserialization() {
        let json = serde_json::json!({
            "email": "updated@example.com",
            "display_name": "Updated Name"
        });

        let update: UpdateContact = serde_json::from_value(json).unwrap();
        assert_eq!(update.email.unwrap(), "updated@example.com");
        assert_eq!(update.display_name.unwrap(), "Updated Name");
        assert!(update.company.is_none());
        assert!(update.phone.is_none());
        assert!(update.notes.is_none());
    }

    #[test]
    fn test_update_contact_deserialization_empty() {
        let json = serde_json::json!({});

        let update: UpdateContact = serde_json::from_value(json).unwrap();
        assert!(update.email.is_none());
        assert!(update.display_name.is_none());
        assert!(update.company.is_none());
        assert!(update.phone.is_none());
        assert!(update.notes.is_none());
    }
}
