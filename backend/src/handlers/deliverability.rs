// Added: Deliverability check handler for TMAIL-39 — email deliverability testing endpoint
// PURPOSE: Admin-only endpoint to run comprehensive deliverability checks for a domain

use axum::{extract::Query, Json};

use crate::error::AppError;
use crate::models::deliverability::{DeliverabilityCheckParams, DeliverabilityReport};
use crate::services::auth_service::Claims;
use crate::services::deliverability_service;

/// GET /api/admin/deliverability/check — Run deliverability checks for a domain
/// CONSTRAINTS: Admin-only endpoint; domain param required
/// NOTE: Runs DNS, blacklist, TLS, and connectivity checks; returns scored report
pub async fn check_deliverability(
    axum::Extension(_claims): axum::Extension<Claims>,
    Query(params): Query<DeliverabilityCheckParams>,
) -> Result<Json<DeliverabilityReport>, AppError> {
    // Added: Validate domain parameter is provided
    let domain = params
        .domain
        .as_deref()
        .filter(|d| !d.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("domain query parameter is required".to_string()))?;

    // Added: Run all deliverability checks
    let report = deliverability_service::run_deliverability_checks(domain).await;

    Ok(Json(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::deliverability::DeliverabilityCheckParams;

    #[test]
    fn test_params_deserialization_with_domain() {
        // Added: Verify query param parsing with domain
        let json = r#"{"domain": "mail.example.com"}"#;
        let params: DeliverabilityCheckParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.domain, Some("mail.example.com".to_string()));
    }

    #[test]
    fn test_params_deserialization_empty() {
        // Added: Verify query param parsing without domain
        let json = r#"{}"#;
        let params: DeliverabilityCheckParams = serde_json::from_str(json).unwrap();
        assert!(params.domain.is_none());
    }

    #[test]
    fn test_report_json_output_format() {
        // Added: Verify the expected JSON output shape
        use crate::models::deliverability::{CheckResult, DeliverabilityReport};
        let report = DeliverabilityReport {
            domain: "example.com".to_string(),
            checks: vec![
                CheckResult::pass("SPF Record", "v=spf1 found"),
                CheckResult::fail("DKIM Record", "No DKIM record"),
                CheckResult::warn("DMARC Record", "p=none policy"),
            ],
            score: 50,
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["domain"], "example.com");
        assert_eq!(parsed["score"], 50);
        assert!(parsed["checks"].is_array());
        assert_eq!(parsed["checks"][0]["status"], "pass");
        assert_eq!(parsed["checks"][1]["status"], "fail");
        assert_eq!(parsed["checks"][2]["status"], "warn");
    }
}
