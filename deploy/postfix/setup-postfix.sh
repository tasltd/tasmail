#!/usr/bin/env bash
# Added: Automated Postfix setup script for TASMail (TMAIL-11)
# Installs Postfix with PostgreSQL integration, configures virtual mailbox
# delivery via Dovecot LMTP, and enables SASL authentication.
#
# Usage: ./setup-postfix.sh --domain example.com --hostname mail.example.com \
#            --db-host localhost --db-name tasmail --db-user tasmail --db-pass tasmail
#
# Dependencies: apt-get (Debian/Ubuntu), PostgreSQL client tools
# Must be run as root (or with sudo)

set -euo pipefail

# Added: Color output helpers (consistent with other deploy scripts)
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

info()  { echo -e "${GREEN}[INFO]${RESET} $1"; }
error() { echo -e "${RED}[ERROR]${RESET} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${RESET} $1"; }

# Added: Script directory for resolving template paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Added: Default values
DOMAIN=""
HOSTNAME=""
DB_HOST="localhost"
DB_NAME="tasmail"
DB_USER="tasmail"
DB_PASS=""
SERVER_IP=""

# Added: Parse command-line arguments
usage() {
    echo "Usage: $0 --domain <domain> [OPTIONS]"
    echo ""
    echo "Required:"
    echo "  --domain <domain>        Mail domain (e.g., example.com)"
    echo ""
    echo "Optional:"
    echo "  --hostname <fqdn>        Mail server FQDN (default: mail.<domain>)"
    echo "  --db-host <host>         PostgreSQL host (default: localhost)"
    echo "  --db-name <name>         PostgreSQL database (default: tasmail)"
    echo "  --db-user <user>         PostgreSQL user (default: tasmail)"
    echo "  --db-pass <pass>         PostgreSQL password (required)"
    echo "  --server-ip <ip>         Public IP for mynetworks (auto-detected if omitted)"
    echo "  -h, --help               Show this help"
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --domain)     DOMAIN="$2";     shift 2 ;;
        --hostname)   HOSTNAME="$2";   shift 2 ;;
        --db-host)    DB_HOST="$2";    shift 2 ;;
        --db-name)    DB_NAME="$2";    shift 2 ;;
        --db-user)    DB_USER="$2";    shift 2 ;;
        --db-pass)    DB_PASS="$2";    shift 2 ;;
        --server-ip)  SERVER_IP="$2";  shift 2 ;;
        -h|--help)    usage ;;
        *)            error "Unknown argument: $1"; exit 1 ;;
    esac
done

# Added: Validate required arguments
if [[ -z "$DOMAIN" ]]; then
    error "--domain is required"
    echo "Run '$0 --help' for usage."
    exit 1
fi

if [[ -z "$DB_PASS" ]]; then
    error "--db-pass is required"
    echo "Run '$0 --help' for usage."
    exit 1
fi

# Added: Default hostname to mail.<domain> if not specified
HOSTNAME="${HOSTNAME:-mail.${DOMAIN}}"

# Added: Check root privileges
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root (or with sudo)."
    exit 1
fi

echo -e "${BOLD}TASMail Postfix Setup${RESET}"
echo "Domain:   ${DOMAIN}"
echo "Hostname: ${HOSTNAME}"
echo "Database: ${DB_USER}@${DB_HOST}/${DB_NAME}"
echo ""

# --------------------------------------------------------------------------
# Step 1: Install Postfix and PostgreSQL integration
# --------------------------------------------------------------------------
info "Installing Postfix packages..."

# Added: Pre-seed debconf to avoid interactive prompts during install
export DEBIAN_FRONTEND=noninteractive
debconf-set-selections <<< "postfix postfix/mailname string ${HOSTNAME}"
debconf-set-selections <<< "postfix postfix/main_mailer_type string 'Internet Site'"

if command -v apt-get &>/dev/null; then
    apt-get update -qq
    apt-get install -y -qq postfix postfix-pgsql
elif command -v dnf &>/dev/null; then
    dnf install -y postfix postfix-pgsql
elif command -v yum &>/dev/null; then
    yum install -y postfix postfix-pgsql
else
    error "Unsupported package manager. Install postfix and postfix-pgsql manually."
    exit 1
fi

info "Postfix packages installed."

# --------------------------------------------------------------------------
# Step 2: Create vmail system user for virtual mailbox storage
# --------------------------------------------------------------------------
info "Creating vmail user (uid=5000, gid=5000)..."

if ! getent group vmail &>/dev/null; then
    groupadd -g 5000 vmail
    info "Created vmail group (gid=5000)"
else
    warn "vmail group already exists"
fi

if ! id -u vmail &>/dev/null 2>&1; then
    useradd -u 5000 -g vmail -s /usr/sbin/nologin -d /var/mail/vhosts -M vmail
    info "Created vmail user (uid=5000)"
else
    warn "vmail user already exists"
fi

# --------------------------------------------------------------------------
# Step 3: Create virtual mailbox directory structure
# --------------------------------------------------------------------------
info "Creating mail directory structure..."
mkdir -p "/var/mail/vhosts/${DOMAIN}"
chown -R vmail:vmail /var/mail/vhosts
chmod -R 770 /var/mail/vhosts
info "Mail directories created at /var/mail/vhosts/${DOMAIN}"

# --------------------------------------------------------------------------
# Step 4: Deploy PostgreSQL lookup maps
# --------------------------------------------------------------------------
info "Creating PostgreSQL lookup maps..."

