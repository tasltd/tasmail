// Added: Rspamd spam filter models for TMAIL-15
// PURPOSE: Data structs for spam settings, quarantine, and API request/response types
// CONSTRAINTS: spam_action enum must match the PostgreSQL ENUM defined in migration 050

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Rspamd action classification matching the spam_action PostgreSQL ENUM
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "spam_action", rename_all = "snake_case")]
pub enum SpamAction {
    #[serde(rename = "reject")]
    Reject,
    #[serde(rename = "greylist")]
    Greylist,
    #[serde(rename = "add_header")]
    AddHeader,
    #[serde(rename = "no_action")]
    NoAction,
}

/// PURPOSE: Domain-level spam filter configuration stored in PostgreSQL
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpamSettings {
    pub id: Uuid,
    pub domain_id: Option<Uuid>,
    pub threshold_reject: f64,
    pub threshold_greylist: f64,
    pub threshold_add_header: f64,
    pub learn_spam_enabled: bool,
    pub learn_ham_enabled: bool,
    pub dkim_signing_enabled: bool,
    pub arc_signing_enabled: bool,
    pub autolearn_enabled: bool,
    pub custom_rules: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PURPOSE: Request body for updating spam settings
#[derive(Debug, Deserialize)]
pub struct UpdateSpamSettings {
    pub threshold_reject: Option<f64>,
    pub threshold_greylist: Option<f64>,
    pub threshold_add_header: Option<f64>,
    pub learn_spam_enabled: Option<bool>,
    pub learn_ham_enabled: Option<bool>,
    pub dkim_signing_enabled: Option<bool>,
    pub arc_signing_enabled: Option<bool>,
    pub autolearn_enabled: Option<bool>,
    pub custom_rules: Option<serde_json::Value>,
}

/// PURPOSE: Quarantined email record with RLS enforced at DB level
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpamQuarantine {
    pub id: Uuid,
    pub user_id: Uuid,
    pub message_id: String,
    pub sender: Option<String>,
    pub subject: Option<String>,
    pub score: f64,
    pub action: SpamAction,
    pub symbols: serde_json::Value,
    pub quarantined_at: DateTime<Utc>,
    pub released: bool,
    pub released_at: Option<DateTime<Utc>>,
}

/// PURPOSE: Request body for learning a message as spam or ham
#[derive(Debug, Deserialize)]
pub struct LearnRequest {
    pub message_id: String,
    pub folder: String,
    pub is_spam: bool,
}

/// PURPOSE: Aggregated spam statistics for the stats endpoint
#[derive(Debug, Serialize)]
pub struct SpamStats {
    pub total_scanned: i64,
    pub total_blocked: i64,
    pub total_passed: i64,
    pub quarantined: i64,
    pub released: i64,
}

impl SpamSettings {
    /// PURPOSE: Fetch spam settings for a domain (or global defaults)
    pub async fn get_for_domain(pool: &PgPool, domain_id: Option<Uuid>) -> Result<Option<SpamSettings>, sqlx::Error> {
        if let Some(did) = domain_id {
            sqlx::query_as::<_, SpamSettings>(
                "SELECT * FROM spam_settings WHERE domain_id = $1 LIMIT 1"
            )
            .bind(did)
            .fetch_optional(pool)
            .await
        } else {
            sqlx::query_as::<_, SpamSettings>(
                "SELECT * FROM spam_settings WHERE domain_id IS NULL LIMIT 1"
            )
            .fetch_optional(pool)
            .await
        }
    }

    /// PURPOSE: Upsert spam settings for a domain
    pub async fn upsert(pool: &PgPool, domain_id: Option<Uuid>, data: &UpdateSpamSettings) -> Result<SpamSettings, sqlx::Error> {
        sqlx::query_as::<_, SpamSettings>(
            "INSERT INTO spam_settings (domain_id, threshold_reject, threshold_greylist, threshold_add_header,
             learn_spam_enabled, learn_ham_enabled, dkim_signing_enabled, arc_signing_enabled,
             autolearn_enabled, custom_rules)
             VALUES ($1, COALESCE($2, 15.0), COALESCE($3, 4.0), COALESCE($4, 6.0),
                     COALESCE($5, true), COALESCE($6, true), COALESCE($7, true), COALESCE($8, false),
                     COALESCE($9, true), COALESCE($10, '[]'::jsonb))
             ON CONFLICT (id) DO UPDATE SET
               threshold_reject = COALESCE($2, spam_settings.threshold_reject),
               threshold_greylist = COALESCE($3, spam_settings.threshold_greylist),
               threshold_add_header = COALESCE($4, spam_settings.threshold_add_header),
               learn_spam_enabled = COALESCE($5, spam_settings.learn_spam_enabled),
               learn_ham_enabled = COALESCE($6, spam_settings.learn_ham_enabled),
               dkim_signing_enabled = COALESCE($7, spam_settings.dkim_signing_enabled),
               arc_signing_enabled = COALESCE($8, spam_settings.arc_signing_enabled),
               autolearn_enabled = COALESCE($9, spam_settings.autolearn_enabled),
               custom_rules = COALESCE($10, spam_settings.custom_rules),
               updated_at = now()
             RETURNING *"
        )
        .bind(domain_id)
        .bind(data.threshold_reject)
        .bind(data.threshold_greylist)
        .bind(data.threshold_add_header)
        .bind(data.learn_spam_enabled)
        .bind(data.learn_ham_enabled)
        .bind(data.dkim_signing_enabled)
        .bind(data.arc_signing_enabled)
        .bind(data.autolearn_enabled)
        .bind(&data.custom_rules)
        .fetch_one(pool)
        .await
    }
}

