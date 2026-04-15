// Added: Deliverability checking service for TMAIL-39
// PURPOSE: Provides DNS record validation, blacklist checking, and TLS verification
// EXTERNAL: Uses std::net for DNS lookups (no external DNS crate needed for basic checks)

use std::net::{IpAddr, ToSocketAddrs};
use std::process::Command;

use crate::models::deliverability::{CheckResult, DeliverabilityReport};

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
}
