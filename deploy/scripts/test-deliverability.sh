#!/usr/bin/env bash
# Added: Email deliverability testing script for TMAIL-39
# PURPOSE: Comprehensive deliverability check for a mail server domain
# USAGE: ./test-deliverability.sh mail.example.com
# OUTPUT: Scored report (0-100) with pass/fail per check

set -euo pipefail

# Added: Color codes for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Added: Score tracking
TOTAL_CHECKS=0
PASSED_CHECKS=0
WARNED_CHECKS=0

# Added: Validate arguments
if [ $# -lt 1 ]; then
    echo "Usage: $0 <mail-server-domain>"
    echo "Example: $0 mail.example.com"
    exit 1
fi

DOMAIN="$1"

# Added: Helper functions for result formatting
pass() {
    local check_name="$1"
    local details="$2"
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    PASSED_CHECKS=$((PASSED_CHECKS + 1))
    echo -e "  ${GREEN}[PASS]${NC} ${BOLD}${check_name}${NC}: ${details}"
}

fail() {
    local check_name="$1"
    local details="$2"
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    echo -e "  ${RED}[FAIL]${NC} ${BOLD}${check_name}${NC}: ${details}"
}

warn() {
    local check_name="$1"
    local details="$2"
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    WARNED_CHECKS=$((WARNED_CHECKS + 1))
    echo -e "  ${YELLOW}[WARN]${NC} ${BOLD}${check_name}${NC}: ${details}"
}

error() {
    local check_name="$1"
    local details="$2"
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    echo -e "  ${RED}[ERROR]${NC} ${BOLD}${check_name}${NC}: ${details}"
}

section() {
    echo ""
    echo -e "${BLUE}${BOLD}=== $1 ===${NC}"
}

echo -e "${BOLD}Email Deliverability Check for: ${DOMAIN}${NC}"
echo "Date: $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
echo "============================================="

# --- Section 1: Service Connectivity ---
section "Service Connectivity"

# Added: Check Postfix on port 25
if timeout 5 bash -c "echo QUIT | nc -w5 ${DOMAIN} 25" 2>/dev/null | grep -qi "220"; then
    pass "SMTP (Port 25)" "Postfix accepting connections"
else
    fail "SMTP (Port 25)" "Cannot connect or no SMTP banner received"
fi

# Added: Check SMTP submission on port 587
if timeout 5 bash -c "echo QUIT | nc -w5 ${DOMAIN} 587" 2>/dev/null | grep -qi "220"; then
    pass "SMTP Submission (Port 587)" "Accepting connections on submission port"
else
    fail "SMTP Submission (Port 587)" "Cannot connect or no banner on port 587"
fi

# Added: Check Dovecot IMAP on port 993
if timeout 5 bash -c "echo | openssl s_client -connect ${DOMAIN}:993 -servername ${DOMAIN} 2>/dev/null" | grep -qi "OK\|IMAP\|Dovecot"; then
    pass "IMAPS (Port 993)" "Dovecot accepting IMAP connections"
else
    fail "IMAPS (Port 993)" "Cannot connect to IMAP on port 993"
fi

# --- Section 2: DNS Records ---
section "DNS Records"

# Added: Check MX records
MX_RECORDS=$(dig +short MX "${DOMAIN}" 2>/dev/null || true)
if [ -n "${MX_RECORDS}" ]; then
    MX_COUNT=$(echo "${MX_RECORDS}" | wc -l)
    pass "MX Records" "${MX_COUNT} record(s) found: $(echo ${MX_RECORDS} | tr '\n' ', ')"
else
    fail "MX Records" "No MX records found for ${DOMAIN}"
fi

# Added: Check SPF record
SPF_RECORD=$(dig +short TXT "${DOMAIN}" 2>/dev/null | grep "v=spf1" || true)
if [ -n "${SPF_RECORD}" ]; then
    SPF_COUNT=$(echo "${SPF_RECORD}" | wc -l)
    if [ "${SPF_COUNT}" -gt 1 ]; then
        warn "SPF Record" "Multiple SPF records found — should have exactly one"
    else
        pass "SPF Record" "Found: ${SPF_RECORD}"
    fi
else
    fail "SPF Record" "No SPF record found (missing v=spf1 TXT record)"
fi

# Added: Check DKIM record (default and mail selectors)
DKIM_DEFAULT=$(dig +short TXT "default._domainkey.${DOMAIN}" 2>/dev/null || true)
DKIM_MAIL=$(dig +short TXT "mail._domainkey.${DOMAIN}" 2>/dev/null || true)
if echo "${DKIM_DEFAULT}" | grep -qi "v=DKIM1\|p="; then
    pass "DKIM Record" "Found at default._domainkey.${DOMAIN}"
elif echo "${DKIM_MAIL}" | grep -qi "v=DKIM1\|p="; then
    pass "DKIM Record" "Found at mail._domainkey.${DOMAIN}"
else
    warn "DKIM Record" "No DKIM record found at default or mail selectors"
fi

# Added: Check DMARC record
DMARC_RECORD=$(dig +short TXT "_dmarc.${DOMAIN}" 2>/dev/null || true)
if echo "${DMARC_RECORD}" | grep -qi "v=DMARC1"; then
    if echo "${DMARC_RECORD}" | grep -qi "p=reject"; then
        pass "DMARC Record" "Found with p=reject (strongest policy)"
    elif echo "${DMARC_RECORD}" | grep -qi "p=quarantine"; then
        pass "DMARC Record" "Found with p=quarantine"
    else
        warn "DMARC Record" "Found but policy is p=none (monitoring only)"
    fi
else
    fail "DMARC Record" "No DMARC record found"
fi

# --- Section 3: Reverse DNS ---
section "Reverse DNS"

# Added: Resolve domain to IP and check PTR
SERVER_IP=$(dig +short A "${DOMAIN}" 2>/dev/null | head -1 || true)
if [ -n "${SERVER_IP}" ]; then
    PTR_RECORD=$(dig +short -x "${SERVER_IP}" 2>/dev/null | sed 's/\.$//' || true)
    if [ -n "${PTR_RECORD}" ]; then
        if [ "${PTR_RECORD,,}" = "${DOMAIN,,}" ]; then
            pass "Reverse DNS (PTR)" "PTR ${PTR_RECORD} matches ${DOMAIN} (IP: ${SERVER_IP})"
        else
            warn "Reverse DNS (PTR)" "PTR ${PTR_RECORD} does not match ${DOMAIN} (IP: ${SERVER_IP})"
        fi
    else
        fail "Reverse DNS (PTR)" "No PTR record for IP ${SERVER_IP}"
    fi
else
    error "Reverse DNS (PTR)" "Could not resolve ${DOMAIN} to an IP address"
fi

# --- Section 4: TLS Certificate ---
section "TLS Certificate"

# Added: Check TLS certificate validity on port 993
TLS_OUTPUT=$(echo | timeout 10 openssl s_client -connect "${DOMAIN}:993" -servername "${DOMAIN}" -verify_return_error 2>&1 || true)
if echo "${TLS_OUTPUT}" | grep -qi "Verify return code: 0"; then
    # Added: Extract certificate expiry
    CERT_EXPIRY=$(echo "${TLS_OUTPUT}" | openssl x509 -noout -enddate 2>/dev/null | cut -d= -f2 || true)
    if [ -n "${CERT_EXPIRY}" ]; then
        pass "TLS Certificate (993)" "Valid certificate, expires: ${CERT_EXPIRY}"
    else
        pass "TLS Certificate (993)" "Valid certificate on port 993"
    fi
elif echo "${TLS_OUTPUT}" | grep -qi "Verify return code:"; then
    VERIFY_ERROR=$(echo "${TLS_OUTPUT}" | grep "Verify return code:" || true)
    warn "TLS Certificate (993)" "Certificate issue: ${VERIFY_ERROR}"
else
    fail "TLS Certificate (993)" "Could not verify TLS certificate on port 993"
fi

# Added: Check STARTTLS on port 587
TLS_587=$(echo | timeout 10 openssl s_client -connect "${DOMAIN}:587" -servername "${DOMAIN}" -starttls smtp 2>&1 || true)
if echo "${TLS_587}" | grep -qi "Verify return code: 0"; then
    pass "TLS Certificate (587 STARTTLS)" "Valid STARTTLS certificate on port 587"
elif echo "${TLS_587}" | grep -qi "Verify return code:"; then
    warn "TLS Certificate (587 STARTTLS)" "STARTTLS available but certificate has issues"
else
    fail "TLS Certificate (587 STARTTLS)" "STARTTLS not available or certificate invalid on port 587"
fi

# --- Section 5: Blacklist Check ---
section "Blacklist Check"

if [ -n "${SERVER_IP}" ]; then
    # Added: Reverse the IP octets for DNSBL queries
    IFS='.' read -ra OCTETS <<< "${SERVER_IP}"
    REVERSED_IP="${OCTETS[3]}.${OCTETS[2]}.${OCTETS[1]}.${OCTETS[0]}"

    BLACKLISTED=0
    BLACKLISTS_CHECKED=0

    # Added: Check Spamhaus ZEN
    BLACKLISTS_CHECKED=$((BLACKLISTS_CHECKED + 1))
    SH_RESULT=$(dig +short "${REVERSED_IP}.zen.spamhaus.org" A 2>/dev/null || true)
    if echo "${SH_RESULT}" | grep -q "127\."; then
        fail "Spamhaus ZEN" "IP ${SERVER_IP} is LISTED (${SH_RESULT})"
        BLACKLISTED=$((BLACKLISTED + 1))
    else
        pass "Spamhaus ZEN" "IP ${SERVER_IP} is clean"
    fi

    # Added: Check Barracuda
    BLACKLISTS_CHECKED=$((BLACKLISTS_CHECKED + 1))
    BC_RESULT=$(dig +short "${REVERSED_IP}.b.barracudacentral.org" A 2>/dev/null || true)
    if echo "${BC_RESULT}" | grep -q "127\."; then
        fail "Barracuda" "IP ${SERVER_IP} is LISTED (${BC_RESULT})"
        BLACKLISTED=$((BLACKLISTED + 1))
    else
        pass "Barracuda" "IP ${SERVER_IP} is clean"
    fi

    # Added: Check SORBS
    BLACKLISTS_CHECKED=$((BLACKLISTS_CHECKED + 1))
    SORBS_RESULT=$(dig +short "${REVERSED_IP}.dnsbl.sorbs.net" A 2>/dev/null || true)
    if echo "${SORBS_RESULT}" | grep -q "127\."; then
        fail "SORBS" "IP ${SERVER_IP} is LISTED (${SORBS_RESULT})"
        BLACKLISTED=$((BLACKLISTED + 1))
    else
        pass "SORBS" "IP ${SERVER_IP} is clean"
    fi

    if [ "${BLACKLISTED}" -eq 0 ]; then
        echo -e "  ${GREEN}Not listed on any of ${BLACKLISTS_CHECKED} checked blacklists${NC}"
    fi
else
    error "Blacklist Check" "Skipped — could not resolve domain IP"
fi

# --- Section 6: SMTP Test (send + receive) ---
section "SMTP Send/Receive Test"

# Added: Attempt to send a test email via SMTP if swaks is available
if command -v swaks >/dev/null 2>&1; then
    TEST_ID="deliverability-test-$(date +%s)"
    SWAKS_RESULT=$(swaks --to "postmaster@${DOMAIN}" --from "test@${DOMAIN}" \
        --server "${DOMAIN}" --port 25 \
        --header "Subject: Deliverability Test ${TEST_ID}" \
        --body "This is an automated deliverability test." \
        --timeout 15 2>&1 || true)
    if echo "${SWAKS_RESULT}" | grep -qi "250.*OK\|250.*Accepted"; then
        pass "SMTP Send Test" "Test email accepted by server"
    else
        warn "SMTP Send Test" "Email may not have been accepted (swaks returned unexpected response)"
    fi
else
    warn "SMTP Send Test" "Skipped — swaks not installed (apt install swaks)"
fi

# --- Calculate Score ---
echo ""
echo "============================================="

# Added: Calculate score (pass=full points, warn=half, fail/error=0)
if [ "${TOTAL_CHECKS}" -gt 0 ]; then
    # NOTE: Using awk for floating point arithmetic
    SCORE=$(awk "BEGIN {
        pass=${PASSED_CHECKS};
        warn=${WARNED_CHECKS};
        total=${TOTAL_CHECKS};
        points_per = 100.0 / total;
        score = (pass * points_per) + (warn * points_per * 0.5);
        printf \"%.0f\", score
    }")
else
    SCORE=0
fi

# Added: Score color
if [ "${SCORE}" -ge 80 ]; then
    SCORE_COLOR="${GREEN}"
elif [ "${SCORE}" -ge 60 ]; then
    SCORE_COLOR="${YELLOW}"
else
    SCORE_COLOR="${RED}"
fi

FAILED_CHECKS=$((TOTAL_CHECKS - PASSED_CHECKS - WARNED_CHECKS))

echo -e "${BOLD}Deliverability Score: ${SCORE_COLOR}${SCORE}/100${NC}"
echo -e "  Passed: ${GREEN}${PASSED_CHECKS}${NC} | Warnings: ${YELLOW}${WARNED_CHECKS}${NC} | Failed: ${RED}${FAILED_CHECKS}${NC} | Total: ${TOTAL_CHECKS}"
echo ""
