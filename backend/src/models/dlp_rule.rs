// Added: DLP rule and violation models for TMAIL-108 — Data Loss Prevention scanning

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents the action to take when a DLP rule matches
/// CONSTRAINTS: Must match the dlp_action PostgreSQL enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "dlp_action", rename_all = "lowercase")]
pub enum DlpAction {
    #[serde(rename = "block")]
    Block,
    #[serde(rename = "quarantine")]
    Quarantine,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "log")]
    Log,
}

/// PURPOSE: Severity level for DLP rule violations
/// CONSTRAINTS: Must match the dlp_severity PostgreSQL enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "dlp_severity", rename_all = "lowercase")]
pub enum DlpSeverity {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "critical")]
    Critical,
}

/// PURPOSE: A DLP rule that defines a pattern to scan outgoing emails against
/// CONSTRAINTS: pattern must be a valid regex when pattern_type is 'regex'
/// EXTERNAL: PostgreSQL dlp_rules table
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DlpRule {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub pattern: String,
    pub pattern_type: String,
    pub action: DlpAction,
    pub severity: DlpSeverity,
    pub apply_to_subject: bool,
    pub apply_to_body: bool,
    pub apply_to_attachments: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PURPOSE: Records when a DLP rule matched outgoing email content
/// EXTERNAL: PostgreSQL dlp_violations table with FK to dlp_rules and users
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DlpViolation {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub user_id: Uuid,
    pub action_taken: DlpAction,
    pub matched_pattern: String,
    pub matched_text: Option<String>,
    pub message_subject: Option<String>,
    pub recipient: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Added: Request body for creating a new DLP rule
#[derive(Debug, Deserialize)]
pub struct CreateDlpRuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub pattern: String,
    pub pattern_type: Option<String>,
    pub action: Option<DlpAction>,
    pub severity: Option<DlpSeverity>,
    pub apply_to_subject: Option<bool>,
    pub apply_to_body: Option<bool>,
    pub apply_to_attachments: Option<bool>,
}

/// Added: Request body for updating an existing DLP rule
#[derive(Debug, Deserialize)]
pub struct UpdateDlpRuleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub pattern: Option<String>,
    pub pattern_type: Option<String>,
    pub action: Option<DlpAction>,
    pub severity: Option<DlpSeverity>,
    pub apply_to_subject: Option<bool>,
    pub apply_to_body: Option<bool>,
    pub apply_to_attachments: Option<bool>,
    pub active: Option<bool>,
}

/// Added: Request body for test-scanning text against DLP rules
#[derive(Debug, Deserialize)]
pub struct DlpScanRequest {
    pub subject: Option<String>,
    pub body: Option<String>,
    pub recipient: Option<String>,
}

/// Added: Response for a single match found during DLP scan
#[derive(Debug, Serialize)]
pub struct DlpScanMatch {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub action: DlpAction,
    pub severity: DlpSeverity,
    pub matched_pattern: String,
    pub matched_text: String,
}

