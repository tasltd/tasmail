use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduledEmail {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub bcc_addresses: Vec<String>,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub scheduled_at: DateTime<Utc>,
    pub status: String,
    pub cancel_token: Uuid,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateScheduledEmail {
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub delay_seconds: Option<i64>,
}

impl ScheduledEmail {
    /// Create a new scheduled email
    pub async fn create(
        pool: &PgPool,
        mailbox_id: Uuid,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        text_body: Option<&str>,
        html_body: Option<&str>,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduledEmail, sqlx::Error> {
        sqlx::query_as::<_, ScheduledEmail>(
            "INSERT INTO scheduled_emails (mailbox_id, to_addresses, cc_addresses, bcc_addresses, subject, text_body, html_body, scheduled_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(to)
        .bind(cc)
        .bind(bcc)
        .bind(subject)
        .bind(text_body)
        .bind(html_body)
        .bind(scheduled_at)
        .fetch_one(pool)
        .await
    }

    /// Cancel a scheduled email by cancel_token (used for undo-send)
    pub async fn cancel_by_token(
        pool: &PgPool,
        cancel_token: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE scheduled_emails SET status = 'cancelled', cancelled_at = NOW()
             WHERE cancel_token = $1 AND status = 'pending'"
        )
        .bind(cancel_token)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get all pending emails ready to send
    pub async fn find_ready_to_send(
        pool: &PgPool,
    ) -> Result<Vec<ScheduledEmail>, sqlx::Error> {
        sqlx::query_as::<_, ScheduledEmail>(
            "SELECT * FROM scheduled_emails
             WHERE status = 'pending' AND scheduled_at <= NOW()
             ORDER BY scheduled_at ASC
             LIMIT 50"
        )
        .fetch_all(pool)
        .await
    }

    /// Mark as sent
    pub async fn mark_sent(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE scheduled_emails SET status = 'sent', sent_at = NOW() WHERE id = $1"
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark as failed
    pub async fn mark_failed(pool: &PgPool, id: Uuid, error: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE scheduled_emails SET status = 'failed' WHERE id = $1"
        )
        .bind(id)
        .execute(pool)
        .await?;
        // NOTE: error details could be stored in a separate column or logged
        tracing::error!("Scheduled email {} failed: {}", id, error);
        Ok(result.rows_affected() > 0)
    }

    /// List scheduled emails for a mailbox
    pub async fn list_for_mailbox(
        pool: &PgPool,
        mailbox_id: Uuid,
        status: Option<&str>,
    ) -> Result<Vec<ScheduledEmail>, sqlx::Error> {
        if let Some(status) = status {
            sqlx::query_as::<_, ScheduledEmail>(
                "SELECT * FROM scheduled_emails WHERE mailbox_id = $1 AND status = $2 ORDER BY scheduled_at ASC"
            )
            .bind(mailbox_id)
            .bind(status)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, ScheduledEmail>(
                "SELECT * FROM scheduled_emails WHERE mailbox_id = $1 ORDER BY scheduled_at DESC LIMIT 100"
            )
            .bind(mailbox_id)
            .fetch_all(pool)
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_email_serialization() {
        let email = ScheduledEmail {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            to_addresses: vec!["user@example.com".to_string()],
            cc_addresses: vec![],
            bcc_addresses: vec![],
            subject: "Test Subject".to_string(),
            text_body: Some("Hello".to_string()),
            html_body: None,
            scheduled_at: Utc::now() + chrono::Duration::seconds(30),
            status: "pending".to_string(),
            cancel_token: Uuid::new_v4(),
            created_at: Utc::now(),
            sent_at: None,
            cancelled_at: None,
        };

        let json = serde_json::to_string(&email).unwrap();
        assert!(json.contains("Test Subject"));
        assert!(json.contains("pending"));
        assert!(json.contains("user@example.com"));

        let deserialized: ScheduledEmail = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.subject, "Test Subject");
        assert_eq!(deserialized.to_addresses.len(), 1);
    }

    #[test]
    fn test_create_scheduled_email_request_deserialization() {
        let json = r#"{
            "to": ["a@b.com"],
            "subject": "Hi",
            "delay_seconds": 10
        }"#;
        let req: CreateScheduledEmail = serde_json::from_str(json).unwrap();
        assert_eq!(req.to, vec!["a@b.com"]);
        assert_eq!(req.delay_seconds, Some(10));
        assert!(req.scheduled_at.is_none());
    }

    #[test]
    fn test_create_scheduled_email_with_schedule() {
        let json = r#"{
            "to": ["a@b.com", "c@d.com"],
            "cc": ["e@f.com"],
            "subject": "Meeting",
            "text_body": "See you there",
            "scheduled_at": "2026-04-15T10:00:00Z"
        }"#;
        let req: CreateScheduledEmail = serde_json::from_str(json).unwrap();
        assert_eq!(req.to.len(), 2);
        assert!(req.scheduled_at.is_some());
        assert!(req.delay_seconds.is_none());
    }
}
