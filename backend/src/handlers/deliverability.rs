// Added: Deliverability check handler for TMAIL-39 — email deliverability testing endpoint
// PURPOSE: Admin-only endpoint to run comprehensive deliverability checks for a domain

use axum::{extract::Query, Json};

use crate::error::AppError;
use crate::models::deliverability::{
    DeliverabilityCheckParams, DeliverabilityReport, ExternalToolsResponse,
};
use crate::services::auth_service::{self, Claims};
use crate::services::deliverability_service;

/// GET /api/admin/deliverability/check — Run deliverability checks for a domain
/// CONSTRAINTS: Admin-only endpoint; domain param required
/// NOTE: Runs DNS, blacklist, TLS, and connectivity checks; returns scored report
pub async fn check_deliverability(
    axum::Extension(claims): axum::Extension<Claims>,
    Query(params): Query<DeliverabilityCheckParams>,
) -> Result<Json<DeliverabilityReport>, AppError> {
    // Fix: TMAIL-210 — admin-only.
    auth_service::require_admin(&claims)?;
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

/// GET /api/admin/deliverability/external-tools — TMAIL-39 external tools panel.
/// CONSTRAINTS: Admin-only. Always returns a fresh mail-tester handle (single-use),
/// the Google Postmaster Tools deep-link for the supplied domain, and the manual
/// provider checklist (Gmail/Outlook/Yahoo/ProtonMail).
/// NOTE: Domain is optional — when omitted, the Postmaster link falls back to the
/// bare managedomains landing page. The mail-tester handle does NOT depend on domain.
pub async fn external_deliverability_tools(
    axum::Extension(claims): axum::Extension<Claims>,
    Query(params): Query<DeliverabilityCheckParams>,
) -> Result<Json<ExternalToolsResponse>, AppError> {
    auth_service::require_admin(&claims)?;
    let domain = params.domain.as_deref().unwrap_or("").trim();
    Ok(Json(deliverability_service::build_external_tools(domain)))
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
    fn test_external_tools_response_shape() {
        // Added: TMAIL-39 — guard the wire format the SPA consumes. A field rename here
        // would silently break the External Tools panel; this test pins the JSON shape.
        let resp = crate::services::deliverability_service::build_external_tools(
            "mail.example.com",
        );
        let v = serde_json::to_value(&resp).unwrap();
        assert!(v["mail_tester"]["test_address"]
            .as_str()
            .unwrap()
            .starts_with("test-tasmail-"));
        assert!(v["mail_tester"]["report_url"]
            .as_str()
            .unwrap()
            .starts_with("https://www.mail-tester.com/"));
        assert_eq!(v["mail_tester"]["expires_in_minutes"], 45);
        assert!(v["google_postmaster"]["dashboard_url"]
            .as_str()
            .unwrap()
            .contains("mail.example.com"));
        assert_eq!(v["providers"].as_array().unwrap().len(), 4);
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