/// Added: Pagination query params for listing violations
#[derive(Debug, Deserialize)]
pub struct ViolationListParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl DlpRule {
    /// Added: List all DLP rules (active and inactive)
    pub async fn list_all(pool: &PgPool) -> Result<Vec<DlpRule>, sqlx::Error> {
        sqlx::query_as::<_, DlpRule>(
            "SELECT * FROM dlp_rules ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// Added: List only active DLP rules for scanning
    pub async fn list_active(pool: &PgPool) -> Result<Vec<DlpRule>, sqlx::Error> {
        sqlx::query_as::<_, DlpRule>(
            "SELECT * FROM dlp_rules WHERE active = true ORDER BY severity DESC, created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// Added: Create a new DLP rule
    pub async fn create(
        pool: &PgPool,
        req: &CreateDlpRuleRequest,
    ) -> Result<DlpRule, sqlx::Error> {
        let pattern_type = req.pattern_type.as_deref().unwrap_or("regex");
        let action = req.action.clone().unwrap_or(DlpAction::Warn);
        let severity = req.severity.clone().unwrap_or(DlpSeverity::Medium);

        sqlx::query_as::<_, DlpRule>(
            "INSERT INTO dlp_rules (name, description, pattern, pattern_type, action, severity, apply_to_subject, apply_to_body, apply_to_attachments)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING *",
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.pattern)
        .bind(pattern_type)
        .bind(&action)
        .bind(&severity)
        .bind(req.apply_to_subject.unwrap_or(true))
        .bind(req.apply_to_body.unwrap_or(true))
        .bind(req.apply_to_attachments.unwrap_or(false))
        .fetch_one(pool)
        .await
    }

    /// Added: Update an existing DLP rule by ID
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        req: &UpdateDlpRuleRequest,
    ) -> Result<Option<DlpRule>, sqlx::Error> {
        // NOTE: Use COALESCE-style approach — each field uses its current value if not provided
        sqlx::query_as::<_, DlpRule>(
            "UPDATE dlp_rules SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                pattern = COALESCE($4, pattern),
                pattern_type = COALESCE($5, pattern_type),
                action = COALESCE($6, action),
                severity = COALESCE($7, severity),
                apply_to_subject = COALESCE($8, apply_to_subject),
                apply_to_body = COALESCE($9, apply_to_body),
                apply_to_attachments = COALESCE($10, apply_to_attachments),
                active = COALESCE($11, active),
                updated_at = now()
             WHERE id = $1
             RETURNING *",
        )
        .bind(id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.pattern)
        .bind(&req.pattern_type)
        .bind(&req.action)
        .bind(&req.severity)
        .bind(req.apply_to_subject)
        .bind(req.apply_to_body)
        .bind(req.apply_to_attachments)
        .bind(req.active)
        .fetch_optional(pool)
        .await
    }

    /// Added: Delete a DLP rule and its associated violations
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        // NOTE: Delete violations first due to FK constraint
        sqlx::query("DELETE FROM dlp_violations WHERE rule_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        let result = sqlx::query("DELETE FROM dlp_rules WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl DlpViolation {
    /// Added: Record a new DLP violation
    pub async fn create(
        pool: &PgPool,
        rule_id: Uuid,
        user_id: Uuid,
        action_taken: &DlpAction,
        matched_pattern: &str,
        matched_text: Option<&str>,
        message_subject: Option<&str>,
        recipient: Option<&str>,
    ) -> Result<DlpViolation, sqlx::Error> {
        sqlx::query_as::<_, DlpViolation>(
            "INSERT INTO dlp_violations (rule_id, user_id, action_taken, matched_pattern, matched_text, message_subject, recipient)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *",
        )
        .bind(rule_id)
        .bind(user_id)
        .bind(action_taken)
        .bind(matched_pattern)
        .bind(matched_text)
        .bind(message_subject)
        .bind(recipient)
        .fetch_one(pool)
        .await
    }

    /// Added: List violations with pagination, newest first
    pub async fn list(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DlpViolation>, sqlx::Error> {
        sqlx::query_as::<_, DlpViolation>(
            "SELECT * FROM dlp_violations ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlp_action_serialization() {
        // Added: Verify DlpAction serializes to lowercase strings
        assert_eq!(serde_json::to_string(&DlpAction::Block).unwrap(), "\"block\"");
        assert_eq!(serde_json::to_string(&DlpAction::Quarantine).unwrap(), "\"quarantine\"");
        assert_eq!(serde_json::to_string(&DlpAction::Warn).unwrap(), "\"warn\"");
        assert_eq!(serde_json::to_string(&DlpAction::Log).unwrap(), "\"log\"");
    }

    #[test]
    fn test_dlp_action_deserialization() {
        // Added: Verify DlpAction deserializes from lowercase strings
        let action: DlpAction = serde_json::from_str("\"block\"").unwrap();
        assert_eq!(action, DlpAction::Block);
        let action: DlpAction = serde_json::from_str("\"quarantine\"").unwrap();
        assert_eq!(action, DlpAction::Quarantine);
    }

    #[test]
    fn test_dlp_severity_serialization() {
        // Added: Verify DlpSeverity enum round-trips through JSON
        assert_eq!(serde_json::to_string(&DlpSeverity::Low).unwrap(), "\"low\"");
        assert_eq!(serde_json::to_string(&DlpSeverity::Critical).unwrap(), "\"critical\"");
    }

    #[test]
    fn test_dlp_severity_deserialization() {
        // Added: Verify DlpSeverity parses from string
        let severity: DlpSeverity = serde_json::from_str("\"high\"").unwrap();
        assert_eq!(severity, DlpSeverity::High);
    }

    #[test]
    fn test_dlp_rule_deserialization() {
        // Added: Verify DlpRule struct deserializes from JSON
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "Credit Card Detection",
            "description": "Detects credit card numbers in outgoing emails",
            "pattern": "\\b\\d{4}[- ]?\\d{4}[- ]?\\d{4}[- ]?\\d{4}\\b",
            "pattern_type": "regex",
            "action": "block",
            "severity": "critical",
            "apply_to_subject": true,
            "apply_to_body": true,
            "apply_to_attachments": false,
            "active": true,
            "created_at": "2026-04-14T10:00:00Z",
            "updated_at": "2026-04-14T10:00:00Z"
        }"#;
        let rule: DlpRule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.name, "Credit Card Detection");
        assert_eq!(rule.action, DlpAction::Block);
        assert_eq!(rule.severity, DlpSeverity::Critical);
        assert!(rule.apply_to_body);
        assert!(!rule.apply_to_attachments);
    }

    #[test]
    fn test_dlp_rule_serialization() {
        // Added: Verify DlpRule serialization produces expected JSON shape
        let rule = DlpRule {
            id: Uuid::new_v4(),
            name: "SSN Detector".to_string(),
            description: Some("Detects US Social Security Numbers".to_string()),
            pattern: r"\b\d{3}-\d{2}-\d{4}\b".to_string(),
            pattern_type: "regex".to_string(),
            action: DlpAction::Warn,
            severity: DlpSeverity::High,
            apply_to_subject: false,
            apply_to_body: true,
            apply_to_attachments: false,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&rule).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "SSN Detector");
        assert_eq!(parsed["action"], "warn");
        assert_eq!(parsed["severity"], "high");
    }

    #[test]
    fn test_dlp_violation_deserialization() {
        // Added: Verify DlpViolation struct round-trips through JSON
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "rule_id": "550e8400-e29b-41d4-a716-446655440001",
            "user_id": "550e8400-e29b-41d4-a716-446655440002",
            "action_taken": "block",
            "matched_pattern": "\\b\\d{4}[- ]?\\d{4}[- ]?\\d{4}[- ]?\\d{4}\\b",
            "matched_text": "4111-1111-1111-1111",
            "message_subject": "Invoice details",
            "recipient": "client@example.com",
            "created_at": "2026-04-14T10:00:00Z"
        }"#;
        let violation: DlpViolation = serde_json::from_str(json).unwrap();
        assert_eq!(violation.matched_text, Some("4111-1111-1111-1111".to_string()));
        assert_eq!(violation.action_taken, DlpAction::Block);
    }

    #[test]
    fn test_create_dlp_rule_request_defaults() {
        // Added: Verify CreateDlpRuleRequest deserializes with optional fields as None
        let json = r#"{
            "name": "Test Rule",
            "pattern": "test.*pattern"
        }"#;
        let req: CreateDlpRuleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Test Rule");
        assert_eq!(req.pattern, "test.*pattern");
        assert!(req.description.is_none());
        assert!(req.pattern_type.is_none());
        assert!(req.action.is_none());
        assert!(req.severity.is_none());
    }

    #[test]
    fn test_create_dlp_rule_request_full() {
        // Added: Verify CreateDlpRuleRequest with all fields populated
        let json = r#"{
            "name": "IBAN Detector",
            "description": "Detects IBAN numbers",
            "pattern": "\\b[A-Z]{2}\\d{2}[A-Z0-9]{4,}\\b",
            "pattern_type": "regex",
            "action": "quarantine",
            "severity": "high",
            "apply_to_subject": false,
            "apply_to_body": true,
            "apply_to_attachments": true
        }"#;
        let req: CreateDlpRuleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.action, Some(DlpAction::Quarantine));
        assert_eq!(req.severity, Some(DlpSeverity::High));
        assert_eq!(req.apply_to_attachments, Some(true));
    }

