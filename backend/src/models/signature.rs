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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_serialization() {
        let id = Uuid::new_v4();
        let mailbox_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let sig = Signature {
            id,
            mailbox_id,
            name: "Work Signature".to_string(),
            html_body: "<p>Best regards,<br>Kwame</p>".to_string(),
            text_body: "Best regards,\nKwame".to_string(),
            is_default: true,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(&sig).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["mailbox_id"], mailbox_id.to_string());
        assert_eq!(json["name"], "Work Signature");
        assert_eq!(json["html_body"], "<p>Best regards,<br>Kwame</p>");
        assert_eq!(json["text_body"], "Best regards,\nKwame");
        assert_eq!(json["is_default"], true);
    }

    #[test]
    fn test_signature_roundtrip() {
        let sig = Signature {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            name: "Personal".to_string(),
            html_body: "<i>Cheers</i>".to_string(),
            text_body: "Cheers".to_string(),
            is_default: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&sig).unwrap();
        let deserialized: Signature = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, sig.id);
        assert_eq!(deserialized.name, "Personal");
        assert_eq!(deserialized.is_default, false);
    }

    #[test]
    fn test_create_signature_deserialization() {
        let json = serde_json::json!({
            "name": "New Sig",
            "html_body": "<b>Hello</b>",
            "text_body": "Hello",
            "is_default": true
        });

        let create: CreateSignature = serde_json::from_value(json).unwrap();
        assert_eq!(create.name, "New Sig");
        assert_eq!(create.html_body, "<b>Hello</b>");
        assert_eq!(create.text_body, "Hello");
        assert_eq!(create.is_default.unwrap(), true);
    }

    #[test]
    fn test_create_signature_is_default_optional() {
        let json = serde_json::json!({
            "name": "No Default",
            "html_body": "<p>Hi</p>",
            "text_body": "Hi"
        });

        let create: CreateSignature = serde_json::from_value(json).unwrap();
        assert_eq!(create.name, "No Default");
        assert!(create.is_default.is_none());
    }

    #[test]
    fn test_create_signature_missing_required_field_fails() {
        let json = serde_json::json!({
            "name": "Incomplete"
        });
        let result = serde_json::from_value::<CreateSignature>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_signature_deserialization() {
        let json = serde_json::json!({
            "name": "Updated Sig",
            "is_default": false
        });

        let update: UpdateSignature = serde_json::from_value(json).unwrap();
        assert_eq!(update.name.unwrap(), "Updated Sig");
        assert!(update.html_body.is_none());
        assert!(update.text_body.is_none());
        assert_eq!(update.is_default.unwrap(), false);
    }

    #[test]
    fn test_update_signature_deserialization_empty() {
        let json = serde_json::json!({});

        let update: UpdateSignature = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert!(update.html_body.is_none());
        assert!(update.text_body.is_none());
        assert!(update.is_default.is_none());
    }
}
