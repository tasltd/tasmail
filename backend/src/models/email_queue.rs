// Added: Email queue model with retry logic for TMAIL-58
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a queued email with retry tracking
/// CONSTRAINTS: status must be one of: pending, sending, sent, failed, dead_letter, bounced
/// NOTE: `priority` higher value = drained first (urgent=10, normal=0, bulk=-10)
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
    // Added: TMAIL-58 priority queue support — higher value drains first
    #[serde(default)]
    pub priority: i32,
}

/// PURPOSE: Aggregated queue statistics by status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub pending: i64,
    pub sending: i64,
    pub sent: i64,
    pub failed: i64,
    pub dead_letter: i64,
    // Added: TMAIL-58 bounced status (hard-bounced after NDR detection)
    #[serde(default)]
    pub bounced: i64,
}

/// PURPOSE: Helper row for counting items per status
#[derive(Debug, sqlx::FromRow)]
struct StatusCount {
    status: String,
    count: i64,
}

/// Changed: TMAIL-58 spec retry schedule — 3 retries at 5s, 30s, 300s (5m)
/// retry_count=0 → wait 5s, retry_count=1 → wait 30s, retry_count=2 → wait 300s, retry_count>=3 → dead_letter
const RETRY_SCHEDULE_SECS: &[i64] = &[5, 30, 300];

/// Added: Priority levels (callers pass these as the priority arg to `enqueue_with_priority`)
pub const PRIORITY_URGENT: i32 = 10;
pub const PRIORITY_NORMAL: i32 = 0;
pub const PRIORITY_BULK: i32 = -10;

