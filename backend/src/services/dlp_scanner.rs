// Added: DLP scanner service for TMAIL-108 — scans outgoing email content against DLP rules

use regex::Regex;
use std::sync::LazyLock;

use crate::models::dlp_rule::{DlpAction, DlpRule, DlpScanMatch, DlpSeverity};

// Added: Pre-compiled built-in patterns for common sensitive data types

/// PURPOSE: Matches credit card numbers (Visa, MasterCard, Amex, Discover) with optional separators
/// CONSTRAINTS: Luhn validation is NOT performed — only format matching
static CREDIT_CARD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:4\d{3}|5[1-5]\d{2}|3[47]\d{2}|6(?:011|5\d{2}))[- ]?\d{4}[- ]?\d{4}[- ]?\d{1,4}\b").unwrap()
});

/// PURPOSE: Matches US Social Security Numbers in XXX-XX-XXXX format
/// CONSTRAINTS: Does not match SSNs without dashes to reduce false positives
static SSN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap()
});

/// PURPOSE: Matches IBAN numbers (2 letter country code + 2 check digits + up to 30 alphanumeric)
static IBAN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{4,30}\b").unwrap()
});

/// Added: Default list of dangerous attachment extensions blocked by the DLP milter
/// NOTE: Extensions are matched case-insensitively against the filename suffix.
///   Operators can override via the `TASMAIL_DLP_BLOCKED_EXTENSIONS` env var
///   (comma-separated, no leading dot) read by the milter binary.
pub const DEFAULT_BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "bat", "cmd", "com", "scr", "pif", "vbs", "vbe", "js", "jse",
    "wsf", "wsh", "ps1", "psm1", "msi", "msp", "hta", "cpl", "jar",
    "lnk", "reg", "iso", "img",
];

/// PURPOSE: Returns true when the filename ends in one of the supplied blocked extensions
/// CONSTRAINTS: case-insensitive; empty filename or missing extension returns false
pub fn is_blocked_attachment(filename: &str, blocked: &[&str]) -> bool {
    let lower = filename.to_lowercase();
    let ext = match lower.rsplit_once('.') {
        Some((_, e)) if !e.is_empty() => e,
        _ => return false,
    };
    blocked.iter().any(|b| b.eq_ignore_ascii_case(ext))
}

/// PURPOSE: Scan a list of attachment filenames against blocked extensions
/// EXTERNAL: Pure function — caller provides filenames extracted from MIME parts
pub fn scan_attachments(
    filenames: &[String],
    blocked: &[&str],
) -> Vec<DlpScanMatch> {
    let mut matches = Vec::new();
    for filename in filenames {
        if is_blocked_attachment(filename, blocked) {
            matches.push(DlpScanMatch {
                rule_id: uuid::Uuid::nil(),
                rule_name: "Blocked Attachment Extension".to_string(),
                action: DlpAction::Block,
                severity: DlpSeverity::High,
                matched_pattern: blocked.join(","),
                matched_text: filename.clone(),
            });
        }
    }
    matches
}

/// PURPOSE: Built-in DLP patterns for common sensitive data — used as defaults
/// NOTE: These are applied in addition to user-created rules in the database
pub fn get_builtin_patterns() -> Vec<BuiltinPattern> {
    vec![
        BuiltinPattern {
            name: "Credit Card Number".to_string(),
            regex: &CREDIT_CARD_REGEX,
            action: DlpAction::Block,
            severity: DlpSeverity::Critical,
        },
        BuiltinPattern {
            name: "US Social Security Number".to_string(),
            regex: &SSN_REGEX,
            action: DlpAction::Block,
            severity: DlpSeverity::Critical,
        },
        BuiltinPattern {
            name: "IBAN Bank Account".to_string(),
            regex: &IBAN_REGEX,
            action: DlpAction::Warn,
            severity: DlpSeverity::High,
        },
    ]
}

