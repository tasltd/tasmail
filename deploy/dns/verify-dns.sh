#!/usr/bin/env bash
# Added: DNS record verification script for TASMail (TMAIL-13)
# Checks all required and recommended DNS records for a mail domain
#
# Usage: ./verify-dns.sh example.com [mail-hostname]
#   example.com     — the mail domain to verify
#   mail-hostname   — optional, defaults to "mail.example.com"
#
# Dependencies: dig (dnsutils/bind-utils), host
# Exit codes: 0 = all pass, 1 = one or more failures

set -euo pipefail

# Added: Color codes for pass/fail output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

# Added: Counters for summary
PASS=0
FAIL=0
WARN=0

pass() {
    echo -e "  ${GREEN}[PASS]${RESET} $1"
    ((PASS++))
}

fail() {
    echo -e "  ${RED}[FAIL]${RESET} $1"
    ((FAIL++))
}

warn() {
    echo -e "  ${YELLOW}[WARN]${RESET} $1"
    ((WARN++))
}

# Added: Validate arguments
if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <domain> [mail-hostname]"
    echo "  Example: $0 example.com"
    echo "  Example: $0 example.com mail.example.com"
    exit 1
fi

DOMAIN="$1"
MAIL_HOST="${2:-mail.${DOMAIN}}"

echo -e "${BOLD}Verifying DNS records for: ${DOMAIN}${RESET}"
echo -e "${BOLD}Mail hostname: ${MAIL_HOST}${RESET}"
echo ""

# Added: Check that dig is available
if ! command -v dig &>/dev/null; then
    echo -e "${RED}Error: 'dig' command not found. Install dnsutils (Debian/Ubuntu) or bind-utils (RHEL/Fedora).${RESET}"
    exit 1
fi

# --------------------------------------------------------------------------
# MX Record
# --------------------------------------------------------------------------
echo -e "${BOLD}1. MX Record${RESET}"
MX_RESULT=$(dig +short MX "${DOMAIN}" 2>/dev/null)
if [[ -n "$MX_RESULT" ]]; then
    pass "MX record found: ${MX_RESULT}"
else
    fail "No MX record found for ${DOMAIN}"
fi

# --------------------------------------------------------------------------
# A Record for mail hostname
# --------------------------------------------------------------------------
echo -e "${BOLD}2. A Record (${MAIL_HOST})${RESET}"
A_RESULT=$(dig +short A "${MAIL_HOST}" 2>/dev/null)
if [[ -n "$A_RESULT" ]]; then
    pass "A record found: ${A_RESULT}"
    MAIL_IP="$A_RESULT"
else
    fail "No A record found for ${MAIL_HOST}"
    MAIL_IP=""
fi

# --------------------------------------------------------------------------
# SPF Record
# --------------------------------------------------------------------------
echo -e "${BOLD}3. SPF Record${RESET}"
SPF_RESULT=$(dig +short TXT "${DOMAIN}" 2>/dev/null | grep -i "v=spf1" || true)
if [[ -n "$SPF_RESULT" ]]; then
    pass "SPF record found: ${SPF_RESULT}"
    # Added: Warn if using ~all (softfail) instead of -all (hardfail)
    if echo "$SPF_RESULT" | grep -q '~all'; then
        warn "SPF uses ~all (softfail). Consider -all (hardfail) for stricter enforcement"
    fi
else
    fail "No SPF record found for ${DOMAIN}"
fi

# --------------------------------------------------------------------------
# DKIM Record
# --------------------------------------------------------------------------
echo -e "${BOLD}4. DKIM Record (default._domainkey)${RESET}"
DKIM_RESULT=$(dig +short TXT "default._domainkey.${DOMAIN}" 2>/dev/null)
if [[ -n "$DKIM_RESULT" ]]; then
    pass "DKIM record found"
    # Added: Verify it contains required DKIM fields
    if echo "$DKIM_RESULT" | grep -qi "v=DKIM1"; then
        pass "DKIM version tag present"
    else
        warn "DKIM record missing v=DKIM1 version tag"
    fi
else
    fail "No DKIM record found at default._domainkey.${DOMAIN}"
fi

# --------------------------------------------------------------------------
# DMARC Record
# --------------------------------------------------------------------------
echo -e "${BOLD}5. DMARC Record${RESET}"
DMARC_RESULT=$(dig +short TXT "_dmarc.${DOMAIN}" 2>/dev/null)
if [[ -n "$DMARC_RESULT" ]]; then
    pass "DMARC record found: ${DMARC_RESULT}"
    # Added: Check DMARC policy strictness
    if echo "$DMARC_RESULT" | grep -q 'p=none'; then
        warn "DMARC policy is 'none' (monitoring only). Consider p=quarantine or p=reject"
    elif echo "$DMARC_RESULT" | grep -q 'p=reject'; then
        pass "DMARC policy is 'reject' (strictest)"
    fi
