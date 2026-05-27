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

    // Added: TMAIL-119 — auto-collect helper. Insert if new, leave existing rows alone so we
    // don't overwrite a hand-curated display name with whatever the user typed in the To: field.
    pub async fn upsert_from_send(
        pool: &PgPool,
        mailbox_id: Uuid,
        email: &str,
        display_name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO contacts (mailbox_id, email, display_name) VALUES ($1, $2, $3)
             ON CONFLICT (mailbox_id, email) DO NOTHING",
        )
        .bind(mailbox_id)
        .bind(email)
        .bind(display_name)
        .execute(pool)
        .await?;
        Ok(())
    }
}

// Added: TMAIL-119 — parse "Display Name <email@host>" or bare "email@host" into (name, email).
// Returns None if the input has no recognisable address part. Used by auto-collect and CSV import.
pub fn parse_recipient(input: &str) -> Option<(Option<String>, String)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let (Some(lt), Some(gt)) = (trimmed.rfind('<'), trimmed.rfind('>')) {
        if gt > lt {
            let email = trimmed[lt + 1..gt].trim();
            if email.contains('@') && !email.contains(' ') {
                let name = trimmed[..lt].trim().trim_matches('"').trim();
                let name = if name.is_empty() { None } else { Some(name.to_string()) };
                return Some((name, email.to_string()));
            }
        }
    }
    if trimmed.contains('@') && !trimmed.contains(' ') {
        return Some((None, trimmed.to_string()));
    }
    None
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

    // Added: TMAIL-119 — parse_recipient covers the formats auto-collect and CSV import see.
    #[test]
    fn test_parse_recipient_bare_email() {
        let (name, email) = super::parse_recipient("alice@example.com").unwrap();
        assert!(name.is_none());
        assert_eq!(email, "alice@example.com");
    }

    #[test]
    fn test_parse_recipient_named() {
        let (name, email) = super::parse_recipient("Alice Smith <alice@example.com>").unwrap();
        assert_eq!(name.as_deref(), Some("Alice Smith"));
        assert_eq!(email, "alice@example.com");
    }

    #[test]
    fn test_parse_recipient_quoted_name() {
        let (name, email) = super::parse_recipient("\"Alice, Smith\" <alice@example.com>").unwrap();
        assert_eq!(name.as_deref(), Some("Alice, Smith"));
        assert_eq!(email, "alice@example.com");
    }

    #[test]
    fn test_parse_recipient_whitespace() {
        let (name, email) = super::parse_recipient("   bob@example.com   ").unwrap();
        assert!(name.is_none());
        assert_eq!(email, "bob@example.com");
    }

    #[test]
    fn test_parse_recipient_empty() {
        assert!(super::parse_recipient("").is_none());
        assert!(super::parse_recipient("   ").is_none());
    }

    #[test]
    fn test_parse_recipient_no_at_sign() {
        assert!(super::parse_recipient("not an email").is_none());
        assert!(super::parse_recipient("Alice <not an email>").is_none());
    }

    #[test]
    fn test_parse_recipient_empty_display_name() {
        // "  <foo@bar>" should yield None for name, valid email
        let (name, email) = super::parse_recipient(" <foo@bar.com>").unwrap();
        assert!(name.is_none());
        assert_eq!(email, "foo@bar.com");
    }
}