impl SpamQuarantine {
    /// PURPOSE: List quarantined messages for current user (RLS enforced)
    pub async fn list_for_user(pool: &PgPool) -> Result<Vec<SpamQuarantine>, sqlx::Error> {
        sqlx::query_as::<_, SpamQuarantine>(
            "SELECT * FROM spam_quarantine ORDER BY quarantined_at DESC LIMIT 200"
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Release a quarantined message (mark as released)
    pub async fn release(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE spam_quarantine SET released = true, released_at = now() WHERE id = $1 AND released = false"
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Permanently delete a quarantined message
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM spam_quarantine WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Get aggregate spam statistics
    pub async fn stats(pool: &PgPool) -> Result<SpamStats, sqlx::Error> {
        // NOTE: total_scanned comes from quarantine table count; in production this would query Rspamd /stat
        let row = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT
               COUNT(*) as total,
               COUNT(*) FILTER (WHERE released = false) as blocked,
               COUNT(*) FILTER (WHERE released = true) as released
             FROM spam_quarantine"
        )
        .fetch_one(pool)
        .await?;

        Ok(SpamStats {
            total_scanned: row.0,
            total_blocked: row.1,
            total_passed: 0, // NOTE: Would come from Rspamd /stat endpoint in production
            quarantined: row.0,
            released: row.2,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spam_action_serialization() {
        let action = SpamAction::Reject;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"reject\"");

        let action = SpamAction::AddHeader;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"add_header\"");
    }

    #[test]
    fn test_spam_action_deserialization() {
        let action: SpamAction = serde_json::from_str("\"greylist\"").unwrap();
        assert_eq!(action, SpamAction::Greylist);

        let action: SpamAction = serde_json::from_str("\"no_action\"").unwrap();
        assert_eq!(action, SpamAction::NoAction);
    }

    #[test]
    fn test_update_spam_settings_deserialization() {
        let json = r#"{
            "threshold_reject": 20.0,
            "dkim_signing_enabled": true,
            "autolearn_enabled": false
        }"#;
        let req: UpdateSpamSettings = serde_json::from_str(json).unwrap();
        assert_eq!(req.threshold_reject, Some(20.0));
        assert_eq!(req.dkim_signing_enabled, Some(true));
        assert_eq!(req.autolearn_enabled, Some(false));
        assert!(req.threshold_greylist.is_none());
    }

    #[test]
    fn test_learn_request_deserialization() {
        let json = r#"{
            "message_id": "<abc@example.com>",
            "folder": "INBOX",
            "is_spam": true
        }"#;
        let req: LearnRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.message_id, "<abc@example.com>");
        assert_eq!(req.folder, "INBOX");
        assert!(req.is_spam);
    }

    #[test]
    fn test_learn_request_ham() {
        let json = r#"{"message_id": "msg-1", "folder": "Spam", "is_spam": false}"#;
        let req: LearnRequest = serde_json::from_str(json).unwrap();
        assert!(!req.is_spam);
    }

    #[test]
    fn test_spam_stats_serialization() {
        let stats = SpamStats {
            total_scanned: 1000,
            total_blocked: 150,
            total_passed: 850,
            quarantined: 50,
            released: 10,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"total_scanned\":1000"));
        assert!(json.contains("\"total_blocked\":150"));
        assert!(json.contains("\"quarantined\":50"));
    }

    #[test]
    fn test_quarantine_serialization() {
        let q = SpamQuarantine {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            message_id: "<test@example.com>".to_string(),
            sender: Some("spammer@bad.com".to_string()),
            subject: Some("Buy now!!!".to_string()),
            score: 12.50,
            action: SpamAction::Reject,
            symbols: serde_json::json!(["BAYES_SPAM", "DKIM_SIGNED"]),
            quarantined_at: Utc::now(),
            released: false,
            released_at: None,
        };
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains("\"sender\":\"spammer@bad.com\""));
        assert!(json.contains("\"action\":\"reject\""));
        assert!(json.contains("\"released\":false"));
    }

    #[test]
    fn test_quarantine_released_serialization() {
        let q = SpamQuarantine {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            message_id: "msg-2".to_string(),
            sender: None,
            subject: None,
            score: 5.00,
            action: SpamAction::Greylist,
            symbols: serde_json::json!([]),
            quarantined_at: Utc::now(),
            released: true,
            released_at: Some(Utc::now()),
        };
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains("\"released\":true"));
        assert!(json.contains("\"released_at\":"));
    }
}
