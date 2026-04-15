// Added: Deliverability check models for TMAIL-39 — email deliverability testing and scoring

use serde::{Deserialize, Serialize};

/// Added: Status of an individual deliverability check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Fail,
    Warn,
    Error,
}

/// Added: A single deliverability check result with name, status, and detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub details: String,
}

/// Added: Full deliverability report with scored check results (0-100)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverabilityReport {
    pub domain: String,
    pub checks: Vec<CheckResult>,
    pub score: u32,
}

/// Added: Query parameters for the deliverability check endpoint
#[derive(Debug, Deserialize)]
pub struct DeliverabilityCheckParams {
    pub domain: Option<String>,
}

impl CheckResult {
    /// Added: Create a passing check result
    pub fn pass(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Pass,
            details: details.to_string(),
        }
    }

    /// Added: Create a failing check result
    pub fn fail(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Fail,
            details: details.to_string(),
        }
    }

    /// Added: Create a warning check result
    pub fn warn(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Warn,
            details: details.to_string(),
        }
    }

    /// Added: Create an error check result (check could not be performed)
    pub fn error(name: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Error,
            details: details.to_string(),
        }
    }
}

impl DeliverabilityReport {
    /// Added: Calculate the overall score based on individual check results
    /// CONSTRAINTS: Score is 0-100; pass=full points, warn=half, fail/error=0
    pub fn calculate_score(checks: &[CheckResult]) -> u32 {
        if checks.is_empty() {
            return 0;
        }
        let points_per_check = 100.0 / checks.len() as f64;
        let total: f64 = checks
            .iter()
            .map(|c| match c.status {
                CheckStatus::Pass => points_per_check,
                CheckStatus::Warn => points_per_check * 0.5,
                CheckStatus::Fail | CheckStatus::Error => 0.0,
            })
            .sum();
        total.round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_result_constructors() {
        // Added: Verify convenience constructors set correct status
        let pass = CheckResult::pass("SPF", "Record found");
        assert_eq!(pass.status, CheckStatus::Pass);
        assert_eq!(pass.name, "SPF");

        let fail = CheckResult::fail("DKIM", "No record");
        assert_eq!(fail.status, CheckStatus::Fail);

        let warn = CheckResult::warn("PTR", "Mismatch");
        assert_eq!(warn.status, CheckStatus::Warn);

        let err = CheckResult::error("TLS", "Connection refused");
        assert_eq!(err.status, CheckStatus::Error);
    }

    #[test]
    fn test_score_all_pass() {
        // Added: All checks passing should yield 100
        let checks = vec![
            CheckResult::pass("SPF", "ok"),
            CheckResult::pass("DKIM", "ok"),
            CheckResult::pass("DMARC", "ok"),
            CheckResult::pass("PTR", "ok"),
        ];
        assert_eq!(DeliverabilityReport::calculate_score(&checks), 100);
    }

    #[test]
    fn test_score_all_fail() {
        // Added: All checks failing should yield 0
        let checks = vec![
            CheckResult::fail("SPF", "no"),
            CheckResult::fail("DKIM", "no"),
        ];
        assert_eq!(DeliverabilityReport::calculate_score(&checks), 0);
    }

    #[test]
    fn test_score_mixed() {
        // Added: Mix of pass/warn/fail should produce partial score
        let checks = vec![
            CheckResult::pass("SPF", "ok"),       // 25 points
            CheckResult::warn("DKIM", "partial"),  // 12.5 points
            CheckResult::fail("DMARC", "no"),      // 0 points
            CheckResult::pass("PTR", "ok"),        // 25 points
        ];
        // Expected: 25 + 12.5 + 0 + 25 = 62.5, rounds to 63
        assert_eq!(DeliverabilityReport::calculate_score(&checks), 63);
    }

    #[test]
    fn test_score_empty_checks() {
        // Added: No checks should return 0
        assert_eq!(DeliverabilityReport::calculate_score(&[]), 0);
    }

    #[test]
    fn test_score_single_warn() {
        // Added: Single warning should yield 50
        let checks = vec![CheckResult::warn("SPF", "partial")];
        assert_eq!(DeliverabilityReport::calculate_score(&checks), 50);
    }

    #[test]
    fn test_check_status_serialization() {
        // Added: CheckStatus serializes to lowercase strings
        let pass_json = serde_json::to_string(&CheckStatus::Pass).unwrap();
        assert_eq!(pass_json, "\"pass\"");
        let fail_json = serde_json::to_string(&CheckStatus::Fail).unwrap();
        assert_eq!(fail_json, "\"fail\"");
        let warn_json = serde_json::to_string(&CheckStatus::Warn).unwrap();
        assert_eq!(warn_json, "\"warn\"");
        let error_json = serde_json::to_string(&CheckStatus::Error).unwrap();
        assert_eq!(error_json, "\"error\"");
    }

    #[test]
    fn test_check_result_json_round_trip() {
        // Added: CheckResult should round-trip through JSON
        let check = CheckResult::pass("MX Records", "2 MX records found");
        let json = serde_json::to_string(&check).unwrap();
        let parsed: CheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "MX Records");
        assert_eq!(parsed.status, CheckStatus::Pass);
        assert_eq!(parsed.details, "2 MX records found");
    }

    #[test]
    fn test_deliverability_report_json_shape() {
        // Added: Verify the overall report JSON structure
        let report = DeliverabilityReport {
            domain: "example.com".to_string(),
            checks: vec![
                CheckResult::pass("SPF", "v=spf1 found"),
                CheckResult::fail("DKIM", "No DKIM record"),
            ],
            score: 50,
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["domain"], "example.com");
        assert_eq!(parsed["score"], 50);
        assert_eq!(parsed["checks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_deliverability_check_params_with_domain() {
        // Added: Verify query params deserialization
        let json = r#"{"domain": "mail.example.com"}"#;
        let params: DeliverabilityCheckParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.domain, Some("mail.example.com".to_string()));
    }

    #[test]
    fn test_deliverability_check_params_empty() {
        // Added: Empty query params should work
        let json = r#"{}"#;
        let params: DeliverabilityCheckParams = serde_json::from_str(json).unwrap();
        assert!(params.domain.is_none());
    }

    #[test]
    fn test_score_with_errors() {
        // Added: Error status checks count as zero points
        let checks = vec![
            CheckResult::pass("SPF", "ok"),
            CheckResult::error("Blacklist", "DNS timeout"),
            CheckResult::pass("DMARC", "ok"),
        ];
        // Expected: 33.3 + 0 + 33.3 = 66.6, rounds to 67
        assert_eq!(DeliverabilityReport::calculate_score(&checks), 67);
    }
}
