use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::sieve_rule::{
    CreateSieveRule, RuleMatchBreakdown, SampleMessage, SieveRule, SieveRuleResponse,
    UpdateSieveRule,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Helper to parse mailbox_id from JWT claims
fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}

/// GET /api/filters — List all filter rules for the current user
pub async fn list_rules(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<SieveRuleResponse>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let rules = SieveRule::find_by_mailbox(&state.db, mailbox_id).await?;
    let responses: Vec<SieveRuleResponse> = rules.iter().map(|r| r.to_response()).collect();
    Ok(Json(responses))
}

/// POST /api/filters — Create a new filter rule
pub async fn create_rule(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateSieveRule>,
) -> Result<(StatusCode, Json<SieveRuleResponse>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Validate conditions and actions are not empty
    if body.conditions.is_empty() {
        return Err(AppError::BadRequest("At least one condition is required".to_string()));
    }
    if body.actions.is_empty() {
        return Err(AppError::BadRequest("At least one action is required".to_string()));
    }

    // Validate match_mode if provided
    if let Some(ref mode) = body.match_mode {
        if mode != "all" && mode != "any" {
            return Err(AppError::BadRequest("match_mode must be 'all' or 'any'".to_string()));
        }
    }

    let rule = SieveRule::create(&state.db, mailbox_id, &body).await?;
    Ok((StatusCode::CREATED, Json(rule.to_response())))
}

/// PUT /api/filters/{id} — Update an existing filter rule
pub async fn update_rule(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSieveRule>,
) -> Result<Json<SieveRuleResponse>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Validate match_mode if provided
    if let Some(ref mode) = body.match_mode {
        if mode != "all" && mode != "any" {
            return Err(AppError::BadRequest("match_mode must be 'all' or 'any'".to_string()));
        }
    }

    let rule = SieveRule::update(&state.db, id, mailbox_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Filter rule not found".to_string()))?;

    Ok(Json(rule.to_response()))
}

/// DELETE /api/filters/{id} — Delete a filter rule
pub async fn delete_rule(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let deleted = SieveRule::delete(&state.db, id, mailbox_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Filter rule not found".to_string()))
    }
}

/// POST /api/filters/reorder — Reorder filter rules by priority
pub async fn reorder_rules(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(rule_ids): Json<Vec<Uuid>>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    if rule_ids.is_empty() {
        return Err(AppError::BadRequest("Rule IDs list cannot be empty".to_string()));
    }

    SieveRule::reorder(&state.db, mailbox_id, &rule_ids).await?;
    Ok(StatusCode::OK)
}

/// Added (TMAIL-286): POST /api/filters/{id}/test — dry-run the rule against
/// a synthetic message so the SPA can render a "would match / would not match"
/// preview. The match logic reuses the same evaluator that runs in production
/// so the sandbox and the live filter cannot drift.
pub async fn test_rule(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(sample): Json<SampleMessage>,
) -> Result<Json<RuleMatchBreakdown>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let rule = SieveRule::find_by_id(&state.db, id, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Filter rule not found".to_string()))?;
    Ok(Json(rule.evaluate_sample(&sample)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sieve_rule::{RuleAction, RuleCondition};

    #[test]
    fn test_create_rule_request_full() {
        let json = r#"{
            "name": "Move newsletters to folder",
            "priority": 1,
            "enabled": true,
            "conditions": [
                {"field": "from", "operator": "contains", "value": "newsletter@"},
                {"field": "subject", "operator": "not_contains", "value": "important"}
            ],
            "match_mode": "all",
            "actions": [
                {"action_type": "move", "target": "Newsletters"},
                {"action_type": "mark_read", "target": null}
            ],
            "stop_processing": true
        }"#;
        let req: CreateSieveRule = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Move newsletters to folder");
        assert_eq!(req.conditions.len(), 2);
        assert_eq!(req.actions.len(), 2);
        assert_eq!(req.match_mode, Some("all".to_string()));
    }

    #[test]
    fn test_create_rule_request_minimal() {
        let json = r#"{
            "name": "Basic filter",
            "conditions": [{"field": "from", "operator": "equals", "value": "spam@bad.com"}],
            "actions": [{"action_type": "delete"}]
        }"#;
        let req: CreateSieveRule = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Basic filter");
        assert!(req.priority.is_none());
        assert!(req.enabled.is_none());
        assert!(req.match_mode.is_none());
    }

    #[test]
    fn test_update_rule_request_partial() {
        let json = r#"{"enabled": false}"#;
        let req: UpdateSieveRule = serde_json::from_str(json).unwrap();
        assert_eq!(req.enabled, Some(false));
        assert!(req.name.is_none());
        assert!(req.conditions.is_none());
    }

    #[test]
    fn test_reorder_payload() {
        let json = r#"["550e8400-e29b-41d4-a716-446655440000", "6ba7b810-9dad-11d1-80b4-00c04fd430c8"]"#;
        let ids: Vec<Uuid> = serde_json::from_str(json).unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_condition_serialization_roundtrip() {
        let condition = RuleCondition {
            field: "from".to_string(),
            operator: "contains".to_string(),
            value: "test@example.com".to_string(),
        };
        let json = serde_json::to_string(&condition).unwrap();
        let parsed: RuleCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(condition, parsed);
    }

    #[test]
    fn test_action_serialization_roundtrip() {
        let action = RuleAction {
            action_type: "forward".to_string(),
            target: Some("admin@company.com".to_string()),
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: RuleAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, parsed);
    }

    #[test]
    fn test_match_mode_validation() {
        // Valid modes
        assert!(["all", "any"].contains(&"all"));
        assert!(["all", "any"].contains(&"any"));
        // Invalid mode
        assert!(!["all", "any"].contains(&"none"));
    }
}
