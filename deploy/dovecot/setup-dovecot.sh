#!/usr/bin/env bash
# Added: Automated Dovecot setup script for TASMail (TMAIL-12)
# Installs Dovecot with IMAP, LMTP, and PostgreSQL authentication.
# Configures SASL socket for Postfix and LMTP for mail delivery.
#
# Usage: ./setup-dovecot.sh --domain example.com --hostname mail.example.com \
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

echo -e "${BOLD}TASMail Dovecot Setup${RESET}"
echo "Domain:   ${DOMAIN}"
echo "Hostname: ${HOSTNAME}"
echo "Database: ${DB_USER}@${DB_HOST}/${DB_NAME}"
echo ""

# --------------------------------------------------------------------------
# Step 1: Install Dovecot packages
# --------------------------------------------------------------------------
info "Installing Dovecot packages..."

if command -v apt-get &>/dev/null; then
    apt-get update -qq
    # Added: dovecot-sieve runs Sieve filters at LMTP delivery time;
    # dovecot-managesieved exposes port 4190 so SPA clients can upload rules.
    apt-get install -y -qq dovecot-core dovecot-imapd dovecot-lmtpd dovecot-pgsql \
        dovecot-sieve dovecot-managesieved
elif command -v dnf &>/dev/null; then
    # Added: On RHEL/Fedora the Sieve runtime ships in dovecot-pigeonhole
    dnf install -y dovecot dovecot-pgsql dovecot-pigeonhole
elif command -v yum &>/dev/null; then
    yum install -y dovecot dovecot-pgsql dovecot-pigeonhole
else
    error "Unsupported package manager. Install dovecot-core, dovecot-imapd, dovecot-lmtpd, dovecot-pgsql, dovecot-sieve, dovecot-managesieved manually."
    exit 1
fi

info "Dovecot packages installed."

# --------------------------------------------------------------------------
# Step 2: Ensure vmail user exists (shared with Postfix setup)
# --------------------------------------------------------------------------
info "Verifying vmail user (uid=5000, gid=5000)..."

if ! getent group vmail &>/dev/null; then
    groupadd -g 5000 vmail
    info "Created vmail group (gid=5000)"
else
    info "vmail group already exists"
fi

if ! id -u vmail &>/dev/null 2>&1; then
    useradd -u 5000 -g vmail -s /usr/sbin/nologin -d /var/mail/vhosts -M vmail
    info "Created vmail user (uid=5000)"
else
    info "vmail user already exists"
fi

# --------------------------------------------------------------------------
# Step 3: Create mail directory structure
# --------------------------------------------------------------------------
info "Creating mail directory structure..."
mkdir -p "/var/mail/vhosts/${DOMAIN}"
chown -R vmail:vmail /var/mail/vhosts
chmod -R 770 /var/mail/vhosts
info "Mail directories ready at /var/mail/vhosts/${DOMAIN}"

# --------------------------------------------------------------------------
# Step 4: Deploy dovecot.conf from template
# --------------------------------------------------------------------------
info "Deploying dovecot.conf..."

DOVECOT_CONF="/etc/dovecot/dovecot.conf"

# Added: Back up existing config if present
if [[ -f "$DOVECOT_CONF" ]]; then
    cp "$DOVECOT_CONF" "${DOVECOT_CONF}.bak.$(date +%Y%m%d%H%M%S)"
    warn "Backed up existing dovecot.conf"
fi

# Added: Copy template and replace domain/hostname placeholders
sed \
    -e "s|example\\.com|${DOMAIN}|g" \
    -e "s|mail\\.example\\.com|${HOSTNAME}|g" \
    -e "s|YOUR_DB_PASSWORD|${DB_PASS}|g" \
    "${SCRIPT_DIR}/dovecot.conf.template" > "$DOVECOT_CONF"

info "dovecot.conf deployed to ${DOVECOT_CONF}"

# --------------------------------------------------------------------------
# Step 5: Deploy dovecot-sql.conf.ext for PostgreSQL authentication
# --------------------------------------------------------------------------
info "Deploying dovecot-sql.conf.ext..."

SQL_CONF="/etc/dovecot/dovecot-sql.conf.ext"

# Added: Deploy from template with credential substitution
if [[ -f "${SCRIPT_DIR}/dovecot-sql.conf.ext.template" ]]; then
    sed \
        -e "s|__DB_HOST__|${DB_HOST}|g" \
        -e "s|__DB_NAME__|${DB_NAME}|g" \
        -e "s|__DB_USER__|${DB_USER}|g" \
        -e "s|__DB_PASS__|${DB_PASS}|g" \
        "${SCRIPT_DIR}/dovecot-sql.conf.ext.template" > "$SQL_CONF"
