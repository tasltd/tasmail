#!/usr/bin/env bash
# Added: TASMail backup script (TMAIL-40)
# PURPOSE: Creates compressed backups of the PostgreSQL database, config files, and maildir.
#          Retains the last 30 days of backups and logs all output.
# USAGE: sudo ./backup.sh
# TRIGGERED BY: tasmail-backup.timer (daily at 2:00 AM)

set -euo pipefail

# Added: Backup configuration
BACKUP_BASE="/var/backups/tasmail"
RETENTION_DAYS=30
LOG_FILE="/var/log/tasmail/backup.log"
TIMESTAMP="$(date '+%Y%m%d_%H%M%S')"
BACKUP_DIR="${BACKUP_BASE}/${TIMESTAMP}"

# Added: Database connection — sourced from environment file
ENV_FILE="/etc/tasmail/tasmail.env"

# Added: Redirect all output to log file (and stdout for interactive use)
mkdir -p "$(dirname "$LOG_FILE")"
exec > >(tee -a "$LOG_FILE") 2>&1

log_info()  { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [INFO] $1"; }
log_error() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [ERROR] $1"; }

log_info "========================================="
log_info "TASMail Backup — Starting"
log_info "========================================="

# Added: Load environment variables for DATABASE_URL
if [[ -f "$ENV_FILE" ]]; then
    # shellcheck disable=SC1090
    set -a
    source "$ENV_FILE"
    set +a
    log_info "Loaded environment from ${ENV_FILE}"
else
    log_error "Environment file not found: ${ENV_FILE}"
    exit 1
fi

# Added: Create backup directory
mkdir -p "$BACKUP_DIR"
log_info "Backup directory: ${BACKUP_DIR}"

# Added: Step 1 — PostgreSQL database dump
log_info "Backing up PostgreSQL database..."
DB_BACKUP="${BACKUP_DIR}/tasmail_db.sql.gz"
if pg_dump "$DATABASE_URL" | gzip > "$DB_BACKUP"; then
    log_info "Database backup complete: $(du -h "$DB_BACKUP" | cut -f1)"
else
    log_error "Database backup FAILED"
    exit 1
fi

# Added: Step 2 — Config files backup
log_info "Backing up configuration files..."
CONFIG_BACKUP="${BACKUP_DIR}/tasmail_config.tar.gz"
if tar -czf "$CONFIG_BACKUP" -C / etc/tasmail 2>/dev/null; then
    log_info "Config backup complete: $(du -h "$CONFIG_BACKUP" | cut -f1)"
else
    log_error "Config backup FAILED"
    exit 1
fi

# Added: Step 3 — Maildir backup (Dovecot mail storage)
log_info "Backing up maildir (/var/mail/)..."
MAIL_BACKUP="${BACKUP_DIR}/tasmail_maildir.tar.gz"
if [[ -d /var/mail ]]; then
    if tar -czf "$MAIL_BACKUP" -C / var/mail 2>/dev/null; then
        log_info "Maildir backup complete: $(du -h "$MAIL_BACKUP" | cut -f1)"
    else
        log_error "Maildir backup FAILED"
        exit 1
    fi
else
    log_info "No /var/mail directory found — skipping maildir backup"
fi

# Added: Step 4 — Attachment storage backup
ATTACHMENT_DIR="${ATTACHMENT_DIR:-/var/lib/tasmail/attachments}"
if [[ -d "$ATTACHMENT_DIR" ]]; then
    log_info "Backing up attachments (${ATTACHMENT_DIR})..."
    ATTACH_BACKUP="${BACKUP_DIR}/tasmail_attachments.tar.gz"
    if tar -czf "$ATTACH_BACKUP" -C "$(dirname "$ATTACHMENT_DIR")" "$(basename "$ATTACHMENT_DIR")" 2>/dev/null; then
        log_info "Attachments backup complete: $(du -h "$ATTACH_BACKUP" | cut -f1)"
    else
        log_error "Attachments backup FAILED"
        exit 1
    fi
fi

# Added: Step 5 — Create single combined archive for easy transfer
log_info "Creating combined backup archive..."
COMBINED="${BACKUP_BASE}/tasmail_backup_${TIMESTAMP}.tar.gz"
if tar -czf "$COMBINED" -C "$BACKUP_BASE" "$TIMESTAMP"; then
    log_info "Combined archive: ${COMBINED} ($(du -h "$COMBINED" | cut -f1))"
else
    log_error "Combined archive creation FAILED"
fi

# Added: Step 6 — Clean up individual backup directory (combined archive is sufficient)
rm -rf "$BACKUP_DIR"

# Added: Step 7 — Prune old backups beyond retention period
log_info "Pruning backups older than ${RETENTION_DAYS} days..."
PRUNED=$(find "$BACKUP_BASE" -name "tasmail_backup_*.tar.gz" -mtime +"$RETENTION_DAYS" -print -delete | wc -l)
log_info "Pruned ${PRUNED} old backup(s)"

log_info "========================================="
log_info "TASMail Backup — Complete"
log_info "========================================="
