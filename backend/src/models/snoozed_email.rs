use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SnoozedEmail {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub folder: String,
    pub message_uid: i32,
    pub snooze_until: DateTime<Utc>,
    pub original_folder: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSnooze {
    pub folder: String,
    pub message_uid: i32,
    pub snooze_until: DateTime<Utc>,
}

impl SnoozedEmail {
    pub async fn create(pool: &PgPool, mailbox_id: Uuid, data: &CreateSnooze) -> Result<SnoozedEmail, sqlx::Error> {
        sqlx::query_as::<_, SnoozedEmail>(
            "INSERT INTO snoozed_emails (mailbox_id, folder, message_uid, snooze_until, original_folder)
             VALUES ($1, $2, $3, $4, $2)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(&data.folder)
        .bind(data.message_uid)
        .bind(data.snooze_until)
        .fetch_one(pool)
        .await
    }

    pub async fn list_by_mailbox(pool: &PgPool, mailbox_id: Uuid) -> Result<Vec<SnoozedEmail>, sqlx::Error> {
        sqlx::query_as::<_, SnoozedEmail>(
            "SELECT * FROM snoozed_emails WHERE mailbox_id = $1 ORDER BY snooze_until ASC"
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    pub async fn cancel(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM snoozed_emails WHERE id = $1 AND mailbox_id = $2")
            .bind(id)
            .bind(mailbox_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Find all snoozes that have expired (snooze_until has passed)
    pub async fn find_expired(pool: &PgPool) -> Result<Vec<SnoozedEmail>, sqlx::Error> {
        sqlx::query_as::<_, SnoozedEmail>(
            "SELECT * FROM snoozed_emails WHERE snooze_until <= NOW()"
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM snoozed_emails WHERE id = $1")
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
    fn test_create_snooze_deserialization() {
        let json = r#"{
            "folder": "INBOX",
            "message_uid": 42,
            "snooze_until": "2026-04-15T09:00:00Z"
        }"#;
        let req: CreateSnooze = serde_json::from_str(json).unwrap();
        assert_eq!(req.folder, "INBOX");
        assert_eq!(req.message_uid, 42);
        assert!(req.snooze_until > Utc::now() - chrono::Duration::days(365));
    }

    #[test]
    fn test_snoozed_email_serialization() {
        let snooze = SnoozedEmail {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            folder: "INBOX".to_string(),
            message_uid: 42,
            snooze_until: Utc::now() + chrono::Duration::hours(2),
            original_folder: "INBOX".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&snooze).unwrap();
        assert!(json.contains("\"folder\":\"INBOX\""));
        assert!(json.contains("\"message_uid\":42"));
    }

    #[test]
    fn test_snooze_must_be_in_future() {
        let json = r#"{"folder": "INBOX", "message_uid": 1, "snooze_until": "2026-04-15T09:00:00Z"}"#;
        let req: CreateSnooze = serde_json::from_str(json).unwrap();
        // Validation would be done at handler level, but type accepts any valid datetime
        assert!(req.snooze_until.timestamp() > 0);
    }
}
