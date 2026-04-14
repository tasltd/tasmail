// Added: Email queue model with retry logic for TMAIL-58
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a queued email with retry tracking
/// CONSTRAINTS: status must be one of: pending, sending, sent, failed, dead_letter
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailQueueItem {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub bcc_addresses: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub status: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub next_retry_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
}

/// PURPOSE: Aggregated queue statistics by status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub pending: i64,
    pub sending: i64,
    pub sent: i64,
    pub failed: i64,
    pub dead_letter: i64,
}

/// PURPOSE: Helper row for counting items per status
#[derive(Debug, sqlx::FromRow)]
struct StatusCount {
    status: String,
    count: i64,
}

/// Added: Base delay in seconds for exponential backoff (30s * 2^retry_count)
const BASE_RETRY_DELAY_SECS: i64 = 30;

impl EmailQueueItem {
    /// PURPOSE: Enqueue a new email for sending
    pub async fn enqueue(
        pool: &PgPool,
        mailbox_id: Uuid,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        body_html: &str,
        body_text: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, EmailQueueItem>(
            "INSERT INTO email_queue (mailbox_id, to_addresses, cc_addresses, bcc_addresses, subject, body_html, body_text)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(to)
        .bind(cc)
        .bind(bcc)
        .bind(subject)
        .bind(body_html)
        .bind(body_text)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Fetch items ready to send (pending or failed with next_retry_at <= NOW)
    /// CONSTRAINTS: Returns at most `limit` items ordered by next_retry_at ASC
    pub async fn fetch_ready(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, EmailQueueItem>(
            "SELECT * FROM email_queue
             WHERE status IN ('pending', 'failed') AND next_retry_at <= NOW()
             ORDER BY next_retry_at ASC
             LIMIT $1"
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Mark an item as currently being sent (optimistic lock)
    pub async fn mark_sending(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE email_queue SET status = 'sending' WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// PURPOSE: Mark an item as successfully sent
    pub async fn mark_sent(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE email_queue SET status = 'sent', sent_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// PURPOSE: Mark an item as failed and schedule next retry with exponential backoff
    /// CONSTRAINTS: next_retry_at = NOW() + base_delay * 2^retry_count
    pub async fn mark_failed(
        pool: &PgPool,
        id: Uuid,
        error: &str,
        retry_count: i32,
    ) -> Result<(), sqlx::Error> {
        // Added: Exponential backoff calculation (30s, 60s, 120s, 240s, 480s)
        let backoff_secs = BASE_RETRY_DELAY_SECS * 2_i64.pow(retry_count as u32);
        sqlx::query(
            "UPDATE email_queue
             SET status = 'failed', last_error = $2, retry_count = $3,
                 next_retry_at = NOW() + make_interval(secs => $4::double precision),
                 failed_at = NOW()
             WHERE id = $1"
        )
        .bind(id)
        .bind(error)
        .bind(retry_count)
        .bind(backoff_secs as f64)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Move item to dead_letter when max retries exceeded
    pub async fn mark_dead_letter(
        pool: &PgPool,
        id: Uuid,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE email_queue SET status = 'dead_letter', last_error = $2, failed_at = NOW() WHERE id = $1"
        )
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: List queue items for a specific mailbox, optionally filtered by status
    pub async fn list_by_mailbox(
        pool: &PgPool,
        mailbox_id: Uuid,
        status: Option<&str>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        if let Some(status) = status {
            sqlx::query_as::<_, EmailQueueItem>(
                "SELECT * FROM email_queue WHERE mailbox_id = $1 AND status = $2 ORDER BY created_at DESC LIMIT 100"
            )
            .bind(mailbox_id)
            .bind(status)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, EmailQueueItem>(
                "SELECT * FROM email_queue WHERE mailbox_id = $1 ORDER BY created_at DESC LIMIT 100"
            )
            .bind(mailbox_id)
            .fetch_all(pool)
            .await
        }
    }

    /// PURPOSE: Delete a queued email (cancel). Only works for pending/failed/dead_letter items.
    /// CONSTRAINTS: Cannot delete items currently being sent
    pub async fn delete(
        pool: &PgPool,
        id: Uuid,
        mailbox_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM email_queue WHERE id = $1 AND mailbox_id = $2 AND status IN ('pending', 'failed', 'dead_letter')"
        )
        .bind(id)
        .bind(mailbox_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Reset a failed/dead_letter item back to pending for retry
    pub async fn retry(
        pool: &PgPool,
        id: Uuid,
        mailbox_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE email_queue SET status = 'pending', retry_count = 0, next_retry_at = NOW(), last_error = NULL, failed_at = NULL
             WHERE id = $1 AND mailbox_id = $2 AND status IN ('failed', 'dead_letter')"
        )
        .bind(id)
        .bind(mailbox_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Get aggregate queue statistics (counts by status)
    pub async fn queue_stats(pool: &PgPool) -> Result<QueueStats, sqlx::Error> {
        let rows = sqlx::query_as::<_, StatusCount>(
            "SELECT status, COUNT(*) as count FROM email_queue GROUP BY status"
        )
        .fetch_all(pool)
        .await?;

        let mut stats = QueueStats {
            pending: 0,
            sending: 0,
            sent: 0,
            failed: 0,
            dead_letter: 0,
        };

        for row in rows {
            match row.status.as_str() {
                "pending" => stats.pending = row.count,
                "sending" => stats.sending = row.count,
                "sent" => stats.sent = row.count,
                "failed" => stats.failed = row.count,
                "dead_letter" => stats.dead_letter = row.count,
                _ => {}
            }
        }

        Ok(stats)
    }

    /// PURPOSE: Calculate the next retry delay in seconds using exponential backoff
    /// NOTE: Public helper for use in queue_processor and tests
    pub fn calculate_backoff_secs(retry_count: i32) -> i64 {
        BASE_RETRY_DELAY_SECS * 2_i64.pow(retry_count as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_queue_item_serialization() {
        let item = EmailQueueItem {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            to_addresses: vec!["alice@example.com".to_string()],
            cc_addresses: vec![],
            bcc_addresses: vec![],
            subject: "Test Subject".to_string(),
            body_html: "<p>Hello</p>".to_string(),
            body_text: "Hello".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            max_retries: 5,
            next_retry_at: Utc::now(),
            last_error: None,
            created_at: Utc::now(),
            sent_at: None,
            failed_at: None,
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("Test Subject"));
        assert!(json.contains("pending"));
        assert!(json.contains("alice@example.com"));

        let deserialized: EmailQueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.subject, "Test Subject");
        assert_eq!(deserialized.to_addresses.len(), 1);
        assert_eq!(deserialized.retry_count, 0);
    }

    #[test]
    fn test_email_queue_item_with_error() {
        let item = EmailQueueItem {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            to_addresses: vec!["bob@example.com".to_string()],
            cc_addresses: vec!["cc@example.com".to_string()],
            bcc_addresses: vec![],
            subject: "Failed Email".to_string(),
            body_html: "".to_string(),
            body_text: "body".to_string(),
            status: "failed".to_string(),
            retry_count: 3,
            max_retries: 5,
            next_retry_at: Utc::now(),
            last_error: Some("SMTP connection refused".to_string()),
            created_at: Utc::now(),
            sent_at: None,
            failed_at: Some(Utc::now()),
        };

        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["status"], "failed");
        assert_eq!(json["retry_count"], 3);
        assert_eq!(json["last_error"], "SMTP connection refused");
    }

    #[test]
    fn test_queue_stats_serialization() {
        let stats = QueueStats {
            pending: 10,
            sending: 2,
            sent: 150,
            failed: 3,
            dead_letter: 1,
        };

        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["pending"], 10);
        assert_eq!(json["sending"], 2);
        assert_eq!(json["sent"], 150);
        assert_eq!(json["failed"], 3);
        assert_eq!(json["dead_letter"], 1);
    }

    #[test]
    fn test_queue_stats_deserialization() {
        let json = r#"{"pending":5,"sending":1,"sent":100,"failed":2,"dead_letter":0}"#;
        let stats: QueueStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.pending, 5);
        assert_eq!(stats.sending, 1);
        assert_eq!(stats.sent, 100);
        assert_eq!(stats.failed, 2);
        assert_eq!(stats.dead_letter, 0);
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        // Added: Verify backoff formula: 30s * 2^retry_count
        assert_eq!(EmailQueueItem::calculate_backoff_secs(0), 30);
        assert_eq!(EmailQueueItem::calculate_backoff_secs(1), 60);
        assert_eq!(EmailQueueItem::calculate_backoff_secs(2), 120);
        assert_eq!(EmailQueueItem::calculate_backoff_secs(3), 240);
        assert_eq!(EmailQueueItem::calculate_backoff_secs(4), 480);
    }

    #[test]
    fn test_dead_letter_item_serialization() {
        let item = EmailQueueItem {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            to_addresses: vec!["user@test.com".to_string()],
            cc_addresses: vec![],
            bcc_addresses: vec![],
            subject: "Dead Letter".to_string(),
            body_html: "".to_string(),
            body_text: "content".to_string(),
            status: "dead_letter".to_string(),
            retry_count: 5,
            max_retries: 5,
            next_retry_at: Utc::now(),
            last_error: Some("Max retries exceeded".to_string()),
            created_at: Utc::now(),
            sent_at: None,
            failed_at: Some(Utc::now()),
        };

        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["status"], "dead_letter");
        assert_eq!(json["retry_count"], 5);
        assert_eq!(json["max_retries"], 5);
    }

    #[test]
    fn test_multiple_recipients_serialization() {
        let item = EmailQueueItem {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            to_addresses: vec![
                "a@test.com".to_string(),
                "b@test.com".to_string(),
                "c@test.com".to_string(),
            ],
            cc_addresses: vec!["d@test.com".to_string()],
            bcc_addresses: vec!["e@test.com".to_string(), "f@test.com".to_string()],
            subject: "Group Mail".to_string(),
            body_html: "<p>Hi all</p>".to_string(),
            body_text: "Hi all".to_string(),
            status: "pending".to_string(),
            retry_count: 0,
            max_retries: 5,
            next_retry_at: Utc::now(),
            last_error: None,
            created_at: Utc::now(),
            sent_at: None,
            failed_at: None,
        };

        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["to_addresses"].as_array().unwrap().len(), 3);
        assert_eq!(json["cc_addresses"].as_array().unwrap().len(), 1);
        assert_eq!(json["bcc_addresses"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_sent_item_has_sent_at() {
        let now = Utc::now();
        let item = EmailQueueItem {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            to_addresses: vec!["user@test.com".to_string()],
            cc_addresses: vec![],
            bcc_addresses: vec![],
            subject: "Sent".to_string(),
            body_html: "".to_string(),
            body_text: "done".to_string(),
            status: "sent".to_string(),
            retry_count: 0,
            max_retries: 5,
            next_retry_at: now,
            last_error: None,
            created_at: now,
            sent_at: Some(now),
            failed_at: None,
        };

        assert!(item.sent_at.is_some());
        assert!(item.failed_at.is_none());
        assert_eq!(item.status, "sent");
    }
}
