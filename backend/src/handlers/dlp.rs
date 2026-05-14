// Added: DLP rule and violation management handlers for TMAIL-108

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::dlp_rule::{
    CreateDlpRuleRequest, DlpRule, DlpScanMatch, DlpScanRequest, DlpViolation,
    UpdateDlpRuleRequest, ViolationListParams,
};
use crate::services::auth_service::{self, Claims};
use crate::services::dlp_scanner;
use crate::state::AppState;

/// GET /api/admin/dlp/rules — List all DLP rules
pub async fn list_rules(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<DlpRule>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let rules = DlpRule::list_all(&state.db).await?;
    Ok(Json(rules))
}

/// POST /api/admin/dlp/rules — Create a new DLP rule
pub async fn create_rule(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateDlpRuleRequest>,
) -> Result<(StatusCode, Json<DlpRule>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate pattern_type value
    if let Some(ref pattern_type) = body.pattern_type {
        let valid_types = ["regex", "keyword", "dictionary"];
        if !valid_types.contains(&pattern_type.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid pattern_type '{}'. Must be one of: {}",
                pattern_type,
                valid_types.join(", ")
            )));
        }
    }

    // Added: Validate regex compiles when pattern_type is regex
    let is_regex = body.pattern_type.as_deref().unwrap_or("regex") == "regex";
    if is_regex {
        if let Err(regex_err) = regex::Regex::new(&body.pattern) {
            return Err(AppError::BadRequest(format!(
                "Invalid regex pattern '{}': {}",
                body.pattern, regex_err
            )));
        }
    }

    let rule = DlpRule::create(&state.db, &body).await?;
    Ok((StatusCode::CREATED, Json(rule)))
}

/// PUT /api/admin/dlp/rules/:id — Update an existing DLP rule
pub async fn update_rule(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateDlpRuleRequest>,
) -> Result<Json<DlpRule>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate regex if pattern is being updated
    if let Some(ref pattern) = body.pattern {
        let is_regex = body.pattern_type.as_deref().unwrap_or("regex") == "regex";
        if is_regex {
            if let Err(regex_err) = regex::Regex::new(pattern) {
                return Err(AppError::BadRequest(format!(
                    "Invalid regex pattern '{}': {}",
                    pattern, regex_err
                )));
            }
        }
    }

    let rule = DlpRule::update(&state.db, id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("DLP rule '{}' not found", id)))?;
    Ok(Json(rule))
}

/// DELETE /api/admin/dlp/rules/:id — Delete a DLP rule and its violations
pub async fn delete_rule(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let deleted = DlpRule::delete(&state.db, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("DLP rule '{}' not found", id)))
    }
}

/// GET /api/admin/dlp/violations — List DLP violations with pagination
pub async fn list_violations(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(params): Query<ViolationListParams>,
) -> Result<Json<Vec<DlpViolation>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let violations = DlpViolation::list(&state.db, limit, offset).await?;
    Ok(Json(violations))
}

/// POST /api/admin/dlp/scan — Test scan text against all active DLP rules (dry-run)
pub async fn test_scan(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<DlpScanRequest>,
) -> Result<Json<Vec<DlpScanMatch>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let rules = DlpRule::list_active(&state.db).await?;
    let matches = dlp_scanner::scan_content(
        &rules,
        body.subject.as_deref(),
        body.body.as_deref(),
    );
    Ok(Json(matches))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_rule_request_deserialization() {
        // Added: Verify handler can deserialize a typical create request
        let json = r#"{
            "name": "Credit Card Blocker",
            "pattern": "\\b\\d{4}[- ]?\\d{4}[- ]?\\d{4}[- ]?\\d{4}\\b",
            "action": "block",
            "severity": "critical"
        }"#;
        let req: CreateDlpRuleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Credit Card Blocker");
    }

    #[test]
    fn test_scan_request_minimal() {
        // Added: Scan request with only body
        let json = r#"{"body": "Some text to scan"}"#;
        let req: DlpScanRequest = serde_json::from_str(json).unwrap();
        assert!(req.subject.is_none());
        assert_eq!(req.body, Some("Some text to scan".to_string()));
    }

    #[test]
    fn test_violation_list_params_defaults() {
        // Added: ViolationListParams with no values should parse to None
        let json = r#"{}"#;
        let params: ViolationListParams = serde_json::from_str(json).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
    }

    #[test]
    fn test_violation_list_params_with_values() {
        // Added: ViolationListParams with explicit pagination
        let json = r#"{"limit": 25, "offset": 50}"#;
        let params: ViolationListParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, Some(25));
        assert_eq!(params.offset, Some(50));
    }
}
