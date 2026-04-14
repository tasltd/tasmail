// Added: Phishing link scanner service for TMAIL-124 — pure heuristic analysis, no external API calls

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// PURPOSE: Result of scanning an email for phishing indicators
/// CONSTRAINTS: risk_score is 0-100; suspicious_links contains per-URL detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub suspicious_links: Vec<SuspiciousLink>,
    pub suspicious_sender: bool,
    pub spoofed_display_name: bool,
    pub risk_score: i32,
}

/// Added: Detail about a single suspicious URL found in the email body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspiciousLink {
    pub url: String,
    pub display_text: String,
    pub reasons: Vec<String>,
}

// Added: Pre-compiled regexes for URL extraction from HTML email bodies
static ANCHOR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<a\s[^>]*href\s*=\s*["']([^"']+)["'][^>]*>(.*?)</a>"#).unwrap()
});

static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[^\s<>"']+[^\s<>"',.]"#).unwrap()
});

// Added: Regex to detect IP-address-based URLs (e.g., http://192.168.1.1/phish)
static IP_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"#).unwrap()
});

// Added: Regex to detect mixed Unicode scripts (homograph attack indicator)
static MIXED_SCRIPT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // NOTE: Detects Cyrillic characters mixed into otherwise-Latin domain names
    Regex::new(r"[\u{0400}-\u{04FF}]").unwrap()
});

// Added: Known suspicious TLDs commonly used in phishing campaigns
const SUSPICIOUS_TLDS: &[&str] = &[".tk", ".ml", ".ga", ".cf", ".gq", ".xyz", ".top", ".buzz"];

// Added: Known URL shortener domains that obscure final destination
const URL_SHORTENERS: &[&str] = &[
    "bit.ly", "tinyurl.com", "t.co", "goo.gl", "ow.ly", "is.gd",
    "buff.ly", "rebrand.ly", "short.io", "cutt.ly",
];

// Added: Known brand names that phishers commonly spoof in display names
const KNOWN_BRANDS: &[&str] = &[
    "paypal", "apple", "google", "microsoft", "amazon", "netflix",
    "facebook", "instagram", "whatsapp", "bank", "chase", "wells fargo",
    "citibank", "dhl", "fedex", "ups", "usps",
];