impl EmailQueueItem {
    /// PURPOSE: Enqueue a new email with normal priority
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
        Self::enqueue_with_priority(
            pool, mailbox_id, to, cc, bcc, subject, body_html, body_text, PRIORITY_NORMAL,
        )
        .await
    }

    /// PURPOSE: Enqueue a new email with explicit priority (TMAIL-58)
    /// Higher priority drains first; defaults to PRIORITY_NORMAL.
    #[allow(clippy::too_many_arguments)]
    pub async fn enqueue_with_priority(
        pool: &PgPool,
        mailbox_id: Uuid,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        body_html: &str,
        body_text: &str,
        priority: i32,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, EmailQueueItem>(
            "INSERT INTO email_queue (mailbox_id, to_addresses, cc_addresses, bcc_addresses, subject, body_html, body_text, priority)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(to)
        .bind(cc)
        .bind(bcc)
        .bind(subject)
        .bind(body_html)
        .bind(body_text)
        .bind(priority)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Count messages enqueued for a mailbox within the recent window (TMAIL-58 rate limiting).
    /// Spec: 30 msgs/min/user — caller computes `now - 60s` and compares against `RATE_LIMIT_MAX`.
    pub async fn count_recent_for_mailbox(
        pool: &PgPool,
        mailbox_id: Uuid,
        since_secs: i64,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM email_queue
             WHERE mailbox_id = $1
               AND created_at >= NOW() - make_interval(secs => $2::double precision)"
        )
        .bind(mailbox_id)
        .bind(since_secs as f64)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Fetch items ready to send (pending or failed with next_retry_at <= NOW).
    /// Kept for tests / introspection — the worker uses `claim_batch` instead.
    pub async fn fetch_ready(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        // Changed: priority-ordered (TMAIL-58) — urgent items drain first, then by next_retry_at
        sqlx::query_as::<_, EmailQueueItem>(
            "SELECT * FROM email_queue
             WHERE status IN ('pending', 'failed') AND next_retry_at <= NOW()
             ORDER BY priority DESC, next_retry_at ASC
             LIMIT $1"
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Atomically claim a batch of ready items by transitioning them from
    /// pending/failed → sending in a single statement. Uses `FOR UPDATE SKIP LOCKED`
    /// so multiple worker instances/processes can run concurrently without re-sending
    /// the same email.
    ///
    /// PRODUCTION-GRADE: Each item is claimed once across the entire fleet of workers.
    /// CTE pattern is the standard Postgres recipe for "competing consumers".
    pub async fn claim_batch(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        // Changed: TMAIL-58 priority-ordered claim — urgent items drain first
        sqlx::query_as::<_, EmailQueueItem>(
            "WITH claimable AS (
                 SELECT id
                 FROM email_queue
                 WHERE status IN ('pending', 'failed') AND next_retry_at <= NOW()
                 ORDER BY priority DESC, next_retry_at ASC
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED
             )
             UPDATE email_queue eq
             SET status = 'sending'
             FROM claimable
             WHERE eq.id = claimable.id
             RETURNING eq.*",
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
        // Changed: TMAIL-58 spec backoff schedule (5s, 30s, 300s) instead of pure exponential
        let backoff_secs = Self::calculate_backoff_secs(retry_count);
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

    /// PURPOSE: Mark item as hard-bounced after NDR detection (TMAIL-58 bounce handling)
    /// Bounced items never retry — they're terminal like dead_letter but for SMTP-level rejections
    /// (invalid mailbox, mailbox full, blocked domain, etc.) rather than transient failures.
    pub async fn mark_bounced(
        pool: &PgPool,
        id: Uuid,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE email_queue SET status = 'bounced', last_error = $2, failed_at = NOW() WHERE id = $1"
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

        Ok(Self::aggregate_status_counts(&rows))
    }

    /// PURPOSE: Aggregate per-status counts into QueueStats. Pure helper, testable without a DB.
    fn aggregate_status_counts(rows: &[StatusCount]) -> QueueStats {
        let mut stats = QueueStats {
            pending: 0,
            sending: 0,
            sent: 0,
            failed: 0,
            dead_letter: 0,
            bounced: 0,
        };

        for row in rows {
            match row.status.as_str() {
                "pending" => stats.pending = row.count,
                "sending" => stats.sending = row.count,
                "sent" => stats.sent = row.count,
                "failed" => stats.failed = row.count,
                "dead_letter" => stats.dead_letter = row.count,
                "bounced" => stats.bounced = row.count,
                _ => {}
            }
        }

        stats
    }

    /// PURPOSE: Calculate the next retry delay in seconds.
    /// Schedule (TMAIL-58 spec): retry_count=0 → 5s, retry_count=1 → 30s, retry_count=2 → 300s (5m).
    /// For retry_count >= len(RETRY_SCHEDULE_SECS), the caller should mark dead_letter — but
    /// we still cap at the last value defensively.
    pub fn calculate_backoff_secs(retry_count: i32) -> i64 {
        let idx = (retry_count.max(0) as usize).min(RETRY_SCHEDULE_SECS.len() - 1);
        RETRY_SCHEDULE_SECS[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(status: &str, retry_count: i32) -> EmailQueueItem {
        EmailQueueItem {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            to_addresses: vec!["alice@example.com".to_string()],
            cc_addresses: vec![],
            bcc_addresses: vec![],
            subject: "Test Subject".to_string(),
            body_html: "<p>Hello</p>".to_string(),
            body_text: "Hello".to_string(),
            status: status.to_string(),
            retry_count,
            max_retries: 3,
            next_retry_at: Utc::now(),
            last_error: None,
            created_at: Utc::now(),
            sent_at: None,
            failed_at: None,
            priority: PRIORITY_NORMAL,
        }
    }

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
            max_retries: 3,
            next_retry_at: Utc::now(),
            last_error: None,
            created_at: Utc::now(),
            sent_at: None,
            failed_at: None,
            priority: PRIORITY_NORMAL,
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
            priority: PRIORITY_NORMAL,
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
            bounced: 4,
        };

        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["pending"], 10);
        assert_eq!(json["sending"], 2);
        assert_eq!(json["sent"], 150);
        assert_eq!(json["failed"], 3);
        assert_eq!(json["dead_letter"], 1);
        assert_eq!(json["bounced"], 4);
    }

    #[test]
    fn test_queue_stats_deserialization() {
        let json = r#"{"pending":5,"sending":1,"sent":100,"failed":2,"dead_letter":0,"bounced":7}"#;
        let stats: QueueStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.pending, 5);
        assert_eq!(stats.sending, 1);
        assert_eq!(stats.sent, 100);
        assert_eq!(stats.failed, 2);
        assert_eq!(stats.dead_letter, 0);
        assert_eq!(stats.bounced, 7);
    }

    // Added: TMAIL-58 — verify backoff schedule matches spec (5s, 30s, 300s)
    #[test]
    fn test_backoff_matches_spec_schedule() {
        assert_eq!(EmailQueueItem::calculate_backoff_secs(0), 5, "first retry should wait 5s");
        assert_eq!(EmailQueueItem::calculate_backoff_secs(1), 30, "second retry should wait 30s");
        assert_eq!(EmailQueueItem::calculate_backoff_secs(2), 300, "third retry should wait 5m");
    }

    // Added: TMAIL-58 — backoff must cap at the last schedule value (not panic on overflow)
    #[test]
    fn test_backoff_caps_at_last_value() {
        assert_eq!(EmailQueueItem::calculate_backoff_secs(3), 300);
        assert_eq!(EmailQueueItem::calculate_backoff_secs(99), 300);
        assert_eq!(EmailQueueItem::calculate_backoff_secs(-1), 5, "negative retry_count clamps to 0");
    }

    #[test]
    fn test_dead_letter_item_serialization() {
        let item = sample_item("dead_letter", 3);
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["status"], "dead_letter");
        assert_eq!(json["retry_count"], 3);
    }

    #[test]
    fn test_multiple_recipients_serialization() {
        let mut item = sample_item("pending", 0);
        item.to_addresses = vec![
            "a@test.com".to_string(),
            "b@test.com".to_string(),
            "c@test.com".to_string(),
        ];
        item.cc_addresses = vec!["d@test.com".to_string()];
        item.bcc_addresses = vec!["e@test.com".to_string(), "f@test.com".to_string()];

        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["to_addresses"].as_array().unwrap().len(), 3);
        assert_eq!(json["cc_addresses"].as_array().unwrap().len(), 1);
        assert_eq!(json["bcc_addresses"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_sent_item_has_sent_at() {
        let now = Utc::now();
        let mut item = sample_item("sent", 0);
        item.sent_at = Some(now);
        item.failed_at = None;
        assert!(item.sent_at.is_some());
        assert!(item.failed_at.is_none());
        assert_eq!(item.status, "sent");
    }

    // Added: TMAIL-58 — bounced is a distinct terminal state, separate from dead_letter
    #[test]
    fn test_bounced_status_serialization() {
        let mut item = sample_item("bounced", 0);
        item.last_error = Some("550 5.1.1 mailbox unavailable".to_string());
        item.failed_at = Some(Utc::now());

        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["status"], "bounced");
        assert_eq!(json["last_error"], "550 5.1.1 mailbox unavailable");
    }

    // Added: TMAIL-58 — priority field round-trips through JSON
    #[test]
    fn test_priority_serialization_default_and_urgent() {
        let normal = sample_item("pending", 0);
        let json = serde_json::to_value(&normal).unwrap();
        assert_eq!(json["priority"], 0);

        let mut urgent = sample_item("pending", 0);
        urgent.priority = PRIORITY_URGENT;
        let json = serde_json::to_value(&urgent).unwrap();
        assert_eq!(json["priority"], 10);

        let mut bulk = sample_item("pending", 0);
        bulk.priority = PRIORITY_BULK;
        let json = serde_json::to_value(&bulk).unwrap();
        assert_eq!(json["priority"], -10);
    }

    // Added: TMAIL-58 — older clients/rows may omit priority; default to 0
    #[test]
    fn test_priority_defaults_when_absent_in_json() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000001",
            "mailbox_id": "00000000-0000-0000-0000-000000000002",
            "to_addresses": ["x@y.z"],
            "cc_addresses": [],
            "bcc_addresses": [],
            "subject": "s",
            "body_html": "",
            "body_text": "",
            "status": "pending",
            "retry_count": 0,
            "max_retries": 3,
            "next_retry_at": "2026-01-01T00:00:00Z",
            "last_error": null,
            "created_at": "2026-01-01T00:00:00Z",
            "sent_at": null,
            "failed_at": null
        }"#;
        let item: EmailQueueItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.priority, 0);
    }

    // Added: TMAIL-58 — priority constants reflect spec ordering
    #[test]
    fn test_priority_constants_ordering() {
        assert!(PRIORITY_URGENT > PRIORITY_NORMAL);
        assert!(PRIORITY_NORMAL > PRIORITY_BULK);
        assert_eq!(PRIORITY_URGENT, 10);
        assert_eq!(PRIORITY_NORMAL, 0);
        assert_eq!(PRIORITY_BULK, -10);
    }

    // Added: TMAIL-58 — aggregate_status_counts groups rows correctly
    #[test]
    fn test_aggregate_status_counts_distributes_correctly() {
        let rows = vec![
            StatusCount { status: "pending".into(), count: 7 },
            StatusCount { status: "sending".into(), count: 2 },
            StatusCount { status: "sent".into(), count: 150 },
            StatusCount { status: "failed".into(), count: 3 },
            StatusCount { status: "dead_letter".into(), count: 1 },
            StatusCount { status: "bounced".into(), count: 4 },
        ];
        let stats = EmailQueueItem::aggregate_status_counts(&rows);
        assert_eq!(stats.pending, 7);
        assert_eq!(stats.sending, 2);
        assert_eq!(stats.sent, 150);
        assert_eq!(stats.failed, 3);
        assert_eq!(stats.dead_letter, 1);
        assert_eq!(stats.bounced, 4);
    }

    // Added: TMAIL-58 — unknown statuses are ignored, not promoted into another bucket
    #[test]
    fn test_aggregate_status_counts_ignores_unknown_status() {
        let rows = vec![
            StatusCount { status: "pending".into(), count: 5 },
            StatusCount { status: "alien_status".into(), count: 99 },
        ];
        let stats = EmailQueueItem::aggregate_status_counts(&rows);
        assert_eq!(stats.pending, 5);
        assert_eq!(stats.sent, 0);
        assert_eq!(stats.bounced, 0);
    }

    // Added: TMAIL-58 — empty inputs produce all-zero stats (covers "no rows yet" startup case)
    #[test]
    fn test_aggregate_status_counts_empty_input() {
        let stats = EmailQueueItem::aggregate_status_counts(&[]);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.sending, 0);
        assert_eq!(stats.sent, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.dead_letter, 0);
        assert_eq!(stats.bounced, 0);
    }
}
