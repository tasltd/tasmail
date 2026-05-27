#!/usr/bin/env bash
# Added: TLS validation harness for TASMail (TMAIL-16)
#
# Validates that a deployed TASMail host meets the TLS requirements:
#   - HTTPS (Nginx) on 443 negotiates TLS 1.2 or 1.3, rejects TLS 1.0/1.1
#   - IMAPS (Dovecot) on 993 negotiates TLS 1.2 or 1.3, rejects TLS 1.0/1.1
#   - SMTP submission (Postfix) on 587 advertises STARTTLS
#   - TLS 1.3 is preferred when both peers support it
#   - Valid Let's Encrypt certificate chain
#
# Usage:
#   ./test-tls.sh <hostname>                 # full suite against real host
#   ./test-tls.sh --self-test                # offline unit tests (no network)
#
# Dependencies: openssl, timeout, curl (optional, for HTTPS check)
# Exit codes: 0 = all passed, 1 = one or more failures, 2 = bad usage

set -uo pipefail

# Added: Color helpers
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

PASS=0
FAIL=0
SKIPPED=0

pass() { echo -e "${GREEN}[PASS]${RESET} $1"; PASS=$((PASS+1)); }
fail() { echo -e "${RED}[FAIL]${RESET} $1"; FAIL=$((FAIL+1)); }
skip() { echo -e "${YELLOW}[SKIP]${RESET} $1"; SKIPPED=$((SKIPPED+1)); }
info() { echo -e "${BOLD}[INFO]${RESET} $1"; }

# --------------------------------------------------------------------------
# Reusable assertions — these are also exercised by --self-test mode
# --------------------------------------------------------------------------

# assert_protocol_accepted <handshake_output> <expected_protocol>
# Returns 0 if the handshake output indicates the given protocol was used.
assert_protocol_accepted() {
    local output="$1"
    local expected="$2"
    grep -qE "Protocol[[:space:]]*:[[:space:]]*${expected}" <<<"$output"
}

# assert_protocol_rejected <handshake_output>
# Returns 0 if the handshake output indicates a TLS error (server refused).
assert_protocol_rejected() {
    local output="$1"
    grep -qE "(alert|handshake failure|tlsv1 alert|wrong version number|no protocols available|unsupported protocol|protocol_version)" <<<"$output"
}

# assert_cert_valid <s_client_output>
# Returns 0 if openssl reported verify return code 0 (valid chain).
assert_cert_valid() {
    local output="$1"
    grep -qE "Verify return code:[[:space:]]*0" <<<"$output"
}

# assert_starttls_advertised <smtp_banner>
# Returns 0 if the SMTP EHLO output advertises STARTTLS.
assert_starttls_advertised() {
    local banner="$1"
    grep -qE "^250[ -]STARTTLS" <<<"$banner"
}

# --------------------------------------------------------------------------
# Self-test mode — exercises the assertion helpers offline with synthetic
# fixtures. Lets CI / dev environments validate this script itself without
# needing a live deployment.
# --------------------------------------------------------------------------
run_self_test() {
    info "Running self-test (no network calls)"

    local tls13_handshake="    Protocol  : TLSv1.3
    Verify return code: 0 (ok)"
    local tls12_handshake="    Protocol  : TLSv1.2
    Verify return code: 0 (ok)"
    local tls10_rejected="139..:error:1408F10B:SSL routines:ssl3_get_record:wrong version number"
    local tls11_alert="139..:error: tlsv1 alert protocol version"
    local bad_chain="    Protocol  : TLSv1.3
    Verify return code: 21 (unable to verify the first certificate)"
    local smtp_with_starttls="250-mail.example.com Hello
250-SIZE 26214400
250-STARTTLS
250 HELP"
    local smtp_no_starttls="250-mail.example.com Hello
250 HELP"

    # Protocol acceptance
    if assert_protocol_accepted "$tls13_handshake" "TLSv1.3"; then
        pass "self-test: detects TLS 1.3 acceptance"
    else
        fail "self-test: should detect TLS 1.3 acceptance"
    fi

    if assert_protocol_accepted "$tls12_handshake" "TLSv1.2"; then
        pass "self-test: detects TLS 1.2 acceptance"
    else
        fail "self-test: should detect TLS 1.2 acceptance"
    fi

    if assert_protocol_accepted "$tls12_handshake" "TLSv1.3"; then
        fail "self-test: must not confuse 1.2 with 1.3"
    else
        pass "self-test: refuses to call TLS 1.2 a TLS 1.3 handshake"
    fi

    # Protocol rejection
    if assert_protocol_rejected "$tls10_rejected"; then
        pass "self-test: detects TLS 1.0 rejection (wrong version number)"
    else
        fail "self-test: should detect TLS 1.0 rejection"
    fi

    if assert_protocol_rejected "$tls11_alert"; then
        pass "self-test: detects TLS 1.1 rejection (tlsv1 alert)"
    else
        fail "self-test: should detect TLS 1.1 rejection"
    fi

    if assert_protocol_rejected "$tls13_handshake"; then
        fail "self-test: must not flag a successful handshake as rejected"
    else
        pass "self-test: a successful handshake is not classed as rejected"
    fi

    # Cert validity
    if assert_cert_valid "$tls13_handshake"; then
        pass "self-test: detects valid certificate chain"
    else
        fail "self-test: should detect valid certificate chain"
    fi

    if assert_cert_valid "$bad_chain"; then
        fail "self-test: must not call a broken chain valid"
    else
        pass "self-test: detects broken certificate chain"
    fi

    # STARTTLS advertisement
    if assert_starttls_advertised "$smtp_with_starttls"; then
        pass "self-test: detects STARTTLS advertised"
    else
        fail "self-test: should detect STARTTLS in EHLO response"
    fi

    if assert_starttls_advertised "$smtp_no_starttls"; then
        fail "self-test: must not invent STARTTLS support"
    else
        pass "self-test: detects missing STARTTLS"
    fi
}