# Added: Helper to deploy a template with placeholder substitution
deploy_pgsql_map() {
    local template_name="$1"
    local dest_path="$2"

    if [[ -f "${SCRIPT_DIR}/${template_name}" ]]; then
        sed \
            -e "s|__DB_HOST__|${DB_HOST}|g" \
            -e "s|__DB_NAME__|${DB_NAME}|g" \
            -e "s|__DB_USER__|${DB_USER}|g" \
            -e "s|__DB_PASS__|${DB_PASS}|g" \
            "${SCRIPT_DIR}/${template_name}" > "${dest_path}"
    else
        error "Template not found: ${SCRIPT_DIR}/${template_name}"
        exit 1
    fi

    # Added: Restrict permissions — only root and postfix can read DB credentials
    chown root:postfix "${dest_path}"
    chmod 640 "${dest_path}"
    info "  Deployed ${dest_path}"
}

deploy_pgsql_map "virtual_mailbox_domains.cf.template" "/etc/postfix/pgsql-virtual-mailbox-domains.cf"
deploy_pgsql_map "virtual_mailbox_maps.cf.template"    "/etc/postfix/pgsql-virtual-mailbox-maps.cf"
deploy_pgsql_map "virtual_alias_maps.cf.template"      "/etc/postfix/pgsql-virtual-alias-maps.cf"

# --------------------------------------------------------------------------
# Step 5: Deploy main.cf from template
# --------------------------------------------------------------------------
info "Deploying main.cf..."

# Added: Back up existing main.cf if present
if [[ -f /etc/postfix/main.cf ]]; then
    cp /etc/postfix/main.cf "/etc/postfix/main.cf.bak.$(date +%Y%m%d%H%M%S)"
    warn "Backed up existing main.cf"
fi

# Added: Copy template and replace placeholders
sed \
    -e "s|example\\.com|${DOMAIN}|g" \
    -e "s|mail\\.example\\.com|${HOSTNAME}|g" \
    -e "s|YOUR_SERVER_IP|${SERVER_IP:-127.0.0.1}|g" \
    -e "s|pgsql:/etc/postfix/pgsql-virtual-mailbox-maps\\.cf|pgsql:/etc/postfix/pgsql-virtual-mailbox-maps.cf|g" \
    -e "s|pgsql:/etc/postfix/pgsql-virtual-alias-maps\\.cf|pgsql:/etc/postfix/pgsql-virtual-alias-maps.cf|g" \
    "${SCRIPT_DIR}/main.cf.template" > /etc/postfix/main.cf

# Added: Update virtual_mailbox_domains to use PostgreSQL lookup instead of static list
sed -i "s|^virtual_mailbox_domains = .*|virtual_mailbox_domains = pgsql:/etc/postfix/pgsql-virtual-mailbox-domains.cf|" /etc/postfix/main.cf

info "main.cf deployed to /etc/postfix/main.cf"

# --------------------------------------------------------------------------
# Step 6: Configure master.cf for submission port (587) with SASL
# --------------------------------------------------------------------------
info "Configuring master.cf for submission port (587)..."

MASTER_CF="/etc/postfix/master.cf"

# Added: Back up existing master.cf
if [[ -f "$MASTER_CF" ]]; then
    cp "$MASTER_CF" "${MASTER_CF}.bak.$(date +%Y%m%d%H%M%S)"
fi

# Added: Check if submission is already configured
if grep -q "^submission " "$MASTER_CF" 2>/dev/null; then
    warn "Submission port already configured in master.cf — skipping"
else
    # Added: Append submission service configuration
    cat >> "$MASTER_CF" << 'MASTER_EOF'

# Added: Submission service (port 587) — authenticated mail sending
submission inet n       -       y       -       -       smtpd
  -o syslog_name=postfix/submission
  -o smtpd_tls_security_level=encrypt
  -o smtpd_sasl_auth_enable=yes
  -o smtpd_tls_auth_only=yes
  -o smtpd_reject_unlisted_recipient=no
  -o smtpd_recipient_restrictions=permit_sasl_authenticated,reject
  -o milter_macro_daemon_name=ORIGINATING
MASTER_EOF
    info "Submission service added to master.cf"
fi

# --------------------------------------------------------------------------
# Step 7: Validate configuration and start Postfix
# --------------------------------------------------------------------------
info "Validating Postfix configuration..."
if postfix check 2>&1; then
    info "Postfix configuration is valid"
else
    error "Postfix configuration check failed — review errors above"
    exit 1
fi

info "Enabling and starting Postfix..."
systemctl enable postfix
systemctl restart postfix

# Added: Verify Postfix is running
if systemctl is-active --quiet postfix; then
    info "Postfix is running"
else
    error "Postfix failed to start — check: journalctl -u postfix -n 50"
    exit 1
fi

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
echo -e "${BOLD}Postfix Setup Complete${RESET}"
echo "========================================="
echo "  Domain:         ${DOMAIN}"
echo "  Hostname:       ${HOSTNAME}"
echo "  Virtual mail:   /var/mail/vhosts/${DOMAIN}"
echo "  SMTP port:      25 (inbound), 587 (submission)"
echo "  DB lookups:     /etc/postfix/pgsql-virtual-*.cf"
echo "  LMTP delivery:  unix:private/dovecot-lmtp"
echo "  SASL auth:      via Dovecot (private/auth)"
echo "========================================="
echo ""
info "Next steps:"
echo "  1. Run setup-dovecot.sh to configure Dovecot (LMTP + SASL)"
echo "  2. Run setup-tls.sh to obtain TLS certificates"
echo "  3. Run verify-dns.sh to check DNS records"