else
    error "Template not found: ${SCRIPT_DIR}/dovecot-sql.conf.ext.template"
    exit 1
fi

# Added: Restrict permissions — contains database credentials
chown root:root "$SQL_CONF"
chmod 600 "$SQL_CONF"
info "dovecot-sql.conf.ext deployed to ${SQL_CONF}"

# --------------------------------------------------------------------------
# Step 6: Create Postfix integration directories
# --------------------------------------------------------------------------
info "Setting up Postfix integration sockets..."

# Added: Ensure Postfix spool directory exists for LMTP and SASL sockets
POSTFIX_PRIVATE="/var/spool/postfix/private"
if [[ -d "$POSTFIX_PRIVATE" ]]; then
    info "Postfix private directory exists at ${POSTFIX_PRIVATE}"
else
    warn "Postfix private directory not found — install Postfix first"
    warn "LMTP and SASL sockets will be created when Dovecot starts"
fi

# --------------------------------------------------------------------------
# Step 7: Create quota warning script
# --------------------------------------------------------------------------
info "Installing quota warning script..."

QUOTA_SCRIPT="/usr/local/bin/quota-warning.sh"
cat > "$QUOTA_SCRIPT" << 'QUOTA_EOF'
#!/usr/bin/env bash
# Added: Dovecot quota warning script for TASMail (TMAIL-12)
# Called by Dovecot when a user approaches their mailbox quota limit.
# Arguments: $1 = percentage threshold, $2 = username (email)

PERCENT="$1"
USER="$2"

cat << MSG | /usr/lib/dovecot/dovecot-lda -d "$USER" -o "plugin/quota=maildir:User quota:noenforcing"
From: postmaster@$(hostname -d)
Subject: Mailbox quota warning - ${PERCENT}% used

Your mailbox is now ${PERCENT}% full. Please delete some messages or
contact your administrator to increase your quota.

If you reach 100%, you will not be able to receive new mail.
MSG
QUOTA_EOF

chmod +x "$QUOTA_SCRIPT"
chown root:root "$QUOTA_SCRIPT"
info "Quota warning script installed at ${QUOTA_SCRIPT}"

# --------------------------------------------------------------------------
# Step 8: Validate configuration and start Dovecot
# --------------------------------------------------------------------------
info "Validating Dovecot configuration..."
if doveconf -n > /dev/null 2>&1; then
    info "Dovecot configuration is valid"
else
    error "Dovecot configuration check failed:"
    doveconf -n 2>&1 | tail -20
    exit 1
fi

info "Enabling and starting Dovecot..."
systemctl enable dovecot
systemctl restart dovecot

# Added: Verify Dovecot is running
if systemctl is-active --quiet dovecot; then
    info "Dovecot is running"
else
    error "Dovecot failed to start — check: journalctl -u dovecot -n 50"
    exit 1
fi

# Added: Verify LMTP socket is available (may take a moment)
sleep 1
if [[ -S "/var/spool/postfix/private/dovecot-lmtp" ]]; then
    info "LMTP socket is active at /var/spool/postfix/private/dovecot-lmtp"
else
    warn "LMTP socket not yet created — it will appear after first mail delivery"
fi

if [[ -S "/var/spool/postfix/private/auth" ]]; then
    info "SASL auth socket is active at /var/spool/postfix/private/auth"
else
    warn "SASL auth socket not yet created — Postfix may need a restart"
fi

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
echo -e "${BOLD}Dovecot Setup Complete${RESET}"
echo "========================================="
echo "  Domain:         ${DOMAIN}"
echo "  Hostname:       ${HOSTNAME}"
echo "  Protocols:      IMAP (993/TLS), LMTP (socket), ManageSieve (4190/STARTTLS)"
echo "  Sieve:          Server-side filters at LMTP delivery, vacation responder ready"
echo "  Mail storage:   /var/mail/vhosts/${DOMAIN}/"
echo "  Auth backend:   PostgreSQL (${DB_HOST}/${DB_NAME})"
echo "  Pass scheme:    Argon2id"
echo "  LMTP socket:    /var/spool/postfix/private/dovecot-lmtp"
echo "  SASL socket:    /var/spool/postfix/private/auth"
echo "  SQL config:     /etc/dovecot/dovecot-sql.conf.ext"
echo "========================================="
echo ""
info "Next steps:"
echo "  1. Restart Postfix to pick up SASL/LMTP sockets: systemctl restart postfix"
echo "  2. Run setup-tls.sh to obtain TLS certificates"
echo "  3. Test IMAP login: openssl s_client -connect ${HOSTNAME}:993"
echo "  4. Test ManageSieve: openssl s_client -starttls sieve -connect ${HOSTNAME}:4190"
echo "  5. Test LMTP delivery: doveadm user '*'"