/// PURPOSE: Scan an email for phishing indicators using heuristic analysis only
/// CONSTRAINTS: No external API calls — all checks are local regex/string matching
pub fn scan_email(html_body: &str, sender_display_name: &str, sender_email: &str) -> ScanResult {
    let mut suspicious_links = Vec::new();

    // Added: Extract all <a href="...">display text</a> pairs from HTML body
    for cap in ANCHOR_REGEX.captures_iter(html_body) {
        let href = cap.get(1).map_or("", |m: regex::Match<'_>| m.as_str());
        let display_text = cap.get(2).map_or("", |m: regex::Match<'_>| m.as_str());
        // NOTE: Strip HTML tags from display text for clean comparison
        let clean_display = strip_html_tags(display_text);

        let mut reasons = Vec::new();

        // Added: Check for mismatched display text vs actual URL
        check_display_mismatch(href, &clean_display, &mut reasons);

        // Added: Check for suspicious TLDs
        check_suspicious_tld(href, &mut reasons);

        // Added: Check for IP-address-based URLs
        check_ip_url(href, &mut reasons);

        // Added: Check for known URL shorteners
        check_url_shortener(href, &mut reasons);

        // Added: Check for homograph attacks (mixed Unicode scripts in domain)
        check_homograph(href, &mut reasons);

        // Added: Check for excessive subdomains (more than 3 levels)
        check_excessive_subdomains(href, &mut reasons);

        if !reasons.is_empty() {
            suspicious_links.push(SuspiciousLink {
                url: href.to_string(),
                display_text: clean_display,
                reasons,
            });
        }
    }

    // Added: Check sender for brand spoofing in display name
    let spoofed_display_name = check_sender_spoofing(sender_display_name, sender_email);
    let suspicious_sender = spoofed_display_name;

    // Added: Calculate composite risk score from all indicators
    let risk_score = calculate_risk_score(&suspicious_links, suspicious_sender, spoofed_display_name);

    ScanResult {
        suspicious_links,
        suspicious_sender,
        spoofed_display_name,
        risk_score,
    }
}

/// Added: Strip HTML tags from anchor display text for clean comparison
fn strip_html_tags(text: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    tag_re.replace_all(text, "").trim().to_string()
}

/// Added: Check if display text looks like a URL but differs from the actual href
fn check_display_mismatch(href: &str, display_text: &str, reasons: &mut Vec<String>) {
    // NOTE: Only flag if display text itself looks like a URL (contains a domain)
    if URL_REGEX.is_match(display_text) || display_text.contains('.') && !display_text.contains(' ') {
        let display_domain = extract_domain(display_text);
        let href_domain = extract_domain(href);

        if !display_domain.is_empty() && !href_domain.is_empty() && display_domain != href_domain {
            reasons.push(format!(
                "Display text '{}' shows domain '{}' but links to '{}'",
                display_text, display_domain, href_domain
            ));
        }
    }
}

/// Added: Check if URL uses a known suspicious TLD
fn check_suspicious_tld(url: &str, reasons: &mut Vec<String>) {
    let domain = extract_domain(url).to_lowercase();
    for tld in SUSPICIOUS_TLDS {
        if domain.ends_with(tld) {
            reasons.push(format!("Suspicious TLD: {}", tld));
            break;
        }
    }
}

/// Added: Check if URL uses a raw IP address instead of a domain name
fn check_ip_url(url: &str, reasons: &mut Vec<String>) {
    if IP_URL_REGEX.is_match(url) {
        reasons.push("URL uses IP address instead of domain name".to_string());
    }
}

/// Added: Check if URL points to a known URL shortener service
fn check_url_shortener(url: &str, reasons: &mut Vec<String>) {
    let domain = extract_domain(url).to_lowercase();
    for shortener in URL_SHORTENERS {
        if domain == *shortener || domain.ends_with(&format!(".{}", shortener)) {
            reasons.push(format!("URL shortener detected: {}", shortener));
            break;
        }
    }
}

/// Added: Check for homograph attacks using mixed Unicode scripts in domain
fn check_homograph(url: &str, reasons: &mut Vec<String>) {
    let domain = extract_domain(url);
    if MIXED_SCRIPT_REGEX.is_match(&domain) {
        reasons.push("Possible homograph attack: domain contains mixed Unicode scripts".to_string());
    }
}

/// Added: Check for excessive subdomains (more than 3 levels indicates phishing)
fn check_excessive_subdomains(url: &str, reasons: &mut Vec<String>) {
    let domain = extract_domain(url);
    // NOTE: Count dots to determine subdomain depth — "a.b.c.d.com" has 4 dots = 5 levels
    let dot_count = domain.chars().filter(|c| *c == '.').count();
    if dot_count > 3 {
        reasons.push(format!(
            "Excessive subdomains ({} levels) — common phishing technique",
            dot_count + 1
        ));
    }
}

/// Added: Check if sender display name spoofs a known brand
fn check_sender_spoofing(display_name: &str, email: &str) -> bool {
    let name_lower = display_name.to_lowercase();
    let email_lower = email.to_lowercase();

    for brand in KNOWN_BRANDS {
        if name_lower.contains(brand) {
            // NOTE: If display name contains "paypal" but email domain is not paypal.com, it's suspicious
            let brand_domain = format!("{}.", brand);
            let at_brand = format!("@{}", brand);
            if !email_lower.contains(&brand_domain) && !email_lower.contains(&at_brand) {
                return true;
            }
        }
    }
    false
}

/// Added: Extract domain from a URL string (strips protocol, path, port)
fn extract_domain(url: &str) -> String {
    let without_protocol = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    // NOTE: Take everything before the first / or : (to strip path and port)
    without_protocol
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Added: Calculate composite risk score from all detected indicators (0-100)
fn calculate_risk_score(
    suspicious_links: &[SuspiciousLink],
    suspicious_sender: bool,
    spoofed_display_name: bool,
) -> i32 {
    let mut score: i32 = 0;

    // Each suspicious link contributes based on number of reasons
    for link in suspicious_links {
        // NOTE: More reasons on a single link = higher confidence it's phishing
        score += (link.reasons.len() as i32) * 15;
    }

    // Sender spoofing is a strong phishing signal
    if suspicious_sender {
        score += 25;
    }
    if spoofed_display_name {
        score += 20;
    }

    // Clamp to 0-100
    score.clamp(0, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_email_no_suspicious_links() {
        // Added: A legitimate email should produce risk_score 0
        let html = r#"<p>Hello! Visit <a href="https://example.com">our website</a></p>"#;
        let result = scan_email(html, "John", "john@example.com");
        assert_eq!(result.risk_score, 0);
        assert!(result.suspicious_links.is_empty());
        assert!(!result.suspicious_sender);
    }

    #[test]
    fn test_mismatched_display_text_vs_href() {
        // Added: Display text shows paypal.com but href points to evil.com
        let html = r#"<a href="https://evil.com/steal">paypal.com</a>"#;
        let result = scan_email(html, "Someone", "someone@somewhere.com");
        assert!(!result.suspicious_links.is_empty());
        assert!(result.suspicious_links[0].reasons.iter().any(|r| r.contains("Display text")));
        assert!(result.risk_score > 0);
    }

    #[test]
    fn test_suspicious_tld_detection() {
        // Added: URLs with .tk, .ml, etc. should be flagged
        let html = r#"<a href="https://login-secure.tk/verify">Click here</a>"#;
        let result = scan_email(html, "Test", "test@test.com");
        assert!(!result.suspicious_links.is_empty());
        assert!(result.suspicious_links[0].reasons.iter().any(|r| r.contains("Suspicious TLD")));
    }

    #[test]
    fn test_ip_address_url_detection() {
        // Added: URLs using raw IP addresses should be flagged
        let html = r#"<a href="http://192.168.1.100/login">Login now</a>"#;
        let result = scan_email(html, "Admin", "admin@company.com");
        assert!(!result.suspicious_links.is_empty());
        assert!(result.suspicious_links[0].reasons.iter().any(|r| r.contains("IP address")));
    }

    #[test]
    fn test_url_shortener_detection() {
        // Added: bit.ly and similar shorteners should be flagged
        let html = r#"<a href="https://bit.ly/abc123">Check this out</a>"#;
        let result = scan_email(html, "Friend", "friend@gmail.com");
        assert!(!result.suspicious_links.is_empty());
        assert!(result.suspicious_links[0].reasons.iter().any(|r| r.contains("URL shortener")));
    }

    #[test]
    fn test_homograph_attack_detection() {
        // Added: Domain with Cyrillic characters mixed into Latin text
        let html = r#"<a href="https://pа\u{0443}pal.com/login">Verify account</a>"#;
        let result = scan_email(html, "PayPal", "noreply@phisher.com");
        // NOTE: The href domain contains Cyrillic а (U+0430) and у (U+0443)
        assert!(result.suspicious_links.iter().any(|l|
            l.reasons.iter().any(|r| r.contains("homograph"))
        ));
    }

    #[test]
    fn test_excessive_subdomains_detection() {
        // Added: More than 3 subdomain levels is suspicious
        let html = r#"<a href="https://login.secure.account.verify.evil.com/steal">Update password</a>"#;
        let result = scan_email(html, "Support", "support@evil.com");
        assert!(!result.suspicious_links.is_empty());
        assert!(result.suspicious_links[0].reasons.iter().any(|r| r.contains("Excessive subdomains")));
    }

    #[test]
    fn test_sender_brand_spoofing() {
        // Added: Display name contains "PayPal" but email is from a different domain
        let html = r#"<p>Please verify your account</p>"#;
        let result = scan_email(html, "PayPal Security", "security@phisher.xyz");
        assert!(result.spoofed_display_name);
        assert!(result.suspicious_sender);
        assert!(result.risk_score > 0);
    }

    #[test]
    fn test_legitimate_brand_sender_not_flagged() {
        // Added: Display name "PayPal" with matching paypal.com domain should NOT be flagged
        let html = r#"<p>Your receipt is ready</p>"#;
        let result = scan_email(html, "PayPal", "noreply@paypal.com");
        assert!(!result.spoofed_display_name);
        assert!(!result.suspicious_sender);
    }

    #[test]
    fn test_multiple_indicators_increase_risk_score() {
        // Added: Email with multiple phishing signals should have a high risk score
        let html = r#"
            <a href="http://192.168.1.1/login">paypal.com</a>
            <a href="https://secure.login.verify.account.evil.tk/steal">Click here</a>
        "#;
        let result = scan_email(html, "Apple Support", "help@phisher.xyz");
        // NOTE: Expect high risk: IP URL + mismatch + suspicious TLD + excessive subdomains + sender spoofing
        assert!(result.risk_score >= 60, "Risk score should be high for multiple indicators, got {}", result.risk_score);
        assert!(result.suspicious_links.len() >= 2);
    }
}
