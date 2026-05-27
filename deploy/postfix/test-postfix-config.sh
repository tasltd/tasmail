#!/usr/bin/env bash
# Added: TMAIL-11 acceptance test for Postfix configuration templates.
# Runs entirely on the templates + setup script in this directory — no live
# Postfix needed, so the suite is safe in CI and on machines without postfix
# installed. Mirrors deploy/dovecot/test-dovecot-config.sh.
#
# Verifies every requirement enumerated in TMAIL-11:
#   * Postfix + postfix-pgsql installed by setup script
#   * virtual_transport delivers to Dovecot LMTP socket
#   * PostgreSQL-backed virtual_mailbox_domains / _maps / virtual_alias_maps
#   * smtpd_sasl_type = dovecot, smtpd_sasl_path = private/auth
#   * TLS keyed off Let's Encrypt fullchain.pem / privkey.pem
#   * Submission service on port 587 with STARTTLS + SASL
#   * vmail uid/gid 5000 created (matches Dovecot)
#   * Rspamd milter wiring for DKIM/spam
#   * Open-relay guard (reject_unauth_destination)
#
# Usage: ./test-postfix-config.sh
# Exit:  0 = all checks pass, 1 = at least one check failed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MAIN="${SCRIPT_DIR}/main.cf.template"
DOMAINS="${SCRIPT_DIR}/virtual_mailbox_domains.cf.template"
MAILBOXES="${SCRIPT_DIR}/virtual_mailbox_maps.cf.template"
ALIASES="${SCRIPT_DIR}/virtual_alias_maps.cf.template"
SETUP="${SCRIPT_DIR}/setup-postfix.sh"

GREEN='\033[0;32m'
RED='\033[0;31m'
BOLD='\033[1m'
RESET='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0

pass() {
    echo -e "  ${GREEN}[PASS]${RESET} $1"
    PASS_COUNT=$((PASS_COUNT + 1))
}

fail() {
    echo -e "  ${RED}[FAIL]${RESET} $1"
    FAIL_COUNT=$((FAIL_COUNT + 1))
}

# Added: assert_match — assert a regex matches in a file. Args: <file> <regex> <description>
assert_match() {
    local file="$1" regex="$2" desc="$3"
    if grep -Eq "$regex" "$file"; then
        pass "$desc"
    else
        fail "$desc  (missing in $(basename "$file"): /$regex/)"
    fi
}

# Added: assert_match_multiline — same, but treats the file as a single string
# so the regex can span newlines. Needed for the heredoc-emitted submission
# block in setup-postfix.sh where service header + -o options span lines.
assert_match_multiline() {
    local file="$1" regex="$2" desc="$3"
    if grep -Pzoq "$regex" "$file" 2>/dev/null; then
        pass "$desc"
    else
        fail "$desc  (missing in $(basename "$file"))"
    fi
}

echo -e "${BOLD}TMAIL-11 — Postfix configuration acceptance tests${RESET}"
echo "========================================="

# --- File existence ------------------------------------------------------
for f in "$MAIN" "$DOMAINS" "$MAILBOXES" "$ALIASES" "$SETUP"; do
    if [[ -f "$f" ]]; then
        pass "File exists: $(basename "$f")"
    else
        fail "File missing: $f"
    fi
done

# Abort early if templates are missing — every later assertion would cascade.
if [[ $FAIL_COUNT -gt 0 ]]; then
    echo ""
    echo -e "${RED}Cannot continue: required files missing.${RESET}"
    exit 1
fi

echo ""
echo "Package installation (Postfix + PostgreSQL adapter):"
# Added: postfix-pgsql is required for the pgsql: lookup tables below.
# A Postfix install without it silently falls back to refusing every recipient.
assert_match "$SETUP" 'apt-get install.*postfix-pgsql' \
    "Debian/Ubuntu install includes postfix-pgsql"
assert_match "$SETUP" '(dnf|yum) install.*postfix-pgsql' \
    "RHEL family install includes postfix-pgsql"

echo ""
echo "Virtual transport (TMAIL-11 requires Dovecot LMTP):"
# Added: Dovecot listens on /var/spool/postfix/private/dovecot-lmtp via the
# chrooted master.cf path private/dovecot-lmtp. Anything else means mail
# never reaches the user's Maildir.
assert_match "$MAIN" '^virtual_transport\s*=\s*lmtp:unix:private/dovecot-lmtp' \
    "virtual_transport routes to Dovecot LMTP socket"

