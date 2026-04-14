// Added: Phishing report model for TMAIL-124 — persists per-message scan results

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Stores phishing scan results for a specific email message
/// CONSTRAINTS: risk_score must be 0-100, user_action must be one of: none, dismissed, reported, confirmed_safe
/// EXTERNAL: PostgreSQL with RLS — mailbox_id scoped via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PhishingReport {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub message_uid: i32,
    pub folder: String,
    pub suspicious_links: serde_json::Value,
    pub suspicious_sender: bool,
    pub spoofed_display_name: bool,
    pub risk_score: i32,
    pub user_action: String,
    pub created_at: DateTime<Utc>,
}

/// Added: Request body for updating user action on a phishing report
#[derive(Debug, Deserialize)]
pub struct UpdatePhishingAction {
    pub action: String,
}

impl PhishingReport {
    /// Added: Create a new phishing report for a scanned message
    pub async fn create(
        pool: &PgPool,
        mailbox_id: Uuid,
        message_uid: i32,
        folder: &str,
        suspicious_links: serde_json::Value,
        suspicious_sender: bool,
        spoofed_display_name: bool,
        risk_score: i32,
    ) -> Result<PhishingReport, sqlx::Error> {
        sqlx::query_as::<_, PhishingReport>(
            "INSERT INTO phishing_reports (mailbox_id, message_uid, folder, suspicious_links, suspicious_sender, spoofed_display_name, risk_score)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(message_uid)
        .bind(folder)
        .bind(&suspicious_links)
        .bind(suspicious_sender)
        .bind(spoofed_display_name)
        .bind(risk_score)
        .fetch_one(pool)
        .await
    }

    /// Added: Fetch existing phishing report for a specific message (if previously scanned)
    pub async fn find_for_message(
        pool: &PgPool,
        mailbox_id: Uuid,
        folder: &str,
        message_uid: i32,
    ) -> Result<Option<PhishingReport>, sqlx::Error> {
        sqlx::query_as::<_, PhishingReport>(
            "SELECT * FROM phishing_reports WHERE mailbox_id = $1 AND folder = $2 AND message_uid = $3"
        )
        .bind(mailbox_id)
        .bind(folder)
        .bind(message_uid)
        .fetch_optional(pool)
        .await
    }

    /// Added: Update the user's action on a phishing report (dismiss, report, confirm_safe)
    pub async fn update_user_action(
        pool: &PgPool,
        id: Uuid,
        action: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE phishing_reports SET user_action = $1 WHERE id = $2"
        )
        .bind(action)
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
    fn test_phishing_report_deserialization() {
        // Added: Verify JSON round-trip for PhishingReport struct
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "mailbox_id": "550e8400-e29b-41d4-a716-446655440001",
            "message_uid": 42,
            "folder": "INBOX",
            "suspicious_links": [{"url": "http://evil.com", "reason": "IP address URL"}],
            "suspicious_sender": false,
            "spoofed_display_name": true,
            "risk_score": 65,
            "user_action": "none",
            "created_at": "2026-04-14T10:00:00Z"
        }"#;
        let report: PhishingReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.message_uid, 42);
        assert_eq!(report.risk_score, 65);
        assert!(report.spoofed_display_name);
        assert!(!report.suspicious_sender);
    }

    #[test]
    fn test_phishing_report_serialization() {
        // Added: Verify serialization produces expected JSON shape
        let report = PhishingReport {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            folder: "INBOX".to_string(),
            message_uid: 100,
            suspicious_links: serde_json::json!([]),
            suspicious_sender: true,
            spoofed_display_name: false,
            risk_score: 30,
            user_action: "none".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["risk_score"], 30);
        assert_eq!(parsed["suspicious_sender"], true);
        assert_eq!(parsed["user_action"], "none");
    }

    #[test]
    fn test_update_action_deserialization() {
        // Added: Verify UpdatePhishingAction parses correctly
        let json = r#"{"action": "dismissed"}"#;
        let action: UpdatePhishingAction = serde_json::from_str(json).unwrap();
        assert_eq!(action.action, "dismissed");
    }

    #[test]
    fn test_valid_user_actions() {
        // Added: Validate all expected user action values parse correctly
        let valid_actions = vec!["none", "dismissed", "reported", "confirmed_safe"];
        for action_str in valid_actions {
            let json = format!(r#"{{"action": "{}"}}"#, action_str);
            let action: UpdatePhishingAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action.action, action_str);
        }
    }
}
