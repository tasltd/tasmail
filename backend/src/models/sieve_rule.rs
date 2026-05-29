use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Condition field for matching emails
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleCondition {
    /// Field to match: "from", "to", "cc", "subject", "header", "size", "body"
    pub field: String,
    /// Operator: "contains", "not_contains", "equals", "starts_with", "ends_with", "matches_regex", "greater_than", "less_than"
    pub operator: String,
    /// Value to compare against
    pub value: String,
}

/// Action to perform when rule matches
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleAction {
    /// Action type: "move", "copy", "delete", "mark_read", "mark_flagged", "forward", "reject", "add_label", "stop"
    pub action_type: String,
    /// Target value (folder name, email address, label name) — optional for actions like delete/mark_read
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SieveRule {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub name: String,
    pub priority: i32,
    pub enabled: bool,
    /// Conditions as JSONB
    pub conditions: serde_json::Value,
    pub match_mode: String,
    /// Actions as JSONB
    pub actions: serde_json::Value,
    pub stop_processing: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSieveRule {
    pub name: String,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub conditions: Vec<RuleCondition>,
    pub match_mode: Option<String>,
    pub actions: Vec<RuleAction>,
    pub stop_processing: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSieveRule {
    pub name: Option<String>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub conditions: Option<Vec<RuleCondition>>,
    pub match_mode: Option<String>,
    pub actions: Option<Vec<RuleAction>>,
    pub stop_processing: Option<bool>,
}

// Added (TMAIL-286): Sample message payload + per-condition match breakdown
// for the rule-test sandbox at `POST /api/filters/{id}/test`.
#[derive(Debug, Deserialize, Default)]
pub struct SampleMessage {
    pub from: Option<String>,
    pub to: Option<String>,
    pub cc: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ConditionMatchResult {
    pub field: String,
    pub operator: String,
    pub value: String,
    pub matched: bool,
}

#[derive(Debug, Serialize)]
pub struct RuleMatchBreakdown {
    pub matched: bool,
    pub match_mode: String,
    pub condition_results: Vec<ConditionMatchResult>,
}

/// Serializable response with parsed conditions/actions
#[derive(Debug, Serialize)]
pub struct SieveRuleResponse {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub name: String,
    pub priority: i32,
    pub enabled: bool,
    pub conditions: Vec<RuleCondition>,
    pub match_mode: String,
    pub actions: Vec<RuleAction>,
    pub stop_processing: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SieveRule {
    /// Convert raw DB row to typed response
    pub fn to_response(&self) -> SieveRuleResponse {
        SieveRuleResponse {
            id: self.id,
            mailbox_id: self.mailbox_id,
            name: self.name.clone(),
            priority: self.priority,
            enabled: self.enabled,
            conditions: serde_json::from_value(self.conditions.clone()).unwrap_or_default(),
            match_mode: self.match_mode.clone(),
            actions: serde_json::from_value(self.actions.clone()).unwrap_or_default(),
            stop_processing: self.stop_processing,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub async fn find_by_mailbox(pool: &PgPool, mailbox_id: Uuid) -> Result<Vec<SieveRule>, sqlx::Error> {
        sqlx::query_as::<_, SieveRule>(
            "SELECT * FROM sieve_rules WHERE mailbox_id = $1 ORDER BY priority ASC, created_at ASC"
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<Option<SieveRule>, sqlx::Error> {
        sqlx::query_as::<_, SieveRule>(
            "SELECT * FROM sieve_rules WHERE id = $1 AND mailbox_id = $2"
        )
        .bind(id)
        .bind(mailbox_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &PgPool,
        mailbox_id: Uuid,
        data: &CreateSieveRule,
    ) -> Result<SieveRule, sqlx::Error> {
        let conditions_json = serde_json::to_value(&data.conditions).unwrap_or_default();
        let actions_json = serde_json::to_value(&data.actions).unwrap_or_default();

        sqlx::query_as::<_, SieveRule>(
            "INSERT INTO sieve_rules (mailbox_id, name, priority, enabled, conditions, match_mode, actions, stop_processing)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(&data.name)
        .bind(data.priority.unwrap_or(0))
        .bind(data.enabled.unwrap_or(true))
        .bind(conditions_json)
        .bind(data.match_mode.as_deref().unwrap_or("all"))
        .bind(actions_json)
        .bind(data.stop_processing.unwrap_or(true))
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        mailbox_id: Uuid,
        data: &UpdateSieveRule,
    ) -> Result<Option<SieveRule>, sqlx::Error> {
        // NOTE: partial update — only provided fields are changed
        let existing = Self::find_by_id(pool, id, mailbox_id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let name = data.name.as_deref().unwrap_or(&existing.name);
        let priority = data.priority.unwrap_or(existing.priority);
        let enabled = data.enabled.unwrap_or(existing.enabled);
        let match_mode = data.match_mode.as_deref().unwrap_or(&existing.match_mode);
        let stop_processing = data.stop_processing.unwrap_or(existing.stop_processing);

        let conditions_json = match &data.conditions {
            Some(c) => serde_json::to_value(c).unwrap_or_default(),
            None => existing.conditions.clone(),
        };
        let actions_json = match &data.actions {
            Some(a) => serde_json::to_value(a).unwrap_or_default(),
            None => existing.actions.clone(),
        };

        sqlx::query_as::<_, SieveRule>(
            "UPDATE sieve_rules
             SET name = $3, priority = $4, enabled = $5, conditions = $6, match_mode = $7,
                 actions = $8, stop_processing = $9, updated_at = NOW()
             WHERE id = $1 AND mailbox_id = $2
             RETURNING *"
        )
        .bind(id)
        .bind(mailbox_id)
        .bind(name)
        .bind(priority)
        .bind(enabled)
        .bind(conditions_json)
        .bind(match_mode)
        .bind(actions_json)
        .bind(stop_processing)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM sieve_rules WHERE id = $1 AND mailbox_id = $2"
        )
        .bind(id)
        .bind(mailbox_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Reorder rules by updating priorities
    pub async fn reorder(pool: &PgPool, mailbox_id: Uuid, rule_ids: &[Uuid]) -> Result<(), sqlx::Error> {
        for (idx, rule_id) in rule_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE sieve_rules SET priority = $3, updated_at = NOW() WHERE id = $1 AND mailbox_id = $2"
            )
            .bind(rule_id)
            .bind(mailbox_id)
            .bind(idx as i32)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    // Added (TMAIL-286): per-condition breakdown for the sample-test
    // endpoint. Re-uses `evaluate_condition` so the UI sandbox cannot drift
    // from the production matcher.
    pub fn evaluate_sample(&self, sample: &SampleMessage) -> RuleMatchBreakdown {
        let conditions: Vec<RuleCondition> =
            serde_json::from_value(self.conditions.clone()).unwrap_or_default();

        let empty = String::new();
        let from = sample.from.as_ref().unwrap_or(&empty);
        let to = sample.to.as_ref().unwrap_or(&empty);
        let cc = sample.cc.as_ref().unwrap_or(&empty);
        let subject = sample.subject.as_ref().unwrap_or(&empty);
        let body = sample.body.as_ref().unwrap_or(&empty);

        let condition_results: Vec<ConditionMatchResult> = conditions
            .iter()
            .map(|c| {
                let field_value = match c.field.as_str() {
                    "from" => from.as_str(),
                    "to" => to.as_str(),
                    "cc" => cc.as_str(),
                    "subject" => subject.as_str(),
                    "body" => body.as_str(),
                    _ => "",
                };
                let matched = evaluate_condition(field_value, &c.operator, &c.value);
                ConditionMatchResult {
                    field: c.field.clone(),
                    operator: c.operator.clone(),
                    value: c.value.clone(),
                    matched,
                }
            })
            .collect();

        let matched = if condition_results.is_empty() {
            false
        } else {
            match self.match_mode.as_str() {
                "any" => condition_results.iter().any(|r| r.matched),
                _ => condition_results.iter().all(|r| r.matched),
            }
        };

        RuleMatchBreakdown {
            matched,
            match_mode: self.match_mode.clone(),
            condition_results,
        }
    }

    /// Evaluate whether an email matches this rule's conditions
    pub fn matches_email(&self, from: &str, to: &str, subject: &str, headers: &[(String, String)]) -> bool {
        let conditions: Vec<RuleCondition> = serde_json::from_value(self.conditions.clone()).unwrap_or_default();
        if conditions.is_empty() {
            return false;
        }

        let results: Vec<bool> = conditions.iter().map(|c| {
            let field_value = match c.field.as_str() {
                "from" => from,
                "to" | "cc" => to,
                "subject" => subject,
                "header" => {
                    // Find header by name in the value field (format: "Header-Name: value")
                    headers.iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(&c.value))
                        .map(|(_, v)| v.as_str())
                        .unwrap_or("")
                }
                _ => "",
            };
            evaluate_condition(field_value, &c.operator, &c.value)
        }).collect();

        match self.match_mode.as_str() {
            "any" => results.iter().any(|&r| r),
            _ => results.iter().all(|&r| r), // "all" is default
        }
    }
}

/// Evaluate a single condition against a field value
fn evaluate_condition(field_value: &str, operator: &str, target: &str) -> bool {
    let fv_lower = field_value.to_lowercase();
    let target_lower = target.to_lowercase();
    match operator {
        "contains" => fv_lower.contains(&target_lower),
        "not_contains" => !fv_lower.contains(&target_lower),
        "equals" => fv_lower == target_lower,
        "starts_with" => fv_lower.starts_with(&target_lower),
        "ends_with" => fv_lower.ends_with(&target_lower),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_condition(field: &str, operator: &str, value: &str) -> RuleCondition {
        RuleCondition {
            field: field.to_string(),
            operator: operator.to_string(),
            value: value.to_string(),
        }
    }

    fn make_action(action_type: &str, target: Option<&str>) -> RuleAction {
        RuleAction {
            action_type: action_type.to_string(),
            target: target.map(|s| s.to_string()),
        }
    }

    fn make_rule(conditions: Vec<RuleCondition>, match_mode: &str, enabled: bool) -> SieveRule {
        SieveRule {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            name: "Test Rule".to_string(),
            priority: 0,
            enabled,
            conditions: serde_json::to_value(&conditions).unwrap(),
            match_mode: match_mode.to_string(),
            actions: serde_json::to_value(vec![make_action("move", Some("Spam"))]).unwrap(),
            stop_processing: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_condition_contains() {
        assert!(evaluate_condition("hello world", "contains", "world"));
        assert!(!evaluate_condition("hello world", "contains", "foo"));
    }

    #[test]
    fn test_condition_not_contains() {
        assert!(evaluate_condition("hello world", "not_contains", "foo"));
        assert!(!evaluate_condition("hello world", "not_contains", "world"));
    }

    #[test]
    fn test_condition_equals() {
        assert!(evaluate_condition("test@example.com", "equals", "test@example.com"));
        assert!(evaluate_condition("Test@Example.com", "equals", "test@example.com")); // case insensitive
        assert!(!evaluate_condition("test@example.com", "equals", "other@example.com"));
    }

    #[test]
    fn test_condition_starts_with() {
        assert!(evaluate_condition("newsletter@company.com", "starts_with", "newsletter"));
        assert!(!evaluate_condition("newsletter@company.com", "starts_with", "company"));
    }

    #[test]
    fn test_condition_ends_with() {
        assert!(evaluate_condition("user@spam.com", "ends_with", "spam.com"));
        assert!(!evaluate_condition("user@spam.com", "ends_with", "good.com"));
    }

    #[test]
    fn test_matches_email_all_mode() {
        let conditions = vec![
            make_condition("from", "contains", "newsletter"),
            make_condition("subject", "contains", "offer"),
        ];
        let rule = make_rule(conditions, "all", true);

        // Both conditions match
        assert!(rule.matches_email("newsletter@co.com", "me@me.com", "Special offer!", &[]));
        // Only one matches
        assert!(!rule.matches_email("newsletter@co.com", "me@me.com", "Hello", &[]));
        // Neither matches
        assert!(!rule.matches_email("friend@co.com", "me@me.com", "Hello", &[]));
    }

    #[test]
    fn test_matches_email_any_mode() {
        let conditions = vec![
            make_condition("from", "contains", "spam"),
            make_condition("subject", "contains", "free money"),
        ];
        let rule = make_rule(conditions, "any", true);

        // One matches
        assert!(rule.matches_email("spam@bad.com", "me@me.com", "Hello", &[]));
        assert!(rule.matches_email("friend@ok.com", "me@me.com", "Get free money now!", &[]));
        // Neither matches
        assert!(!rule.matches_email("friend@ok.com", "me@me.com", "Hello friend", &[]));
    }

    #[test]
    fn test_matches_email_empty_conditions() {
        let rule = make_rule(vec![], "all", true);
        assert!(!rule.matches_email("from@co.com", "to@me.com", "Sub", &[]));
    }

    #[test]
    fn test_to_response_parsing() {
        let conditions = vec![make_condition("from", "contains", "test")];
        let actions = vec![make_action("move", Some("Archive"))];
        let rule = SieveRule {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            name: "My Rule".to_string(),
            priority: 1,
            enabled: true,
            conditions: serde_json::to_value(&conditions).unwrap(),
            match_mode: "all".to_string(),
            actions: serde_json::to_value(&actions).unwrap(),
            stop_processing: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let resp = rule.to_response();
        assert_eq!(resp.conditions.len(), 1);
        assert_eq!(resp.conditions[0].field, "from");
        assert_eq!(resp.actions.len(), 1);
        assert_eq!(resp.actions[0].action_type, "move");
        assert_eq!(resp.actions[0].target.as_deref(), Some("Archive"));
    }

    #[test]
    fn test_create_request_deserialization() {
        let json = r#"{
            "name": "Move newsletters",
            "conditions": [{"field": "from", "operator": "contains", "value": "newsletter"}],
            "actions": [{"action_type": "move", "target": "Newsletters"}],
            "match_mode": "all",
            "stop_processing": true
        }"#;
        let req: CreateSieveRule = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Move newsletters");
        assert_eq!(req.conditions.len(), 1);
        assert_eq!(req.actions.len(), 1);
    }

    #[test]
    fn test_update_request_partial() {
        let json = r#"{"name": "Updated Name"}"#;
        let req: UpdateSieveRule = serde_json::from_str(json).unwrap();
        assert_eq!(req.name.as_deref(), Some("Updated Name"));
        assert!(req.conditions.is_none());
        assert!(req.actions.is_none());
    }

    #[test]
    fn test_rule_action_types() {
        let actions = vec![
            make_action("move", Some("Trash")),
            make_action("mark_read", None),
            make_action("forward", Some("admin@co.com")),
        ];
        assert_eq!(actions[0].action_type, "move");
        assert_eq!(actions[1].target, None);
        assert_eq!(actions[2].target.as_deref(), Some("admin@co.com"));
    }

    #[test]
    fn test_case_insensitive_matching() {
        let conditions = vec![make_condition("from", "contains", "SPAM")];
        let rule = make_rule(conditions, "all", true);
        assert!(rule.matches_email("spam@bad.com", "me@me.com", "Hello", &[]));
    }

    // Added (TMAIL-286): tests for the sample-evaluation path used by the
    // `POST /api/filters/{id}/test` endpoint. Cover the same shape the SPA
    // sends: a few well-known fields, partial population, ALL vs ANY.
    fn sample(from: &str, subject: &str, body: &str) -> SampleMessage {
        SampleMessage {
            from: Some(from.to_string()),
            subject: Some(subject.to_string()),
            body: Some(body.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_evaluate_sample_all_mode_all_match() {
        let conditions = vec![
            make_condition("from", "contains", "newsletter"),
            make_condition("subject", "contains", "offer"),
        ];
        let rule = make_rule(conditions, "all", true);
        let result = rule.evaluate_sample(&sample(
            "newsletter@store.com",
            "Special offer for you",
            "Body content here",
        ));
        assert!(result.matched);
        assert_eq!(result.condition_results.len(), 2);
        assert!(result.condition_results.iter().all(|c| c.matched));
    }

    #[test]
    fn test_evaluate_sample_all_mode_partial_fails() {
        let conditions = vec![
            make_condition("from", "contains", "newsletter"),
            make_condition("subject", "contains", "offer"),
        ];
        let rule = make_rule(conditions, "all", true);
        let result = rule.evaluate_sample(&sample(
            "newsletter@store.com",
            "Hello friend",
            "Body content",
        ));
        assert!(!result.matched);
        // Per-condition breakdown still flags WHICH one missed.
        assert_eq!(result.condition_results[0].matched, true);
        assert_eq!(result.condition_results[1].matched, false);
    }

    #[test]
    fn test_evaluate_sample_any_mode_one_match() {
        let conditions = vec![
            make_condition("from", "contains", "spam"),
            make_condition("subject", "contains", "free money"),
        ];
        let rule = make_rule(conditions, "any", true);
        let result = rule.evaluate_sample(&sample(
            "friend@ok.com",
            "Get free money now!",
            "Body",
        ));
        assert!(result.matched);
        assert_eq!(result.match_mode, "any");
        assert!(!result.condition_results[0].matched);
        assert!(result.condition_results[1].matched);
    }

    #[test]
    fn test_evaluate_sample_body_field() {
        let conditions = vec![make_condition("body", "contains", "unsubscribe")];
        let rule = make_rule(conditions, "all", true);
        let yes = rule.evaluate_sample(&sample("a@b.c", "hi", "Please unsubscribe here"));
        let no = rule.evaluate_sample(&sample("a@b.c", "hi", "Plain text only"));
        assert!(yes.matched);
        assert!(!no.matched);
    }

    #[test]
    fn test_evaluate_sample_missing_field_treated_as_empty() {
        let conditions = vec![make_condition("subject", "contains", "offer")];
        let rule = make_rule(conditions, "all", true);
        // Subject omitted entirely — treated as empty, so the contains check fails.
        let result = rule.evaluate_sample(&SampleMessage {
            from: Some("x@y.z".to_string()),
            ..Default::default()
        });
        assert!(!result.matched);
        assert!(!result.condition_results[0].matched);
    }

    #[test]
    fn test_evaluate_sample_empty_conditions_never_matches() {
        let rule = make_rule(vec![], "all", true);
        let result = rule.evaluate_sample(&sample("from@x.com", "sub", "body"));
        assert!(!result.matched);
        assert!(result.condition_results.is_empty());
    }
}
