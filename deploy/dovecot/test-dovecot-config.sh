#!/usr/bin/env bash
# Added: TMAIL-12 acceptance test for Dovecot configuration templates.
# Runs entirely on the templates in this directory — no live Dovecot needed,
# so the suite is safe in CI and on machines without dovecot installed.
#
# Verifies every requirement enumerated in TMAIL-12:
#   * IMAP, LMTP, and Sieve protocols enabled
#   * mail_location set to a maildir under the per-user home
#   * SSL/TLS required (port 993 is implied by ssl=required + IMAP)
#   * SASL auth socket exposed to Postfix
#   * PostgreSQL passdb/userdb wired up
#   * vmail user pinned to uid/gid 5000
#   * Argon2id password scheme (matches backend hashing)
#   * Setup script installs the Sieve + ManageSieve packages
#
# Usage: ./test-dovecot-config.sh
# Exit:  0 = all checks pass, 1 = at least one check failed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONF="${SCRIPT_DIR}/dovecot.conf.template"
SQL="${SCRIPT_DIR}/dovecot-sql.conf.ext.template"
SETUP="${SCRIPT_DIR}/setup-dovecot.sh"

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

# Added: assert_match_multiline — same, but treats the file as a single
# string so the regex can span newlines. Needed for `block { ... }` patterns
# whose opening brace and inner directive are on separate lines.
assert_match_multiline() {
    local file="$1" regex="$2" desc="$3"
    if grep -Pzoq "$regex" "$file" 2>/dev/null; then
        pass "$desc"
    else
        fail "$desc  (missing in $(basename "$file"))"
    fi
}

# Added: assert_unfolded — assert a regex matches the file after backslash
# line-continuations are joined. Needed for the SQL queries which wrap with
# trailing backslashes over multiple lines.
assert_unfolded() {
    local file="$1" regex="$2" desc="$3"
    # sed joins lines ending with backslash into one logical line
    if sed -e ':a' -e '/\\$/{N;s/\\\n/ /;ba' -e '}' "$file" | grep -Eq "$regex"; then
        pass "$desc"
    else
        fail "$desc  (missing in $(basename "$file") after line-unfolding)"
    fi
}

echo -e "${BOLD}TMAIL-12 — Dovecot configuration acceptance tests${RESET}"
echo "========================================="

# --- File existence ------------------------------------------------------
for f in "$CONF" "$SQL" "$SETUP"; do
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
echo "Protocols (TMAIL-12 requires IMAP + LMTP + Sieve):"
# Added: All three protocols must be on a single `protocols =` line so Dovecot
# starts the corresponding services. ManageSieve is registered as `sieve`.
assert_match "$CONF" '^protocols\s*=\s*.*\bimap\b'  "protocols line includes imap"
assert_match "$CONF" '^protocols\s*=\s*.*\blmtp\b'  "protocols line includes lmtp"
assert_match "$CONF" '^protocols\s*=\s*.*\bsieve\b' "protocols line includes sieve"

echo ""
echo "Sieve wiring:"
# Added: Server-side Sieve only fires at delivery when LMTP loads the sieve plugin.
assert_match_multiline "$CONF" 'protocol lmtp \{[^}]*mail_plugins[^}]*sieve' \
    "LMTP loads sieve plugin (filters fire at delivery)"
assert_match "$CONF" 'protocol sieve \{' \
    "ManageSieve protocol block defined (port 4190)"
assert_match "$CONF" 'sieve_dir\s*=\s*/var/mail/vhosts/%d/%n/sieve' \
    "Sieve scripts stored per-user under home"
assert_match "$CONF" 'sieve_extensions\s*=.*\+vacation' \
    "Vacation extension enabled (auto-responder)"

echo ""
echo "Mail storage:"
assert_match "$CONF" '^mail_location\s*=\s*maildir:' "mail_location is Maildir format"
assert_match "$CONF" 'mail_uid\s*=\s*vmail'          "mail_uid = vmail"
assert_match "$CONF" 'mail_gid\s*=\s*vmail'          "mail_gid = vmail"

echo ""
echo "TLS / port 993:"
assert_match "$CONF" '^ssl\s*=\s*required'           "ssl = required (993 enforced for IMAP)"
assert_match "$CONF" 'disable_plaintext_auth\s*=\s*yes' "disable_plaintext_auth = yes"
assert_match "$CONF" 'ssl_min_protocol\s*=\s*TLSv1\.[23]' "ssl_min_protocol is TLS 1.2 or 1.3"

echo ""
echo "SASL auth socket for Postfix:"
# Added: Postfix's smtpd_sasl_path expects this exact unix_listener path.
assert_match "$CONF" '/var/spool/postfix/private/auth' "SASL auth socket path for Postfix"
assert_match "$CONF" '/var/spool/postfix/private/dovecot-lmtp' "LMTP socket path for Postfix virtual_transport"
assert_match "$CONF" 'auth_mechanisms\s*=.*\bplain\b' "PLAIN auth mechanism enabled"

echo ""
echo "PostgreSQL user lookup:"
assert_match_multiline "$CONF" 'passdb \{[^}]*driver\s*=\s*sql' "passdb uses SQL driver"
assert_match_multiline "$CONF" 'userdb \{[^}]*driver\s*=\s*sql' "userdb uses SQL driver"
assert_match "$CONF" '/etc/dovecot/dovecot-sql\.conf\.ext'   "passdb/userdb point at dovecot-sql.conf.ext"
assert_match "$SQL"  '^driver\s*=\s*pgsql'           "SQL template uses pgsql driver"
assert_match "$SQL"  '^default_pass_scheme\s*=\s*ARGON2ID' "SQL template uses Argon2id (matches backend)"
assert_unfolded "$SQL" '^password_query\s*=.*FROM mailboxes' "password_query reads from mailboxes table"
assert_unfolded "$SQL" '^user_query\s*=.*5000 AS uid'  "user_query pins uid=5000 (vmail)"
assert_unfolded "$SQL" '^user_query\s*=.*5000 AS gid'  "user_query pins gid=5000 (vmail)"

echo ""
echo "vmail system user creation:"
# Added: Verify the setup script actually creates the uid/gid 5000 pair —
# the SQL user_query above is only effective if these exist on the host.
assert_match "$SETUP" 'groupadd -g 5000 vmail'        "setup script creates vmail group with gid=5000"
assert_match "$SETUP" 'useradd -u 5000 -g vmail'      "setup script creates vmail user with uid=5000"

echo ""
echo "Sieve packages installed by setup script:"
assert_match "$SETUP" 'dovecot-sieve'                 "apt-get installs dovecot-sieve"
assert_match "$SETUP" 'dovecot-managesieved'          "apt-get installs dovecot-managesieved"
assert_match "$SETUP" 'dovecot-pigeonhole'            "yum/dnf installs dovecot-pigeonhole (RPM equivalent)"

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
