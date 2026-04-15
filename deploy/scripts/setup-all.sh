#!/usr/bin/env bash
# Added: Master setup script for TASMail (TMAIL-11, TMAIL-12)
# Orchestrates the full mail server setup: prerequisites check, Postfix,
# Dovecot, TLS certificates, backend service, and DNS verification.
#
# Usage: ./setup-all.sh --domain example.com --hostname mail.example.com \
#            [--db-host localhost] [--db-name tasmail] [--db-user tasmail] \
#            [--db-pass tasmail] [--skip-tls] [--skip-dns]
#
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

# Added: Resolve deploy directory (parent of scripts/)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEPLOY_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${DEPLOY_DIR}/.." && pwd)"

# Added: Default values
DOMAIN=""
HOSTNAME=""
DB_HOST="localhost"
DB_NAME="tasmail"
DB_USER="tasmail"
DB_PASS=""
SKIP_TLS=false
SKIP_DNS=false

# Added: Parse command-line arguments
usage() {
    echo "Usage: $0 --domain <domain> --hostname <fqdn> [OPTIONS]"
    echo ""
    echo "Required:"
    echo "  --domain <domain>        Mail domain (e.g., example.com)"
    echo "  --hostname <fqdn>        Mail server FQDN (e.g., mail.example.com)"
    echo ""
    echo "Optional:"
    echo "  --db-host <host>         PostgreSQL host (default: localhost)"
    echo "  --db-name <name>         PostgreSQL database (default: tasmail)"
    echo "  --db-user <user>         PostgreSQL user (default: tasmail)"
    echo "  --db-pass <pass>         PostgreSQL password (prompted if omitted)"
    echo "  --skip-tls               Skip TLS certificate setup"
    echo "  --skip-dns               Skip DNS verification"
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
        --skip-tls)   SKIP_TLS=true;   shift ;;
        --skip-dns)   SKIP_DNS=true;   shift ;;
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

if [[ -z "$HOSTNAME" ]]; then
    error "--hostname is required"
    echo "Run '$0 --help' for usage."
    exit 1
fi

# Added: Prompt for DB password if not provided on command line
if [[ -z "$DB_PASS" ]]; then
    read -rsp "PostgreSQL password for ${DB_USER}: " DB_PASS
    echo ""
    if [[ -z "$DB_PASS" ]]; then
        error "Database password cannot be empty."
        exit 1
    fi
fi

# Added: Check root privileges
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root (or with sudo)."
    exit 1
fi

echo -e "${BOLD}=========================================${RESET}"
echo -e "${BOLD}TASMail Full Server Setup${RESET}"
echo -e "${BOLD}=========================================${RESET}"
echo "Domain:     ${DOMAIN}"
echo "Hostname:   ${HOSTNAME}"
echo "Database:   ${DB_USER}@${DB_HOST}/${DB_NAME}"
echo "Skip TLS:   ${SKIP_TLS}"
echo "Skip DNS:   ${SKIP_DNS}"
echo "Started:    $(date '+%Y-%m-%d %H:%M:%S')"
echo ""

# --------------------------------------------------------------------------
# Step 1: Check prerequisites
# --------------------------------------------------------------------------
info "Checking prerequisites..."

# Added: Verify operating system is supported (Debian/Ubuntu recommended)
if [[ -f /etc/os-release ]]; then
    # shellcheck source=/dev/null
    source /etc/os-release
    info "OS: ${PRETTY_NAME:-${ID} ${VERSION_ID}}"
else
    warn "Cannot detect OS version — proceeding anyway"
fi

# Added: Check required tools
MISSING_TOOLS=()
for tool in systemctl postconf doveconf psql; do
    # Added: postconf and doveconf may not exist yet (will be installed)
    if [[ "$tool" == "postconf" || "$tool" == "doveconf" ]]; then
        continue
    fi
    if ! command -v "$tool" &>/dev/null; then
        MISSING_TOOLS+=("$tool")
    fi
done

