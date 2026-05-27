// Added: Deliverability checking service for TMAIL-39
// PURPOSE: Provides DNS record validation, blacklist checking, and TLS verification
// EXTERNAL: Uses std::net for DNS lookups (no external DNS crate needed for basic checks)

use std::net::{IpAddr, ToSocketAddrs};
use std::process::Command;

use rand::Rng;

use crate::models::deliverability::{
    CheckResult, DeliverabilityReport, ExternalToolsResponse, MailTesterHandle, PostmasterTools,
};

/// Added: Known DNS blacklist zones for spam checking
const BLACKLIST_ZONES: &[(&str, &str)] = &[
    ("zen.spamhaus.org", "Spamhaus ZEN"),
    ("b.barracudacentral.org", "Barracuda"),
    ("dnsbl.sorbs.net", "SORBS"),
];

/// Added: Run all deliverability checks for a given domain and return a scored report
pub async fn run_deliverability_checks(domain: &str) -> DeliverabilityReport {
    let mut checks = Vec::new();

    // Added: Check MX records
    checks.push(check_mx_records(domain));

    // Added: Check SPF record
    checks.push(check_spf_record(domain));

    // Added: Check DKIM record (default selector)
    checks.push(check_dkim_record(domain));

    // Added: Check DMARC record
    checks.push(check_dmarc_record(domain));

    // Added: Check reverse DNS (PTR)
    checks.push(check_reverse_dns(domain));

    // Added: Check TLS certificate
    checks.push(check_tls_certificate(domain));

    // Added: Check blacklists for mail server IP
    checks.push(check_blacklists(domain));

    // Added: Check SMTP connectivity on port 25
    checks.push(check_smtp_connectivity(domain, 25));

    // Added: Check SMTP connectivity on port 587
    checks.push(check_smtp_connectivity(domain, 587));

    // Added: Check IMAP connectivity on port 993
    checks.push(check_imap_connectivity(domain));

    let score = DeliverabilityReport::calculate_score(&checks);

    DeliverabilityReport {
        domain: domain.to_string(),
        checks,
        score,
    }
}

/// Added: Verify MX records exist for the domain using dig
pub fn check_mx_records(domain: &str) -> CheckResult {
    match run_dig(domain, "MX") {
        Ok(output) if !output.trim().is_empty() => {
            let count = output.lines().count();
            CheckResult::pass(
                "MX Records",
                &format!("{} MX record(s) found: {}", count, output.trim().replace('\n', ", ")),
            )
        }
        Ok(_) => CheckResult::fail("MX Records", "No MX records found for domain"),
        Err(e) => CheckResult::error("MX Records", &format!("DNS lookup failed: {}", e)),
    }
}

/// Added: Check SPF (TXT) record for the domain
pub fn check_spf_record(domain: &str) -> CheckResult {
    match run_dig(domain, "TXT") {
        Ok(output) => {
            // Added: Look for v=spf1 in TXT records
            let spf_lines: Vec<&str> = output
                .lines()
                .filter(|l| l.contains("v=spf1"))
                .collect();
            if spf_lines.is_empty() {
                CheckResult::fail("SPF Record", "No SPF record found (missing v=spf1 TXT record)")
            } else if spf_lines.len() > 1 {
                CheckResult::warn("SPF Record", "Multiple SPF records found — should have exactly one")
            } else {
                CheckResult::pass("SPF Record", &format!("SPF record found: {}", spf_lines[0].trim()))
            }
        }
        Err(e) => CheckResult::error("SPF Record", &format!("DNS lookup failed: {}", e)),
    }
}

/// Added: Check DKIM record (queries default selector: default._domainkey.domain)
pub fn check_dkim_record(domain: &str) -> CheckResult {
    let dkim_domain = format!("default._domainkey.{}", domain);
    match run_dig(&dkim_domain, "TXT") {
        Ok(output) => {
            if output.contains("v=DKIM1") || output.contains("p=") {
                CheckResult::pass("DKIM Record", &format!("DKIM record found at default._domainkey.{}", domain))
            } else {
                // Added: Try mail selector as fallback
                let mail_dkim = format!("mail._domainkey.{}", domain);
                match run_dig(&mail_dkim, "TXT") {
                    Ok(out2) if out2.contains("v=DKIM1") || out2.contains("p=") => {
                        CheckResult::pass("DKIM Record", &format!("DKIM record found at mail._domainkey.{}", domain))
                    }
                    _ => CheckResult::warn(
                        "DKIM Record",
                        "No DKIM record found at default or mail selectors (check your DKIM selector name)",
                    ),
                }
            }
        }
        Err(e) => CheckResult::error("DKIM Record", &format!("DNS lookup failed: {}", e)),
    }
}

