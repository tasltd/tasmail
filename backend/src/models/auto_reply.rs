use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AutoReplyRule {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub enabled: bool,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub reply_to_all: bool,
    pub exclude_lists: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertAutoReply {
    pub enabled: bool,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub reply_to_all: Option<bool>,
    pub exclude_lists: Option<bool>,
}

impl AutoReplyRule {
    /// Get auto-reply rule for a mailbox
    pub async fn find_by_mailbox(pool: &PgPool, mailbox_id: Uuid) -> Result<Option<AutoReplyRule>, sqlx::Error> {
        sqlx::query_as::<_, AutoReplyRule>(
            "SELECT * FROM auto_reply_rules WHERE mailbox_id = $1"
        )
        .bind(mailbox_id)
        .fetch_optional(pool)
        .await
    }

    /// Upsert auto-reply rule
    pub async fn upsert(
        pool: &PgPool,
        mailbox_id: Uuid,
        data: &UpsertAutoReply,
    ) -> Result<AutoReplyRule, sqlx::Error> {
        sqlx::query_as::<_, AutoReplyRule>(
            "INSERT INTO auto_reply_rules (mailbox_id, enabled, subject, body_text, body_html, start_date, end_date, reply_to_all, exclude_lists, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
             ON CONFLICT (mailbox_id) DO UPDATE
             SET enabled = $2, subject = $3, body_text = $4, body_html = $5, start_date = $6, end_date = $7,
                 reply_to_all = $8, exclude_lists = $9, updated_at = NOW()
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(data.enabled)
        .bind(&data.subject)
        .bind(&data.body_text)
        .bind(&data.body_html)
        .bind(data.start_date)
        .bind(data.end_date)
        .bind(data.reply_to_all.unwrap_or(false))
        .bind(data.exclude_lists.unwrap_or(true))
        .fetch_one(pool)
        .await
    }

    /// Check if auto-reply is currently active (enabled + within date range)
    pub fn is_active(&self) -> bool {
        if !self.enabled {
            return false;
        }
        let now = Utc::now();
        if let Some(start) = self.start_date {
            if now < start {
                return false;
            }
        }
        if let Some(end) = self.end_date {
            if now > end {
                return false;
            }
        }
        true
    }

    /// Check if we already replied to this sender recently (within 24h)
    pub async fn has_replied_recently(
        pool: &PgPool,
        mailbox_id: Uuid,
        sender: &str,
    ) -> Result<bool, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM auto_reply_log
             WHERE mailbox_id = $1 AND sender_address = $2
             AND replied_at > NOW() - INTERVAL '24 hours'"
        )
        .bind(mailbox_id)
        .bind(sender)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| r.0 > 0).unwrap_or(false))
    }

    /// Record that we replied to a sender
    pub async fn record_reply(
        pool: &PgPool,
        mailbox_id: Uuid,
        sender: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO auto_reply_log (mailbox_id, sender_address) VALUES ($1, $2)"
        )
        .bind(mailbox_id)
        .bind(sender)
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(enabled: bool, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> AutoReplyRule {
        AutoReplyRule {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            enabled,
            subject: "Out of Office".to_string(),
            body_text: "I'm away".to_string(),
            body_html: None,
            start_date: start,
            end_date: end,
            reply_to_all: false,
            exclude_lists: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_is_active_disabled() {
        let rule = make_rule(false, None, None);
        assert!(!rule.is_active());
    }

    #[test]
    fn test_is_active_no_dates() {
        let rule = make_rule(true, None, None);
        assert!(rule.is_active());
    }

    #[test]
    fn test_is_active_within_range() {
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let rule = make_rule(true, Some(start), Some(end));
        assert!(rule.is_active());
    }

    #[test]
    fn test_is_active_before_start() {
        let start = Utc::now() + chrono::Duration::hours(1);
        let rule = make_rule(true, Some(start), None);
        assert!(!rule.is_active());
    }

    #[test]
    fn test_is_active_after_end() {
        let end = Utc::now() - chrono::Duration::hours(1);
        let rule = make_rule(true, None, Some(end));
        assert!(!rule.is_active());
    }

    #[test]
    fn test_upsert_request_deserialization() {
        let json = r#"{
            "enabled": true,
            "subject": "On Vacation",
            "body_text": "I will return on Monday",
            "start_date": "2026-04-10T00:00:00Z",
            "end_date": "2026-04-17T23:59:59Z"
        }"#;
        let req: UpsertAutoReply = serde_json::from_str(json).unwrap();
        assert!(req.enabled);
        assert_eq!(req.subject, "On Vacation");
        assert!(req.start_date.is_some());
        assert!(req.end_date.is_some());
    }
}