if [[ ${#MISSING_TOOLS[@]} -gt 0 ]]; then
    error "Missing required tools: ${MISSING_TOOLS[*]}"
    error "Install them before running this script."
    exit 1
fi

# Added: Verify PostgreSQL is running and accessible
info "Checking PostgreSQL connectivity..."
if PGPASSWORD="$DB_PASS" psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1;" > /dev/null 2>&1; then
    info "PostgreSQL connection successful"
else
    error "Cannot connect to PostgreSQL at ${DB_HOST}/${DB_NAME} as ${DB_USER}"
    error "Ensure PostgreSQL is running and credentials are correct."
    exit 1
fi

# Added: Verify the mailboxes table exists (schema must be migrated)
if PGPASSWORD="$DB_PASS" psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1 FROM mailboxes LIMIT 0;" > /dev/null 2>&1; then
    info "Database schema verified (mailboxes table exists)"
else
    warn "mailboxes table not found — run backend migrations first:"
    warn "  cd ${REPO_DIR}/backend && cargo run  (auto-runs sqlx migrations)"
fi

# Added: Check available disk space (warn if < 5GB)
AVAIL_KB=$(df /var/mail 2>/dev/null | awk 'NR==2 {print $4}' || echo "0")
AVAIL_GB=$((AVAIL_KB / 1024 / 1024))
if [[ $AVAIL_GB -lt 5 ]]; then
    warn "Low disk space on /var/mail: ${AVAIL_GB}GB available (5GB recommended minimum)"
else
    info "Disk space: ${AVAIL_GB}GB available on /var/mail"
fi

echo ""

# --------------------------------------------------------------------------
# Step 2: Run Postfix setup
# --------------------------------------------------------------------------
info "========================================="
info "Step 2/6: Setting up Postfix"
info "========================================="

POSTFIX_SCRIPT="${DEPLOY_DIR}/postfix/setup-postfix.sh"
if [[ -x "$POSTFIX_SCRIPT" ]]; then
    "$POSTFIX_SCRIPT" \
        --domain "$DOMAIN" \
        --hostname "$HOSTNAME" \
        --db-host "$DB_HOST" \
        --db-name "$DB_NAME" \
        --db-user "$DB_USER" \
        --db-pass "$DB_PASS"
else
    error "Postfix setup script not found or not executable: ${POSTFIX_SCRIPT}"
    exit 1
fi

echo ""

# --------------------------------------------------------------------------
# Step 3: Run Dovecot setup
# --------------------------------------------------------------------------
info "========================================="
info "Step 3/6: Setting up Dovecot"
info "========================================="

DOVECOT_SCRIPT="${DEPLOY_DIR}/dovecot/setup-dovecot.sh"
if [[ -x "$DOVECOT_SCRIPT" ]]; then
    "$DOVECOT_SCRIPT" \
        --domain "$DOMAIN" \
        --hostname "$HOSTNAME" \
        --db-host "$DB_HOST" \
        --db-name "$DB_NAME" \
        --db-user "$DB_USER" \
        --db-pass "$DB_PASS"
else
    error "Dovecot setup script not found or not executable: ${DOVECOT_SCRIPT}"
    exit 1
fi

echo ""

# --------------------------------------------------------------------------
# Step 4: Run TLS setup
# --------------------------------------------------------------------------
if [[ "$SKIP_TLS" == "true" ]]; then
    warn "Skipping TLS setup (--skip-tls)"
else
    info "========================================="
    info "Step 4/6: Setting up TLS certificates"
    info "========================================="

    TLS_SCRIPT="${DEPLOY_DIR}/tls/setup-tls.sh"
    if [[ -x "$TLS_SCRIPT" ]]; then
        "$TLS_SCRIPT" "$HOSTNAME"
    else
        error "TLS setup script not found or not executable: ${TLS_SCRIPT}"
        error "Run manually: ${TLS_SCRIPT} ${HOSTNAME}"
    fi
fi

echo ""

# --------------------------------------------------------------------------
# Step 5: Create tasmail system user and install backend
# --------------------------------------------------------------------------
info "========================================="
info "Step 5/6: Setting up TASMail backend service"
info "========================================="

# Added: Create tasmail system user for the backend process
if ! getent group tasmail &>/dev/null; then
    groupadd --system tasmail
    info "Created tasmail group"
else
    info "tasmail group already exists"
fi

if ! id -u tasmail &>/dev/null 2>&1; then
    useradd --system --gid tasmail --shell /usr/sbin/nologin \
        --home-dir /var/lib/tasmail --create-home tasmail
    info "Created tasmail user"
else
    info "tasmail user already exists"
fi

# Added: Create required directories for the backend
mkdir -p /var/lib/tasmail/attachments
mkdir -p /var/log/tasmail
mkdir -p /etc/tasmail
chown -R tasmail:tasmail /var/lib/tasmail /var/log/tasmail
chown root:tasmail /etc/tasmail
chmod 750 /etc/tasmail

# Added: Deploy backend binary if a release build exists
BACKEND_BIN="${REPO_DIR}/backend/target/release/tasmail"
if [[ -f "$BACKEND_BIN" ]]; then
    cp "$BACKEND_BIN" /usr/local/bin/tasmail
    chmod 755 /usr/local/bin/tasmail
    info "Backend binary installed to /usr/local/bin/tasmail"
else
    warn "No release binary found at ${BACKEND_BIN}"
    warn "Build with: cd ${REPO_DIR}/backend && cargo build --release"
fi

# Added: Deploy environment file if not already present
ENV_FILE="/etc/tasmail/tasmail.env"
if [[ ! -f "$ENV_FILE" ]]; then
    if [[ -f "${DEPLOY_DIR}/tasmail.env.example" ]]; then
        cp "${DEPLOY_DIR}/tasmail.env.example" "$ENV_FILE"
        # Added: Fill in the known values
        sed -i \
            -e "s|CHANGE_ME_STRONG_PASSWORD|${DB_PASS}|g" \
            -e "s|localhost/tasmail|${DB_HOST}/${DB_NAME}|g" \
            "$ENV_FILE"
        chown root:tasmail "$ENV_FILE"
        chmod 640 "$ENV_FILE"
        info "Environment file deployed to ${ENV_FILE}"
        warn "Review ${ENV_FILE} and set JWT_SECRET before starting the backend"
    fi
else
    info "Environment file already exists at ${ENV_FILE}"
fi

# Added: Install systemd service files
for SVC_FILE in tasmail-backend.service tasmail-backup.service tasmail-backup.timer; do
    SRC="${DEPLOY_DIR}/systemd/${SVC_FILE}"
    DEST="/etc/systemd/system/${SVC_FILE}"
    if [[ -f "$SRC" ]]; then
        cp "$SRC" "$DEST"
        info "Installed ${DEST}"
    fi
done

systemctl daemon-reload
systemctl enable tasmail-backend.service
info "tasmail-backend.service enabled (start after configuring ${ENV_FILE})"

# Added: Enable backup timer
if [[ -f /etc/systemd/system/tasmail-backup.timer ]]; then
    systemctl enable tasmail-backup.timer
    info "tasmail-backup.timer enabled"
fi

echo ""

# --------------------------------------------------------------------------
# Step 6: DNS verification
# --------------------------------------------------------------------------
if [[ "$SKIP_DNS" == "true" ]]; then
    warn "Skipping DNS verification (--skip-dns)"
else
    info "========================================="
    info "Step 6/6: Verifying DNS records"
    info "========================================="

    DNS_SCRIPT="${DEPLOY_DIR}/dns/verify-dns.sh"
    if [[ -x "$DNS_SCRIPT" ]]; then
        # Added: Run DNS check but don't fail the whole setup if records are missing
        "$DNS_SCRIPT" "$DOMAIN" "$HOSTNAME" || warn "Some DNS records are missing — review output above"
    else
        warn "DNS verification script not found: ${DNS_SCRIPT}"
    fi
fi

echo ""

# --------------------------------------------------------------------------
# Final Summary
# --------------------------------------------------------------------------
echo -e "${BOLD}=========================================${RESET}"
echo -e "${BOLD}TASMail Setup Complete${RESET}"
echo -e "${BOLD}=========================================${RESET}"
echo ""
echo "  Domain:           ${DOMAIN}"
echo "  Mail hostname:    ${HOSTNAME}"
echo "  Database:         ${DB_USER}@${DB_HOST}/${DB_NAME}"
echo ""
echo "  Services installed:"
echo "    - Postfix       (SMTP, port 25 + 587)"
echo "    - Dovecot       (IMAP, port 993)"
echo "    - TASMail       (API backend, port 3000)"
echo ""
echo "  Mail storage:     /var/mail/vhosts/${DOMAIN}/"
echo "  Backend data:     /var/lib/tasmail/"
echo "  Configuration:    /etc/tasmail/tasmail.env"
echo "  Systemd service:  tasmail-backend.service"
echo ""
echo -e "${BOLD}Remaining manual steps:${RESET}"
echo "  1. Review and update /etc/tasmail/tasmail.env (set JWT_SECRET)"
echo "  2. Start the backend: systemctl start tasmail-backend"
echo "  3. Create the first admin user via the backend API"
echo "  4. Configure DNS records (MX, SPF, DKIM, DMARC) if not done"
echo "  5. Test mail flow: send a test email to user@${DOMAIN}"
echo ""
info "Completed at $(date '+%Y-%m-%d %H:%M:%S')"