/// Added: Represents a built-in (hardcoded) DLP pattern
pub struct BuiltinPattern {
    pub name: String,
    pub regex: &'static Regex,
    pub action: DlpAction,
    pub severity: DlpSeverity,
}

/// PURPOSE: Scan email content against all active DLP rules from the database
/// CONSTRAINTS: Rules with pattern_type 'regex' are compiled per-call — consider caching for high volume
/// EXTERNAL: Reads DlpRule list (caller must fetch from DB before calling)
pub fn scan_content(
    rules: &[DlpRule],
    subject: Option<&str>,
    body: Option<&str>,
) -> Vec<DlpScanMatch> {
    let mut matches = Vec::new();

    // Added: Check database-defined rules
    for rule in rules {
        if !rule.active {
            continue;
        }

        match rule.pattern_type.as_str() {
            "regex" => {
                // NOTE: Compile regex per rule — invalid patterns are skipped with a warning
                if let Ok(regex) = Regex::new(&rule.pattern) {
                    scan_with_regex(&regex, rule, subject, body, &mut matches);
                }
            }
            "keyword" => {
                // Added: Simple case-insensitive keyword matching
                scan_with_keyword(&rule.pattern, rule, subject, body, &mut matches);
            }
            "dictionary" => {
                // Added: Dictionary mode — pattern is comma-separated list of keywords
                let keywords: Vec<&str> = rule.pattern.split(',').map(|k| k.trim()).collect();
                for keyword in keywords {
                    if !keyword.is_empty() {
                        scan_with_keyword(keyword, rule, subject, body, &mut matches);
                    }
                }
            }
            _ => {
                // NOTE: Unknown pattern_type — skip silently
            }
        }
    }

    // Added: Check built-in patterns (credit cards, SSNs, IBANs)
    for builtin in get_builtin_patterns() {
        scan_with_builtin_regex(&builtin, subject, body, &mut matches);
    }

    matches
}

/// Added: Scan subject and body with a compiled regex for a database rule
fn scan_with_regex(
    regex: &Regex,
    rule: &DlpRule,
    subject: Option<&str>,
    body: Option<&str>,
    matches: &mut Vec<DlpScanMatch>,
) {
    if rule.apply_to_subject {
        if let Some(subject_text) = subject {
            if let Some(matched) = regex.find(subject_text) {
                matches.push(DlpScanMatch {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    action: rule.action.clone(),
                    severity: rule.severity.clone(),
                    matched_pattern: rule.pattern.clone(),
                    matched_text: matched.as_str().to_string(),
                });
            }
        }
    }

    if rule.apply_to_body {
        if let Some(body_text) = body {
            if let Some(matched) = regex.find(body_text) {
                matches.push(DlpScanMatch {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    action: rule.action.clone(),
                    severity: rule.severity.clone(),
                    matched_pattern: rule.pattern.clone(),
                    matched_text: matched.as_str().to_string(),
                });
            }
        }
    }
}

/// Added: Scan subject and body with a case-insensitive keyword match
fn scan_with_keyword(
    keyword: &str,
    rule: &DlpRule,
    subject: Option<&str>,
    body: Option<&str>,
    matches: &mut Vec<DlpScanMatch>,
) {
    let keyword_lower = keyword.to_lowercase();

    if rule.apply_to_subject {
        if let Some(subject_text) = subject {
            if subject_text.to_lowercase().contains(&keyword_lower) {
                matches.push(DlpScanMatch {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    action: rule.action.clone(),
                    severity: rule.severity.clone(),
                    matched_pattern: keyword.to_string(),
                    matched_text: keyword.to_string(),
                });
            }
        }
    }

    if rule.apply_to_body {
        if let Some(body_text) = body {
            if body_text.to_lowercase().contains(&keyword_lower) {
                matches.push(DlpScanMatch {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    action: rule.action.clone(),
                    severity: rule.severity.clone(),
                    matched_pattern: keyword.to_string(),
                    matched_text: keyword.to_string(),
                });
            }
        }
    }
}

