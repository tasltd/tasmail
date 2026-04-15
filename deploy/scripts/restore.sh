#!/usr/bin/env bash
# Added: TASMail restore script (TMAIL-40)
# PURPOSE: Restores TASMail from a backup tarball — database, config files, and maildir.
# USAGE: sudo ./restore.sh /var/backups/tasmail/tasmail_backup_20260415_020000.tar.gz
# WARNING: This will OVERWRITE current data. Stop the backend service before restoring.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Added: Validate arguments
if [[ $# -ne 1 ]]; then
    log_error "Usage: $0 <backup-tarball>"
    log_error "Example: $0 /var/backups/tasmail/tasmail_backup_20260415_020000.tar.gz"
    exit 1
fi

BACKUP_TARBALL="$1"

if [[ ! -f "$BACKUP_TARBALL" ]]; then
    log_error "Backup file not found: ${BACKUP_TARBALL}"
    exit 1
fi

# Added: Verify running as root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root (or with sudo)"
    exit 1
fi

# Added: Load environment for DATABASE_URL
ENV_FILE="/etc/tasmail/tasmail.env"
if [[ -f "$ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    set +a
fi

# Added: Confirm destructive operation
log_warn "========================================="
log_warn "TASMail Restore — DESTRUCTIVE OPERATION"
log_warn "========================================="
log_warn "This will restore from: ${BACKUP_TARBALL}"
log_warn "Current database and maildir will be OVERWRITTEN."
echo ""
read -rp "Type 'yes' to continue: " CONFIRM
if [[ "$CONFIRM" != "yes" ]]; then
    log_info "Restore cancelled"
    exit 0
fi

# Added: Stop the backend service before restoring
log_info "Stopping tasmail-backend service..."
systemctl stop tasmail-backend || log_warn "Service was not running"

# Added: Extract backup to temporary directory
RESTORE_DIR=$(mktemp -d /tmp/tasmail-restore-XXXXXX)
log_info "Extracting backup to ${RESTORE_DIR}..."
tar -xzf "$BACKUP_TARBALL" -C "$RESTORE_DIR"

# Added: Find the timestamp subdirectory inside the archive
RESTORE_SUBDIR=$(find "$RESTORE_DIR" -mindepth 1 -maxdepth 1 -type d | head -1)
if [[ -z "$RESTORE_SUBDIR" ]]; then
    # Added: Flat archive — files are directly in RESTORE_DIR
    RESTORE_SUBDIR="$RESTORE_DIR"
fi

log_info "Restore source: ${RESTORE_SUBDIR}"

# Added: Step 1 — Restore PostgreSQL database
DB_BACKUP=$(find "$RESTORE_SUBDIR" -name "tasmail_db.sql.gz" | head -1)
if [[ -n "$DB_BACKUP" ]]; then
    log_info "Restoring PostgreSQL database..."
    # Added: Extract DATABASE_URL components for psql
    DB_NAME=$(echo "$DATABASE_URL" | sed -E 's|.*\/([^?]+).*|\1|')

    # Added: Drop and recreate database, then restore
    log_info "Dropping and recreating database '${DB_NAME}'..."
    psql "$DATABASE_URL" -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '${DB_NAME}' AND pid <> pg_backend_pid();" 2>/dev/null || true
    dropdb --if-exists "$DB_NAME" 2>/dev/null || true
    createdb "$DB_NAME" 2>/dev/null || true

    gunzip -c "$DB_BACKUP" | psql "$DATABASE_URL" > /dev/null 2>&1
    log_info "Database restore complete"
else
    log_warn "No database backup found in archive — skipping"
fi

# Added: Step 2 — Restore config files
CONFIG_BACKUP=$(find "$RESTORE_SUBDIR" -name "tasmail_config.tar.gz" | head -1)
if [[ -n "$CONFIG_BACKUP" ]]; then
    log_info "Restoring configuration files to /etc/tasmail/..."
    tar -xzf "$CONFIG_BACKUP" -C /
    log_info "Config restore complete"
else
    log_warn "No config backup found in archive — skipping"
fi

# Added: Step 3 — Restore maildir
MAIL_BACKUP=$(find "$RESTORE_SUBDIR" -name "tasmail_maildir.tar.gz" | head -1)
if [[ -n "$MAIL_BACKUP" ]]; then
    log_info "Restoring maildir to /var/mail/..."
    tar -xzf "$MAIL_BACKUP" -C /
    # Added: Fix ownership for Dovecot
    chown -R vmail:vmail /var/mail/ 2>/dev/null || true
    log_info "Maildir restore complete"
else
    log_warn "No maildir backup found in archive — skipping"
fi

# Added: Step 4 — Restore attachments
ATTACH_BACKUP=$(find "$RESTORE_SUBDIR" -name "tasmail_attachments.tar.gz" | head -1)
if [[ -n "$ATTACH_BACKUP" ]]; then
    ATTACHMENT_DIR="${ATTACHMENT_DIR:-/var/lib/tasmail/attachments}"
    log_info "Restoring attachments to ${ATTACHMENT_DIR}..."
    mkdir -p "$(dirname "$ATTACHMENT_DIR")"
    tar -xzf "$ATTACH_BACKUP" -C "$(dirname "$ATTACHMENT_DIR")"
    chown -R tasmail:tasmail "$(dirname "$ATTACHMENT_DIR")" 2>/dev/null || true
    log_info "Attachments restore complete"
else
    log_warn "No attachments backup found in archive — skipping"
fi

# Added: Clean up temp directory
rm -rf "$RESTORE_DIR"

# Added: Restart the backend service (migrations will run on startup if needed)
log_info "Starting tasmail-backend service..."
systemctl start tasmail-backend

# Added: Wait briefly and check service status
sleep 3
if systemctl is-active --quiet tasmail-backend; then
    log_info "tasmail-backend service is running"
else
    log_error "tasmail-backend service failed to start — check: journalctl -u tasmail-backend -n 50"
    exit 1
fi

log_info "========================================="
log_info "TASMail Restore — Complete"
log_info "========================================="