echo ""
echo "PostgreSQL-backed mailbox lookups:"
# Added: All three lookup tables must point at PostgreSQL .cf files so the
# Rust backend's domain/mailbox/alias rows are the single source of truth.
assert_match "$MAIN" '^virtual_mailbox_maps\s*=\s*pgsql:/etc/postfix/pgsql-virtual-mailbox-maps\.cf' \
    "virtual_mailbox_maps uses pgsql lookup"
assert_match "$MAIN" '^virtual_alias_maps\s*=\s*pgsql:/etc/postfix/pgsql-virtual-alias-maps\.cf' \
    "virtual_alias_maps uses pgsql lookup"
# Added: setup-postfix.sh rewrites virtual_mailbox_domains to a pgsql lookup
# (the template ships a static line for documentation only).
assert_match "$SETUP" 'virtual_mailbox_domains = pgsql:/etc/postfix/pgsql-virtual-mailbox-domains\.cf' \
    "setup script swaps virtual_mailbox_domains to pgsql lookup"

echo ""
echo "PostgreSQL connection templates:"
# Added: Each .cf needs hosts/dbname/user/password placeholders so the
# deploy_pgsql_map helper in setup-postfix.sh has something to substitute.
for tpl in "$DOMAINS" "$MAILBOXES" "$ALIASES"; do
    name="$(basename "$tpl")"
    assert_match "$tpl" '^hosts\s*=\s*__DB_HOST__'   "$name has __DB_HOST__ placeholder"
    assert_match "$tpl" '^dbname\s*=\s*__DB_NAME__'  "$name has __DB_NAME__ placeholder"
    assert_match "$tpl" '^user\s*=\s*__DB_USER__'    "$name has __DB_USER__ placeholder"
    assert_match "$tpl" '^password\s*=\s*__DB_PASS__' "$name has __DB_PASS__ placeholder"
    assert_match "$tpl" '^query\s*=' "$name defines a SQL query"
done
# Added: Queries must filter inactive rows so disabled accounts can't receive
# mail without dropping the entire row.
assert_match "$DOMAINS"   "active = true" "domains query filters active=true"
assert_match "$MAILBOXES" "active = true" "mailboxes query filters active=true"
assert_match "$ALIASES"   "active = true" "aliases query filters active=true"

echo ""
echo "SASL authentication via Dovecot (TMAIL-11 requires Dovecot auth socket):"
# Added: smtpd_sasl_type=dovecot tells Postfix to delegate AUTH to Dovecot
# over the unix socket created by Dovecot's service auth { } block.
assert_match "$MAIN" '^smtpd_sasl_type\s*=\s*dovecot'      "smtpd_sasl_type = dovecot"
assert_match "$MAIN" '^smtpd_sasl_path\s*=\s*private/auth' "smtpd_sasl_path = private/auth (Dovecot socket)"
assert_match "$MAIN" '^smtpd_sasl_auth_enable\s*=\s*yes'   "SASL auth enabled"
# Added: noanonymous + noplaintext on the unencrypted path; the looser
# noanonymous-only set applies after STARTTLS upgrades the connection.
assert_match "$MAIN" '^smtpd_sasl_security_options\s*=.*noanonymous'  "SASL rejects anonymous"
assert_match "$MAIN" '^smtpd_sasl_security_options\s*=.*noplaintext'  "SASL rejects plaintext pre-TLS"

echo ""
echo "TLS / Let's Encrypt:"
# Added: Paths match certbot's standard /etc/letsencrypt/live/<host>/ layout;
# setup-tls.sh provisions and renews the certs that land here.
assert_match "$MAIN" 'smtpd_tls_cert_file\s*=\s*/etc/letsencrypt/live/' \
    "TLS cert pinned to Let's Encrypt path"
assert_match "$MAIN" 'smtpd_tls_key_file\s*=\s*/etc/letsencrypt/live/' \
    "TLS key pinned to Let's Encrypt path"
assert_match "$MAIN" '^smtpd_tls_auth_only\s*=\s*yes' \
    "AUTH only after STARTTLS (no plaintext credentials)"
# Added: TMAIL-16 raised the bar to TLS 1.2 minimum; assert the legacy
# protocols stay disabled so the harness catches regressions from either side.
assert_match "$MAIN" '^smtpd_tls_protocols\s*=.*!TLSv1\b.*!TLSv1\.1' \
    "Inbound TLS rejects TLSv1.0 and TLSv1.1"
