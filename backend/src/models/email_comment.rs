// Added: Email comment model for TMAIL-128 — internal comments on emails
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents an internal comment on an email message
/// CONSTRAINTS: Comments are scoped to (mailbox_id, folder, message_uid)
/// EXTERNAL: PostgreSQL with RLS enforcing mailbox isolation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailComment {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub message_uid: i32,
    pub folder: String,
    pub content: String,
    pub author_name: String,
    pub author_email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateComment {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateComment {
    pub content: String,
}

impl EmailComment {
    /// Added: Create a new comment on a message
    pub async fn create(
        pool: &PgPool,
        mailbox_id: Uuid,
        message_uid: i32,
        folder: &str,
        content: &str,
        author_name: &str,
        author_email: &str,
    ) -> Result<EmailComment, sqlx::Error> {
        sqlx::query_as::<_, EmailComment>(
            "INSERT INTO email_comments (mailbox_id, message_uid, folder, content, author_name, author_email)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(message_uid)
        .bind(folder)
        .bind(content)
        .bind(author_name)
        .bind(author_email)
        .fetch_one(pool)
        .await
    }

    /// Added: List all comments for a specific message, ordered by creation time
    pub async fn list_for_message(
        pool: &PgPool,
        mailbox_id: Uuid,
        folder: &str,
        message_uid: i32,
    ) -> Result<Vec<EmailComment>, sqlx::Error> {
        sqlx::query_as::<_, EmailComment>(
            "SELECT * FROM email_comments
             WHERE mailbox_id = $1 AND folder = $2 AND message_uid = $3
             ORDER BY created_at ASC"
        )
        .bind(mailbox_id)
        .bind(folder)
        .bind(message_uid)
        .fetch_all(pool)
        .await
    }

    /// Added: Update a comment's content (only owner can update, enforced by RLS + mailbox_id check)
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        mailbox_id: Uuid,
        content: &str,
    ) -> Result<Option<EmailComment>, sqlx::Error> {
        sqlx::query_as::<_, EmailComment>(
            "UPDATE email_comments SET content = $3, updated_at = NOW()
             WHERE id = $1 AND mailbox_id = $2
             RETURNING *"
        )
        .bind(id)
        .bind(mailbox_id)
        .bind(content)
        .fetch_optional(pool)
        .await
    }

    /// Added: Delete a comment (only owner can delete, enforced by RLS + mailbox_id check)
    pub async fn delete(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM email_comments WHERE id = $1 AND mailbox_id = $2"
        )
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
    fn test_email_comment_serialization() {
        let id = Uuid::new_v4();
        let mailbox_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let comment = EmailComment {
            id,
            mailbox_id,
            message_uid: 42,
            folder: "INBOX".to_string(),
            content: "Need to follow up on this".to_string(),
            author_name: "Kwame Mensah".to_string(),
            author_email: "kwame@example.com".to_string(),
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(&comment).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["mailbox_id"], mailbox_id.to_string());
        assert_eq!(json["message_uid"], 42);
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["content"], "Need to follow up on this");
        assert_eq!(json["author_name"], "Kwame Mensah");
        assert_eq!(json["author_email"], "kwame@example.com");
    }

    #[test]
    fn test_email_comment_roundtrip() {
        let comment = EmailComment {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            message_uid: 7,
            folder: "Sent".to_string(),
            content: "Client confirmed receipt".to_string(),
            author_name: "Ama Adjei".to_string(),
            author_email: "ama@example.com".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&comment).unwrap();
        let deserialized: EmailComment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, comment.id);
        assert_eq!(deserialized.message_uid, 7);
        assert_eq!(deserialized.folder, "Sent");
        assert_eq!(deserialized.content, "Client confirmed receipt");
    }

    #[test]
    fn test_create_comment_deserialization() {
        let json = serde_json::json!({
            "content": "This needs review before reply"
        });

        let create: CreateComment = serde_json::from_value(json).unwrap();
        assert_eq!(create.content, "This needs review before reply");
    }

    #[test]
    fn test_create_comment_missing_content_fails() {
        let json = serde_json::json!({});
        let result = serde_json::from_value::<CreateComment>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_comment_deserialization() {
        let json = serde_json::json!({
            "content": "Updated note: client responded"
        });

        let update: UpdateComment = serde_json::from_value(json).unwrap();
        assert_eq!(update.content, "Updated note: client responded");
    }
}