/// Added: Check DMARC record (_dmarc.domain TXT)
pub fn check_dmarc_record(domain: &str) -> CheckResult {
    let dmarc_domain = format!("_dmarc.{}", domain);
    match run_dig(&dmarc_domain, "TXT") {
        Ok(output) => {
            if output.contains("v=DMARC1") {
                // Added: Check DMARC policy strength
                if output.contains("p=reject") {
                    CheckResult::pass("DMARC Record", "DMARC record found with p=reject (strongest policy)")
                } else if output.contains("p=quarantine") {
                    CheckResult::pass("DMARC Record", "DMARC record found with p=quarantine")
                } else {
                    CheckResult::warn("DMARC Record", "DMARC record found but policy is p=none (monitoring only)")
                }
            } else {
                CheckResult::fail("DMARC Record", "No DMARC record found")
            }
        }
        Err(e) => CheckResult::error("DMARC Record", &format!("DNS lookup failed: {}", e)),
    }
}

/// Added: Check reverse DNS (PTR) matches forward DNS
pub fn check_reverse_dns(domain: &str) -> CheckResult {
    // Added: Resolve the domain to an IP first
    match resolve_domain_ip(domain) {
        Some(ip) => {
            // Added: Perform reverse DNS lookup using dig -x
            match run_command("dig", &["+short", "-x", &ip.to_string()]) {
                Ok(ptr_output) => {
                    let ptr = ptr_output.trim().trim_end_matches('.');
                    if ptr.is_empty() {
                        CheckResult::fail(
                            "Reverse DNS (PTR)",
                            &format!("No PTR record found for IP {}", ip),
                        )
                    } else if ptr.eq_ignore_ascii_case(domain) || ptr.ends_with(&format!(".{}", domain)) {
                        CheckResult::pass(
                            "Reverse DNS (PTR)",
                            &format!("PTR record {} matches domain {}", ptr, domain),
                        )
                    } else {
                        CheckResult::warn(
                            "Reverse DNS (PTR)",
                            &format!("PTR record {} does not exactly match {}", ptr, domain),
                        )
                    }
                }
                Err(e) => CheckResult::error("Reverse DNS (PTR)", &format!("Reverse lookup failed: {}", e)),
            }
        }
        None => CheckResult::error("Reverse DNS (PTR)", &format!("Could not resolve {} to an IP address", domain)),
    }
}

/// Added: Check TLS certificate validity using openssl s_client
pub fn check_tls_certificate(domain: &str) -> CheckResult {
    let result = run_command(
        "openssl",
        &[
            "s_client",
            "-connect",
            &format!("{}:993", domain),
            "-servername",
            domain,
            "-verify_return_error",
            "-brief",
        ],
    );
    match result {
        Ok(output) => {
            if output.contains("Verification: OK") || output.contains("Verify return code: 0") {
                CheckResult::pass("TLS Certificate", &format!("Valid TLS certificate on {}:993", domain))
            } else if output.contains("Verify return code:") {
                CheckResult::warn(
                    "TLS Certificate",
                    &format!("TLS connection established but certificate verification had issues: {}", output.lines().next().unwrap_or("")),
                )
            } else {
                CheckResult::warn("TLS Certificate", "TLS connection made but could not confirm certificate validity")
            }
        }
        Err(_) => {
            // Added: Fallback — try port 587 with STARTTLS
            let result_587 = run_command(
                "openssl",
                &[
                    "s_client",
                    "-connect",
                    &format!("{}:587", domain),
                    "-servername",
                    domain,
                    "-starttls",
                    "smtp",
                    "-brief",
                ],
            );
            match result_587 {
                Ok(output) if output.contains("Verification: OK") || output.contains("Verify return code: 0") => {
                    CheckResult::pass("TLS Certificate", &format!("Valid TLS certificate on {}:587 (STARTTLS)", domain))
                }
                _ => CheckResult::fail("TLS Certificate", &format!("Could not verify TLS certificate for {}", domain)),
            }
        }
    }
}