assert_match "$MAIN" '^smtp_tls_protocols\s*=.*!TLSv1\b.*!TLSv1\.1' \
    "Outbound TLS rejects TLSv1.0 and TLSv1.1"

echo ""
echo "Submission service (port 587 with STARTTLS + SASL):"
# Added: setup-postfix.sh appends the submission block to master.cf via
# heredoc; the lines below are emitted into master.cf at install time.
assert_match "$SETUP" '^submission inet n' \
    "master.cf appends 'submission inet' service line"
# Added: smtpd_tls_security_level=encrypt on the submission service forces
# STARTTLS before any AUTH command — clients that don't upgrade get rejected.
assert_match_multiline "$SETUP" 'submission inet[\s\S]*?smtpd_tls_security_level=encrypt' \
    "Submission service forces STARTTLS (tls_security_level=encrypt)"
assert_match_multiline "$SETUP" 'submission inet[\s\S]*?smtpd_sasl_auth_enable=yes' \
    "Submission service enables SASL auth"
assert_match_multiline "$SETUP" 'submission inet[\s\S]*?smtpd_tls_auth_only=yes' \
    "Submission service requires AUTH over TLS only"
assert_match_multiline "$SETUP" 'submission inet[\s\S]*?smtpd_recipient_restrictions=permit_sasl_authenticated,reject' \
    "Submission service rejects unauthenticated senders"

echo ""
echo "vmail system user (must match Dovecot uid/gid 5000):"
# Added: Dovecot's user_query pins uid=5000/gid=5000. If Postfix creates a
# different vmail uid, Dovecot can't write the Maildir after LMTP delivery.
assert_match "$SETUP" 'groupadd -g 5000 vmail'  "setup creates vmail group gid=5000"
assert_match "$SETUP" 'useradd -u 5000 -g vmail' "setup creates vmail user uid=5000"
assert_match "$SETUP" 'mkdir -p "/var/mail/vhosts/' "setup creates per-domain Maildir root"

echo ""
echo "Open-relay guard + security restrictions:"
# Added: reject_unauth_destination on smtpd_relay_restrictions is the
# anti-open-relay line — without it any client could relay through us.
assert_match_multiline "$MAIN" 'smtpd_relay_restrictions\s*=[\s\S]*?reject_unauth_destination' \
    "Relay restrictions reject unauthenticated relaying"
assert_match "$MAIN" '^smtpd_helo_required\s*=\s*yes'        "HELO/EHLO required"
assert_match "$MAIN" '^disable_vrfy_command\s*=\s*yes'       "VRFY disabled (no address harvesting)"
assert_match_multiline "$MAIN" 'smtpd_sender_restrictions\s*=[\s\S]*?reject_unknown_sender_domain' \
    "Sender restrictions reject unknown sender domains"

echo ""
echo "Rspamd milter wiring (DKIM signing + spam filtering):"
# Added: Rspamd listens on 127.0.0.1:11332 as the milter endpoint that signs
# outbound mail with DKIM and rejects/marks inbound spam. Without these the
# mail server passes nothing through the spam filter.
assert_match "$MAIN" '^smtpd_milters\s*=\s*inet:127\.0\.0\.1:11332' \
    "smtpd_milters wired to Rspamd on 127.0.0.1:11332"
assert_match "$MAIN" '^milter_default_action\s*=\s*accept' \
    "milter_default_action = accept (fail-open on Rspamd outage)"

echo ""
echo "Postgres credential file permissions:"
# Added: pgsql .cf files store the DB password in plaintext, so they must
# be 640 root:postfix — readable by Postfix at start-up but not world.
assert_match "$SETUP" 'chmod 640 "\$\{dest_path\}"'    "pgsql .cf files chmod 640"
assert_match "$SETUP" 'chown root:postfix "\$\{dest_path\}"' "pgsql .cf files chown root:postfix"

echo ""
echo "postfix check validation step:"
# Added: setup-postfix.sh runs `postfix check` after rendering main.cf so
# typos abort install instead of leaving a broken queue.
assert_match "$SETUP" 'postfix check' "setup script runs 'postfix check' before enabling"

# --- Summary -------------------------------------------------------------
echo ""
echo "========================================="
TOTAL=$((PASS_COUNT + FAIL_COUNT))
if [[ $FAIL_COUNT -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All ${TOTAL} checks passed.${RESET}"
    exit 0
else
    echo -e "${RED}${BOLD}${FAIL_COUNT} of ${TOTAL} checks failed.${RESET}"
    exit 1
fi