    #[test]
    fn test_update_dlp_rule_request_partial() {
        // Added: Verify partial update only sets provided fields
        let json = r#"{"active": false, "action": "block"}"#;
        let req: UpdateDlpRuleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.active, Some(false));
        assert_eq!(req.action, Some(DlpAction::Block));
        assert!(req.name.is_none());
        assert!(req.pattern.is_none());
    }

    #[test]
    fn test_dlp_scan_request_deserialization() {
        // Added: Verify DlpScanRequest for test scan endpoint
        let json = r#"{
            "subject": "Invoice for Q1",
            "body": "Please find card number 4111-1111-1111-1111 attached",
            "recipient": "client@example.com"
        }"#;
        let req: DlpScanRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.subject, Some("Invoice for Q1".to_string()));
        assert!(req.body.unwrap().contains("4111"));
    }

    #[test]
    fn test_dlp_scan_match_serialization() {
        // Added: Verify DlpScanMatch output format
        let scan_match = DlpScanMatch {
            rule_id: Uuid::new_v4(),
            rule_name: "Credit Card".to_string(),
            action: DlpAction::Block,
            severity: DlpSeverity::Critical,
            matched_pattern: r"\d{4}-\d{4}-\d{4}-\d{4}".to_string(),
            matched_text: "4111-1111-1111-1111".to_string(),
        };
        let json = serde_json::to_string(&scan_match).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["rule_name"], "Credit Card");
        assert_eq!(parsed["action"], "block");
        assert_eq!(parsed["severity"], "critical");
    }
}