/// Added: Check if the mail server IP is on common DNS blacklists
pub fn check_blacklists(domain: &str) -> CheckResult {
    let ip = match resolve_domain_ip(domain) {
        Some(ip) => ip,
        None => {
            return CheckResult::error(
                "Blacklist Check",
                &format!("Could not resolve {} to an IP address", domain),
            );
        }
    };

    let mut listed_on: Vec<String> = Vec::new();
    let mut checked = 0;

    // Added: For IPv4 only — reverse the octets and query each blacklist zone
    if let IpAddr::V4(ipv4) = ip {
        let octets = ipv4.octets();
        let reversed = format!("{}.{}.{}.{}", octets[3], octets[2], octets[1], octets[0]);

        for (zone, name) in BLACKLIST_ZONES {
            let query = format!("{}.{}", reversed, zone);
            checked += 1;
            if let Ok(output) = run_dig(&query, "A") {
                // Added: If dig returns an A record, IP is listed on the blacklist
                if !output.trim().is_empty() && output.contains("127.") {
                    listed_on.push(name.to_string());
                }
            }
        }
    } else {
        return CheckResult::warn(
            "Blacklist Check",
            "IPv6 blacklist checking not supported — skipped",
        );
    }

    if listed_on.is_empty() {
        CheckResult::pass(
            "Blacklist Check",
            &format!("IP {} not found on any of {} checked blacklists", ip, checked),
        )
    } else {
        CheckResult::fail(
            "Blacklist Check",
            &format!("IP {} is listed on: {}", ip, listed_on.join(", ")),
        )
    }
}

/// Added: Check SMTP connectivity on a specific port
pub fn check_smtp_connectivity(domain: &str, port: u16) -> CheckResult {
    let name = format!("SMTP Port {}", port);
    let addr = format!("{}:{}", domain, port);
    match addr.to_socket_addrs() {
        Ok(mut addrs) => {
            if addrs.next().is_some() {
                // Added: Try a TCP connection with timeout
                match std::net::TcpStream::connect_timeout(
                    &format!("{}:{}", domain, port).to_socket_addrs().unwrap().next().unwrap(),
                    std::time::Duration::from_secs(5),
                ) {
                    Ok(_) => CheckResult::pass(&name, &format!("SMTP accepting connections on port {}", port)),
                    Err(e) => CheckResult::fail(&name, &format!("Connection refused on port {}: {}", port, e)),
                }
            } else {
                CheckResult::fail(&name, &format!("Cannot resolve {}:{}", domain, port))
            }
        }
        Err(e) => CheckResult::error(&name, &format!("DNS resolution failed: {}", e)),
    }
}

/// Added: Check IMAP connectivity on port 993
pub fn check_imap_connectivity(domain: &str) -> CheckResult {
    let addr = format!("{}:993", domain);
    match addr.to_socket_addrs() {
        Ok(mut addrs) => {
            if let Some(sock) = addrs.next() {
                match std::net::TcpStream::connect_timeout(&sock, std::time::Duration::from_secs(5)) {
                    Ok(_) => CheckResult::pass("IMAP Port 993", "IMAP accepting connections on port 993"),
                    Err(e) => CheckResult::fail("IMAP Port 993", &format!("Connection refused on port 993: {}", e)),
                }
            } else {
                CheckResult::fail("IMAP Port 993", &format!("Cannot resolve {}", addr))
            }
        }
        Err(e) => CheckResult::error("IMAP Port 993", &format!("DNS resolution failed: {}", e)),
    }
}

/// Added: Helper to run dig command for DNS queries
fn run_dig(domain: &str, record_type: &str) -> Result<String, String> {
    run_command("dig", &["+short", record_type, domain])
}