else
    fail "No DMARC record found at _dmarc.${DOMAIN}"
fi

# --------------------------------------------------------------------------
# Autoconfig CNAME (Thunderbird)
# --------------------------------------------------------------------------
echo -e "${BOLD}6. Autoconfig CNAME (Thunderbird)${RESET}"
AUTOCONFIG_RESULT=$(dig +short CNAME "autoconfig.${DOMAIN}" 2>/dev/null)
if [[ -n "$AUTOCONFIG_RESULT" ]]; then
    pass "Autoconfig CNAME found: ${AUTOCONFIG_RESULT}"
else
    # Added: Also check for A record as alternative
    AUTOCONFIG_A=$(dig +short A "autoconfig.${DOMAIN}" 2>/dev/null)
    if [[ -n "$AUTOCONFIG_A" ]]; then
        pass "Autoconfig A record found: ${AUTOCONFIG_A} (CNAME preferred but A works)"
    else
        warn "No autoconfig record found (optional, for Thunderbird auto-setup)"
    fi
fi

# --------------------------------------------------------------------------
# Autodiscover SRV (Outlook)
# --------------------------------------------------------------------------
echo -e "${BOLD}7. Autodiscover SRV (Outlook)${RESET}"
AUTODISCOVER_RESULT=$(dig +short SRV "_autodiscover._tcp.${DOMAIN}" 2>/dev/null)
if [[ -n "$AUTODISCOVER_RESULT" ]]; then
    pass "Autodiscover SRV found: ${AUTODISCOVER_RESULT}"
else
    warn "No autodiscover SRV record found (optional, for Outlook auto-setup)"
fi

# --------------------------------------------------------------------------
# DANE TLSA Record
# --------------------------------------------------------------------------
echo -e "${BOLD}8. DANE TLSA Record${RESET}"
TLSA_RESULT=$(dig +short TLSA "_25._tcp.${MAIL_HOST}" 2>/dev/null)
if [[ -n "$TLSA_RESULT" ]]; then
    pass "DANE TLSA record found: ${TLSA_RESULT}"
else
    warn "No DANE TLSA record found (recommended for SMTP security)"
fi

# --------------------------------------------------------------------------
# MTA-STS Record
# --------------------------------------------------------------------------
echo -e "${BOLD}9. MTA-STS Record${RESET}"
MTASTS_RESULT=$(dig +short TXT "_mta-sts.${DOMAIN}" 2>/dev/null)
if [[ -n "$MTASTS_RESULT" ]]; then
    pass "MTA-STS TXT record found: ${MTASTS_RESULT}"
else
    warn "No MTA-STS record found (recommended for enforcing TLS on inbound mail)"
fi

# --------------------------------------------------------------------------
# SMTP TLS Reporting
# --------------------------------------------------------------------------
echo -e "${BOLD}10. SMTP TLS Reporting${RESET}"
TLSRPT_RESULT=$(dig +short TXT "_smtp._tls.${DOMAIN}" 2>/dev/null)
if [[ -n "$TLSRPT_RESULT" ]]; then
    pass "TLSRPT record found: ${TLSRPT_RESULT}"
else
    warn "No TLSRPT record found (recommended for TLS failure reporting)"
fi

# --------------------------------------------------------------------------
# Reverse PTR Record
# --------------------------------------------------------------------------
echo -e "${BOLD}11. Reverse PTR Record${RESET}"
if [[ -n "${MAIL_IP:-}" ]]; then
    PTR_RESULT=$(dig +short -x "${MAIL_IP}" 2>/dev/null)
    if [[ -n "$PTR_RESULT" ]]; then
        pass "PTR record found: ${PTR_RESULT}"
        # Added: Check PTR matches forward DNS
        if echo "$PTR_RESULT" | grep -qi "${MAIL_HOST}"; then
            pass "PTR matches forward DNS (${MAIL_HOST})"
        else
            warn "PTR (${PTR_RESULT}) does not match mail hostname (${MAIL_HOST})"
        fi
    else
        fail "No PTR record found for ${MAIL_IP} (configure at your VPS provider)"
    fi
else
    warn "Skipped PTR check — no A record IP available"
fi

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
echo -e "${BOLD}Summary${RESET}"
echo -e "  ${GREEN}Passed: ${PASS}${RESET}"
echo -e "  ${RED}Failed: ${FAIL}${RESET}"
echo -e "  ${YELLOW}Warnings: ${WARN}${RESET}"

if [[ $FAIL -gt 0 ]]; then
    echo ""
    echo -e "${RED}Some checks failed. Review the output above and update your DNS records.${RESET}"
    exit 1
else
    echo ""
    echo -e "${GREEN}All critical checks passed.${RESET}"
    exit 0
fi