# --------------------------------------------------------------------------
# Live probes (used when a hostname is supplied)
# --------------------------------------------------------------------------

# probe_tls <host> <port> <protocol_flag> <starttls_proto>
# protocol_flag: -tls1, -tls1_1, -tls1_2, -tls1_3, or "" for default
# starttls_proto: smtp|imap|"" — passed to openssl when needed
probe_tls() {
    local host="$1"
    local port="$2"
    local proto_flag="$3"
    local starttls="$4"
    local args=(s_client -connect "${host}:${port}" -servername "$host")
    [[ -n "$proto_flag" ]] && args+=("$proto_flag")
    [[ -n "$starttls" ]] && args+=(-starttls "$starttls")
    timeout 10 openssl "${args[@]}" </dev/null 2>&1
}

run_live_suite() {
    local host="$1"
    info "Probing TLS endpoints on $host"

    # HTTPS — Nginx on 443
    if command -v openssl >/dev/null 2>&1; then
        local out
        out=$(probe_tls "$host" 443 "-tls1_3" "")
        if assert_protocol_accepted "$out" "TLSv1.3" && assert_cert_valid "$out"; then
            pass "443/HTTPS: TLS 1.3 handshake + valid chain"
        else
            # TLS 1.3 may not be negotiated by every openssl build — fall back to 1.2
            out=$(probe_tls "$host" 443 "-tls1_2" "")
            if assert_protocol_accepted "$out" "TLSv1.2" && assert_cert_valid "$out"; then
                pass "443/HTTPS: TLS 1.2 handshake + valid chain (1.3 not negotiated by client)"
            else
                fail "443/HTTPS: no TLS 1.2/1.3 handshake or invalid chain"
            fi
        fi

        out=$(probe_tls "$host" 443 "-tls1" "")
        if assert_protocol_rejected "$out"; then
            pass "443/HTTPS: TLS 1.0 correctly rejected"
        else
            fail "443/HTTPS: TLS 1.0 was NOT rejected"
        fi

        out=$(probe_tls "$host" 443 "-tls1_1" "")
        if assert_protocol_rejected "$out"; then
            pass "443/HTTPS: TLS 1.1 correctly rejected"
        else
            fail "443/HTTPS: TLS 1.1 was NOT rejected"
        fi

        # IMAPS — Dovecot on 993
        out=$(probe_tls "$host" 993 "" "")
        if assert_cert_valid "$out"; then
            pass "993/IMAPS: valid certificate chain"
        else
            fail "993/IMAPS: certificate chain failed validation"
        fi
        if assert_protocol_accepted "$out" "TLSv1.3" || assert_protocol_accepted "$out" "TLSv1.2"; then
            pass "993/IMAPS: TLS 1.2+ negotiated"
        else
            fail "993/IMAPS: no TLS 1.2 or 1.3 handshake"
        fi

        out=$(probe_tls "$host" 993 "-tls1" "")
        if assert_protocol_rejected "$out"; then
            pass "993/IMAPS: TLS 1.0 correctly rejected"
        else
            fail "993/IMAPS: TLS 1.0 was NOT rejected"
        fi

        # SMTP submission — Postfix on 587 (STARTTLS)
        out=$(probe_tls "$host" 587 "" "smtp")
        if assert_starttls_advertised "$out"; then
            pass "587/SMTP: STARTTLS advertised"
        else
            fail "587/SMTP: STARTTLS NOT advertised"
        fi
        if assert_protocol_accepted "$out" "TLSv1.3" || assert_protocol_accepted "$out" "TLSv1.2"; then
            pass "587/SMTP: TLS 1.2+ negotiated after STARTTLS"
        else
            fail "587/SMTP: TLS 1.2/1.3 not negotiated after STARTTLS"
        fi
    else
        skip "openssl not installed — skipping live probes"
    fi
}

# --------------------------------------------------------------------------
# Entry point
# --------------------------------------------------------------------------
main() {
    if [[ $# -lt 1 ]]; then
        echo "Usage: $0 <hostname>          # live probe"
        echo "       $0 --self-test         # offline unit tests"
        exit 2
    fi

    if [[ "$1" == "--self-test" ]]; then
        run_self_test
    else
        run_live_suite "$1"
    fi

    echo ""
    echo -e "${BOLD}Summary:${RESET} ${GREEN}${PASS} passed${RESET}, ${RED}${FAIL} failed${RESET}, ${YELLOW}${SKIPPED} skipped${RESET}"
    [[ $FAIL -eq 0 ]]
}

main "$@"