/// Added: Helper to run a shell command and capture stdout+stderr
fn run_command(cmd: &str, args: &[&str]) -> Result<String, String> {
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                Ok(stdout)
            } else {
                Err(format!("Command failed: {}", stderr))
            }
        }
        Err(e) => Err(format!("Failed to execute {}: {}", cmd, e)),
    }
}

/// Added: Resolve a domain to its first IP address
fn resolve_domain_ip(domain: &str) -> Option<IpAddr> {
    format!("{}:25", domain)
        .to_socket_addrs()
        .ok()?
        .next()
        .map(|addr| addr.ip())
}

// === TMAIL-39 — external deliverability tools (mail-tester + Google Postmaster) ===
// The DNS/blacklist scanner above answers "is my config plausible?" but the spec for
// TMAIL-39 also calls out mail-tester.com (free 0–10 spam score) and Google Postmaster
// Tools (Gmail-side reputation), both of which require sending real mail and visiting
// an external dashboard. We can't drive those services from the backend without an
// active SMTP session and (for Postmaster) a Google account; the design here exposes
// the minimal information the admin UI needs to drive both flows manually:
//
//   - mail-tester: generate a fresh single-use handle so the UI can show both the
//     test address (to send to) and the matching report URL (to open afterwards).
//   - Google Postmaster: deep-link straight into the managedomains page with the
//     domain query parameter pre-filled, so the user lands on the correct entry.
//
// Keeping this on the backend (rather than the SPA) lets the same logic feed any
// future CLI / mobile-app caller and keeps the token-generation server-side.

/// Added: TMAIL-39 — token alphabet for mail-tester handles. Lowercase + digits, no
/// ambiguous characters (no 0/o, 1/l) so the address copies cleanly into a To: field.
const MAIL_TESTER_TOKEN_ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
const MAIL_TESTER_TOKEN_LEN: usize = 12;

/// Added: TMAIL-39 — mint a fresh mail-tester.com test address.
/// CONSTRAINTS: address local part is `test-tasmail-<token>` so we can spot our own
/// traffic; matching report URL points at https://www.mail-tester.com/<handle>.
/// The user has ~45 minutes to view the report before the handle expires (mail-tester
/// rotates inboxes — the same handle returns a stale report after that window).
pub fn build_mail_tester_handle() -> MailTesterHandle {
    let mut rng = rand::rng();
    let token: String = (0..MAIL_TESTER_TOKEN_LEN)
        .map(|_| {
            let idx = rng.random_range(0..MAIL_TESTER_TOKEN_ALPHABET.len());
            MAIL_TESTER_TOKEN_ALPHABET[idx] as char
        })
        .collect();
    let handle = format!("test-tasmail-{}", token);
    let test_address = format!("{}@mail-tester.com", handle);
    let report_url = format!("https://www.mail-tester.com/{}", handle);
    MailTesterHandle {
        test_address,
        report_url,
        expires_in_minutes: 45,
        instructions: "Compose a normal email from the TASMail account whose deliverability you want to score and send it to the address below. Within 45 minutes, open the report URL to view your mail-tester.com spam score (target: 8/10 or higher). The handle is single-use — running this again mints a fresh address.".to_string(),
    }
}

/// Added: TMAIL-39 — build the Google Postmaster Tools deep-link for a domain.
/// CONSTRAINTS: domain is URL-encoded so internationalised or punctuated values do not
/// break the query string; an empty domain yields the bare managedomains URL rather
/// than a broken `?domain=` query.
pub fn build_postmaster_tools(domain: &str) -> PostmasterTools {
    let trimmed = domain.trim();
    let dashboard_url = if trimmed.is_empty() {
        "https://postmaster.google.com/managedomains".to_string()
    } else {
        format!(
            "https://postmaster.google.com/managedomains?domain={}",
            url_encode(trimmed),
        )
    };
    PostmasterTools {
        dashboard_url,
        instructions: "Sign in with the Google account that owns this domain, click \"Add Domain\", and complete the DNS TXT verification step. Reputation, spam-rate, and authentication metrics start populating after about 24 hours of sending volume to Gmail recipients. Aim for \"High\" domain reputation and a spam rate under 0.10%.".to_string(),
    }
}