/// Added: Scan subject and body with a built-in regex pattern (credit cards, SSNs, IBANs)
fn scan_with_builtin_regex(
    builtin: &BuiltinPattern,
    subject: Option<&str>,
    body: Option<&str>,
    matches: &mut Vec<DlpScanMatch>,
) {
    // NOTE: Built-in patterns always scan both subject and body
    let builtin_rule_id = uuid::Uuid::nil();

    for text in [subject, body].iter().flatten() {
        if let Some(matched) = builtin.regex.find(text) {
            matches.push(DlpScanMatch {
                rule_id: builtin_rule_id,
                rule_name: builtin.name.clone(),
                action: builtin.action.clone(),
                severity: builtin.severity.clone(),
                matched_pattern: builtin.regex.as_str().to_string(),
                matched_text: matched.as_str().to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // Added: Helper to create a test DlpRule with regex pattern
    fn make_regex_rule(name: &str, pattern: &str, action: DlpAction, severity: DlpSeverity) -> DlpRule {
        DlpRule {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: None,
            pattern: pattern.to_string(),
            pattern_type: "regex".to_string(),
            action,
            severity,
            apply_to_subject: true,
            apply_to_body: true,
            apply_to_attachments: false,
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    // Added: Helper to create a keyword-based DlpRule
    fn make_keyword_rule(name: &str, keyword: &str) -> DlpRule {
        DlpRule {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: None,
            pattern: keyword.to_string(),
            pattern_type: "keyword".to_string(),
            action: DlpAction::Warn,
            severity: DlpSeverity::Medium,
            apply_to_subject: true,
            apply_to_body: true,
            apply_to_attachments: false,
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_builtin_credit_card_detection() {
        // Added: Built-in pattern should detect Visa-format credit card in body
        let rules: Vec<DlpRule> = vec![];
        let matches = scan_content(&rules, None, Some("My card is 4111-1111-1111-1111 please charge it"));
        assert!(
            matches.iter().any(|m| m.rule_name == "Credit Card Number"),
            "Should detect credit card number. Matches: {:?}",
            matches.iter().map(|m| &m.rule_name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_builtin_credit_card_no_separators() {
        // Added: Credit cards without dashes should also be detected
        let rules: Vec<DlpRule> = vec![];
        let matches = scan_content(&rules, None, Some("Card: 4111111111111111"));
        assert!(matches.iter().any(|m| m.rule_name == "Credit Card Number"));
    }

    #[test]
    fn test_builtin_ssn_detection() {
        // Added: Built-in pattern should detect US SSN in XXX-XX-XXXX format
        let rules: Vec<DlpRule> = vec![];
        let matches = scan_content(&rules, None, Some("SSN: 123-45-6789"));
        assert!(matches.iter().any(|m| m.rule_name == "US Social Security Number"));
    }

    #[test]
    fn test_builtin_iban_detection() {
        // Added: Built-in pattern should detect IBAN numbers
        let rules: Vec<DlpRule> = vec![];
        let matches = scan_content(&rules, None, Some("Transfer to DE89370400440532013000"));
        assert!(matches.iter().any(|m| m.rule_name == "IBAN Bank Account"));
    }

    #[test]
    fn test_clean_email_no_matches() {
        // Added: Normal email text should not trigger any matches
        let rules: Vec<DlpRule> = vec![];
        let matches = scan_content(
            &rules,
            Some("Meeting tomorrow"),
            Some("Hi team, let's meet at 3pm in the conference room. Thanks!"),
        );
        assert!(matches.is_empty(), "Clean email should not trigger DLP: {:?}", matches);
    }

    #[test]
    fn test_custom_regex_rule_matches_body() {
        // Added: User-defined regex rule should match in body text
        let rules = vec![make_regex_rule(
            "Account Number",
            r"\bACCT-\d{8}\b",
            DlpAction::Quarantine,
            DlpSeverity::High,
        )];
        let matches = scan_content(&rules, None, Some("Reference: ACCT-12345678"));
        assert!(matches.iter().any(|m| m.rule_name == "Account Number"));
        assert_eq!(matches.iter().find(|m| m.rule_name == "Account Number").unwrap().matched_text, "ACCT-12345678");
    }

    #[test]
    fn test_custom_regex_rule_matches_subject() {
        // Added: Regex rule with apply_to_subject should match in email subject
        let rules = vec![make_regex_rule(
            "Confidential",
            r"(?i)\bconfidential\b",
            DlpAction::Warn,
            DlpSeverity::Medium,
        )];
        let matches = scan_content(&rules, Some("CONFIDENTIAL: Q1 Results"), None);
        assert!(matches.iter().any(|m| m.rule_name == "Confidential"));
    }

    #[test]
    fn test_keyword_rule_case_insensitive() {
        // Added: Keyword matching should be case-insensitive
        let rules = vec![make_keyword_rule("Secret Project", "project phoenix")];
        let matches = scan_content(&rules, None, Some("Update on PROJECT PHOENIX timeline"));
        assert!(matches.iter().any(|m| m.rule_name == "Secret Project"));
    }

    #[test]
    fn test_keyword_rule_no_match() {
        // Added: Keyword that does not appear should not match
        let rules = vec![make_keyword_rule("Secret", "top secret")];
        let matches = scan_content(&rules, Some("Lunch plans"), Some("Where should we eat?"));
        // NOTE: Filter out built-in matches to check only custom rule
        let custom_matches: Vec<_> = matches.iter().filter(|m| m.rule_name == "Secret").collect();
        assert!(custom_matches.is_empty());
    }

    #[test]
    fn test_dictionary_rule_matches_any_keyword() {
        // Added: Dictionary pattern (comma-separated) should match any keyword in the list
        let mut rule = make_keyword_rule("PII Keywords", "passport, driver license, social security");
        rule.pattern_type = "dictionary".to_string();
        let rules = vec![rule];
        let matches = scan_content(&rules, None, Some("Please send your driver license copy"));
        assert!(matches.iter().any(|m| m.rule_name == "PII Keywords"));
    }

    #[test]
    fn test_inactive_rule_skipped() {
        // Added: Inactive rules should not be checked
        let mut rule = make_regex_rule("Disabled Rule", r"\bSECRET\b", DlpAction::Block, DlpSeverity::Critical);
        rule.active = false;
        let rules = vec![rule];
        let matches = scan_content(&rules, Some("SECRET data"), None);
        // NOTE: Only built-in matches should be present, not the disabled custom rule
        assert!(!matches.iter().any(|m| m.rule_name == "Disabled Rule"));
    }

    #[test]
    fn test_invalid_regex_pattern_skipped() {
        // Added: Invalid regex should not cause a panic — just skip the rule
        let rules = vec![make_regex_rule("Bad Regex", r"[invalid(", DlpAction::Block, DlpSeverity::High)];
        let matches = scan_content(&rules, Some("test"), Some("test body"));
        // Should not panic, and no match from the bad rule
        assert!(!matches.iter().any(|m| m.rule_name == "Bad Regex"));
    }

    #[test]
    fn test_subject_only_scanning() {
        // Added: Rule that only scans subject should not match body content
        let mut rule = make_regex_rule("Subject Only", r"\bURGENT\b", DlpAction::Warn, DlpSeverity::Low);
        rule.apply_to_body = false;
        let rules = vec![rule];

        let matches = scan_content(&rules, Some("Normal subject"), Some("URGENT in body only"));
        assert!(!matches.iter().any(|m| m.rule_name == "Subject Only"));

        let matches = scan_content(&rules, Some("URGENT subject"), Some("Normal body"));
        assert!(matches.iter().any(|m| m.rule_name == "Subject Only"));
    }

    #[test]
    fn test_multiple_rules_multiple_matches() {
        // Added: Multiple rules should produce multiple matches
        let rules = vec![
            make_regex_rule("SSN Custom", r"\d{3}-\d{2}-\d{4}", DlpAction::Block, DlpSeverity::Critical),
            make_keyword_rule("Confidential", "confidential"),
        ];
        let matches = scan_content(
            &rules,
            Some("Confidential: Employee SSN"),
            Some("Employee SSN: 123-45-6789 is confidential"),
        );
        // NOTE: Should match both custom rules plus built-in SSN pattern
        assert!(matches.iter().any(|m| m.rule_name == "SSN Custom"));
        assert!(matches.iter().any(|m| m.rule_name == "Confidential"));
    }

    #[test]
    fn test_builtin_patterns_count() {
        // Added: Verify we have the expected number of built-in patterns
        let builtins = get_builtin_patterns();
        assert_eq!(builtins.len(), 3, "Should have 3 built-in patterns: credit card, SSN, IBAN");
    }

    #[test]
    fn test_credit_card_in_subject() {
        // Added: Built-in credit card detection should work in subject too
        let rules: Vec<DlpRule> = vec![];
        let matches = scan_content(&rules, Some("Payment with card 5500-0000-0000-0004"), None);
        assert!(matches.iter().any(|m| m.rule_name == "Credit Card Number"));
    }

    #[test]
    fn test_attachment_blocked_extension_basic() {
        // Added: .exe attachments should be blocked by the default list
        let names = vec!["payload.exe".to_string()];
        let matches = scan_attachments(&names, DEFAULT_BLOCKED_EXTENSIONS);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_text, "payload.exe");
        assert_eq!(matches[0].action, DlpAction::Block);
    }

    #[test]
    fn test_attachment_blocked_case_insensitive() {
        // Added: Uppercase extensions must still match the lowercase blocklist
        let names = vec!["Invoice.BAT".to_string(), "script.Vbs".to_string()];
        let matches = scan_attachments(&names, DEFAULT_BLOCKED_EXTENSIONS);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_attachment_safe_extensions_pass() {
        // Added: Common safe extensions should not produce matches
        let names = vec![
            "report.pdf".to_string(),
            "photo.jpg".to_string(),
            "notes.txt".to_string(),
            "data.csv".to_string(),
        ];
        let matches = scan_attachments(&names, DEFAULT_BLOCKED_EXTENSIONS);
        assert!(matches.is_empty(), "Safe files triggered DLP: {:?}", matches);
    }

    #[test]
    fn test_attachment_no_extension_passes() {
        // Added: A filename without an extension should NOT be blocked
        let names = vec!["README".to_string(), "Makefile".to_string()];
        let matches = scan_attachments(&names, DEFAULT_BLOCKED_EXTENSIONS);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_attachment_double_extension_uses_last() {
        // Added: report.pdf.exe must be blocked (last extension wins)
        let names = vec!["report.pdf.exe".to_string()];
        let matches = scan_attachments(&names, DEFAULT_BLOCKED_EXTENSIONS);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_text, "report.pdf.exe");
    }

    #[test]
    fn test_attachment_custom_blocked_list_overrides_default() {
        // Added: Operators can pass a tighter list (e.g. block .zip too)
        let names = vec!["archive.zip".to_string(), "doc.pdf".to_string()];
        let custom: &[&str] = &["zip"];
        let matches = scan_attachments(&names, custom);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_text, "archive.zip");
    }

    #[test]
    fn test_is_blocked_attachment_dotfile_safe() {
        // Added: A bare dotfile (".bashrc") has no real extension — must not block
        assert!(!is_blocked_attachment(".bashrc", DEFAULT_BLOCKED_EXTENSIONS));
    }

    #[test]
    fn test_default_blocked_list_covers_common_threats() {
        // Added: Sanity-check that the default list covers the typical malware vectors
        let critical = ["exe", "bat", "cmd", "vbs", "js", "ps1", "jar"];
        for ext in critical {
            assert!(
                DEFAULT_BLOCKED_EXTENSIONS.iter().any(|e| *e == ext),
                "Default blocklist is missing .{ext}"
            );
        }
    }
}