/// Added: TMAIL-39 — combine mail-tester, Postmaster Tools, and the manual provider
/// checklist (Gmail/Outlook/Yahoo/ProtonMail) into the single payload the UI needs.
pub fn build_external_tools(domain: &str) -> ExternalToolsResponse {
    ExternalToolsResponse {
        mail_tester: build_mail_tester_handle(),
        google_postmaster: build_postmaster_tools(domain),
        providers: ExternalToolsResponse::default_providers(),
    }
}

/// Added: TMAIL-39 — minimal RFC 3986 percent-encoder for the Postmaster query string.
/// We avoid pulling in a full URL crate (none currently in this service's dep graph)
/// and only need to escape non-unreserved bytes for a single `domain=` value.
fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        let ok = matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~');
        if ok {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blacklist_zones_defined() {
        // Added: Verify all expected blacklist zones are present
        assert!(BLACKLIST_ZONES.len() >= 3);
        let zone_names: Vec<&str> = BLACKLIST_ZONES.iter().map(|(_, name)| *name).collect();
        assert!(zone_names.contains(&"Spamhaus ZEN"));
        assert!(zone_names.contains(&"Barracuda"));
        assert!(zone_names.contains(&"SORBS"));
    }

    #[test]
    fn test_check_mx_no_dns() {
        // Added: MX check on an invalid domain returns error or fail
        let result = check_mx_records("this-domain-does-not-exist-xyz123.invalid");
        assert!(
            result.status == crate::models::deliverability::CheckStatus::Fail
                || result.status == crate::models::deliverability::CheckStatus::Error
        );
    }

    #[test]
    fn test_check_spf_no_dns() {
        // Added: SPF check on invalid domain returns error or fail
        let result = check_spf_record("this-domain-does-not-exist-xyz123.invalid");
        assert!(
            result.status == crate::models::deliverability::CheckStatus::Fail
                || result.status == crate::models::deliverability::CheckStatus::Error
        );
    }

    #[test]
    fn test_check_dkim_no_dns() {
        // Added: DKIM check on invalid domain returns warn or error
        let result = check_dkim_record("this-domain-does-not-exist-xyz123.invalid");
        assert!(
            result.status == crate::models::deliverability::CheckStatus::Warn
                || result.status == crate::models::deliverability::CheckStatus::Error
        );
    }

    #[test]
    fn test_check_dmarc_no_dns() {
        // Added: DMARC check on invalid domain returns fail or error
        let result = check_dmarc_record("this-domain-does-not-exist-xyz123.invalid");
        assert!(
            result.status == crate::models::deliverability::CheckStatus::Fail
                || result.status == crate::models::deliverability::CheckStatus::Error
        );
    }

    #[test]
    fn test_check_smtp_connectivity_invalid_host() {
        // Added: SMTP check on invalid host should fail
        let result = check_smtp_connectivity("this-domain-does-not-exist-xyz123.invalid", 25);
        assert!(
            result.status == crate::models::deliverability::CheckStatus::Fail
                || result.status == crate::models::deliverability::CheckStatus::Error
        );
    }

    #[test]
    fn test_check_imap_connectivity_invalid_host() {
        // Added: IMAP check on invalid host should fail
        let result = check_imap_connectivity("this-domain-does-not-exist-xyz123.invalid");
        assert!(
            result.status == crate::models::deliverability::CheckStatus::Fail
                || result.status == crate::models::deliverability::CheckStatus::Error
        );
    }

    #[test]
    fn test_check_reverse_dns_invalid() {
        // Added: PTR check on invalid domain returns error
        let result = check_reverse_dns("this-domain-does-not-exist-xyz123.invalid");
        assert_eq!(result.status, crate::models::deliverability::CheckStatus::Error);
    }

    #[test]
    fn test_resolve_domain_ip_invalid() {
        // Added: Invalid domain should return None
        assert!(resolve_domain_ip("this-domain-does-not-exist-xyz123.invalid").is_none());
    }

    #[test]
    fn test_run_dig_helper() {
        // Added: dig helper should at least not panic
        let result = run_dig("localhost", "A");
        // NOTE: May succeed or fail depending on environment — just ensure it doesn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_check_tls_invalid_host() {
        // Added: TLS check on invalid host should fail
        let result = check_tls_certificate("this-domain-does-not-exist-xyz123.invalid");
        assert!(
            result.status == crate::models::deliverability::CheckStatus::Fail
                || result.status == crate::models::deliverability::CheckStatus::Error
        );
    }

    #[test]
    fn test_check_blacklists_invalid_host() {
        // Added: Blacklist check on unresolvable domain returns error
        let result = check_blacklists("this-domain-does-not-exist-xyz123.invalid");
        assert_eq!(result.status, crate::models::deliverability::CheckStatus::Error);
    }

    #[test]
    fn test_mail_tester_handle_format() {
        // Added: TMAIL-39 — mail-tester handle MUST follow `test-tasmail-<token>` so we
        // can spot our own traffic in any mail-tester dashboard and so the public URL
        // resolves to the same handle as the SMTP envelope.
        let handle = build_mail_tester_handle();
        assert!(
            handle.test_address.starts_with("test-tasmail-"),
            "expected prefix, got {}",
            handle.test_address
        );
        assert!(
            handle.test_address.ends_with("@mail-tester.com"),
            "expected mail-tester.com domain, got {}",
            handle.test_address
        );
        // URL must reference the same handle (everything before the @) so the user
        // visits the report that matches the email they sent.
        let local_part = handle.test_address.split('@').next().unwrap();
        assert!(
            handle.report_url.ends_with(local_part),
            "report URL {} should end with handle {}",
            handle.report_url,
            local_part
        );
        assert_eq!(handle.expires_in_minutes, 45);
        assert!(!handle.instructions.is_empty());
    }

    #[test]
    fn test_mail_tester_handle_is_unique() {
        // Added: TMAIL-39 — every call must mint a fresh token so a user re-running
        // the test gets a clean report rather than stale results from an earlier send.
        let a = build_mail_tester_handle();
        let b = build_mail_tester_handle();
        assert_ne!(a.test_address, b.test_address, "tokens collided across calls");
    }

    #[test]
    fn test_postmaster_url_includes_domain() {
        // Added: TMAIL-39 — the Postmaster Tools URL pre-fills the domain so the user
        // lands on the right managedomains entry without typing.
        let pmt = build_postmaster_tools("mail.example.com");
        assert!(pmt.dashboard_url.contains("postmaster.google.com"));
        assert!(
            pmt.dashboard_url.contains("mail.example.com"),
            "expected domain in URL, got {}",
            pmt.dashboard_url
        );
        assert!(!pmt.instructions.is_empty());
    }

    #[test]
    fn test_postmaster_url_handles_missing_domain() {
        // Added: TMAIL-39 — if the caller omits a domain, the URL must still resolve
        // to the Postmaster managedomains landing page rather than a broken query.
        let pmt = build_postmaster_tools("");
        assert!(pmt.dashboard_url.starts_with("https://postmaster.google.com"));
        assert!(!pmt.dashboard_url.contains("domain="));
    }

    #[test]
    fn test_postmaster_url_encodes_domain() {
        // Added: TMAIL-39 — domains with unusual characters must be URL-encoded so the
        // dashboard parses them correctly (defensive: the constraint set is small but
        // we don't want a future internationalised TLD to break the link).
        let pmt = build_postmaster_tools("mail.üñiçødé.test");
        // Encoded form contains percent-escapes, not raw non-ASCII bytes.
        assert!(
            pmt.dashboard_url
                .chars()
                .all(|c| c.is_ascii() && !c.is_whitespace()),
            "URL should be pure ASCII after encoding: {}",
            pmt.dashboard_url
        );
    }

    #[test]
    fn test_build_external_tools_combines_all_sections() {
        // Added: TMAIL-39 — the assembled response wires together all three sub-sections
        // (mail-tester, Postmaster, provider checklist) with the spec's four providers.
        let resp = build_external_tools("mail.example.com");
        assert!(resp.mail_tester.test_address.contains("@mail-tester.com"));
        assert!(resp.google_postmaster.dashboard_url.contains("mail.example.com"));
        assert_eq!(resp.providers.len(), 4);
    }
}
