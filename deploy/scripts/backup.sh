#!/usr/bin/env bash
# Added: TASMail backup script (TMAIL-40, TMAIL-42)
# PURPOSE: Daily backups for TASMail.
#   - PostgreSQL: full pg_dump every run, gzip-compressed.
#   - Maildir + attachments: incremental rsync into a "current" snapshot,
#     then a date-stamped hardlink snapshot (`--link-dest`) for cheap PITR.
#   - Config: small tarball each run.
#   - Combined dated tarball for off-site transport.
#   - 30-day retention on dated snapshots and combined archives.
#   - Optional off-site push (rclone or rsync over SSH).
# USAGE: sudo ./backup.sh
# TRIGGERED BY: tasmail-backup.timer (daily at 2:00 AM)

set -euo pipefail

# Added: Backup configuration
BACKUP_BASE="${BACKUP_BASE:-/var/backups/tasmail}"
RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-30}"
LOG_FILE="${BACKUP_LOG_FILE:-/var/log/tasmail/backup.log}"
TIMESTAMP="$(date '+%Y%m%d_%H%M%S')"
DATE_TAG="$(date '+%Y-%m-%d')"

# Added: Working layout
#   $BACKUP_BASE/db/                 — gz'd pg_dump files (one per run)
#   $BACKUP_BASE/maildir/current/    — live rsync mirror of /var/mail
#   $BACKUP_BASE/maildir/YYYY-MM-DD/ — hardlink snapshots
#   $BACKUP_BASE/attachments/...     — same layout as maildir
#   $BACKUP_BASE/config/...          — tarballs of /etc/tasmail
#   $BACKUP_BASE/combined/tasmail_backup_<ts>.tar.gz  — transportable bundle
DB_DIR="${BACKUP_BASE}/db"
MAILDIR_BASE="${BACKUP_BASE}/maildir"
MAILDIR_CURRENT="${MAILDIR_BASE}/current"
MAILDIR_SNAPSHOT="${MAILDIR_BASE}/${DATE_TAG}"
ATTACH_BASE="${BACKUP_BASE}/attachments"
ATTACH_CURRENT="${ATTACH_BASE}/current"
ATTACH_SNAPSHOT="${ATTACH_BASE}/${DATE_TAG}"
CONFIG_DIR="${BACKUP_BASE}/config"
COMBINED_DIR="${BACKUP_BASE}/combined"

# Added: Database connection — sourced from environment file (overridable for tests)
ENV_FILE="${BACKUP_ENV_FILE:-/etc/tasmail/tasmail.env}"

# Added: Off-site configuration (NONE by default; opt-in)
BACKUP_OFFSITE_TYPE="${BACKUP_OFFSITE_TYPE:-none}"   # none | rclone | rsync
BACKUP_OFFSITE_DEST="${BACKUP_OFFSITE_DEST:-}"
BACKUP_OFFSITE_OPTS="${BACKUP_OFFSITE_OPTS:-}"

# Added: Source paths (overridable for tests)
MAILDIR_SRC="${MAILDIR_SRC:-/var/mail}"
ATTACHMENT_DIR="${ATTACHMENT_DIR:-/var/lib/tasmail/attachments}"
CONFIG_SRC="${CONFIG_SRC:-/etc/tasmail}"

mkdir -p "$(dirname "$LOG_FILE")"
mkdir -p "$DB_DIR" "$MAILDIR_BASE" "$ATTACH_BASE" "$CONFIG_DIR" "$COMBINED_DIR"

# Added: Tee output to log (stdout retained for systemd journal)
exec > >(tee -a "$LOG_FILE") 2>&1

log_info()  { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [INFO] $1"; }
log_warn()  { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [WARN] $1"; }
log_error() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [ERROR] $1"; }

log_info "========================================="
log_info "TASMail Backup — Starting (run ${TIMESTAMP})"
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

if [[ -z "${DATABASE_URL:-}" ]]; then
    log_error "DATABASE_URL is not set after sourcing ${ENV_FILE}"
    exit 1
fi

# ----------------------------------------------------------------------------
# Step 1 — PostgreSQL dump
# ----------------------------------------------------------------------------
log_info "Backing up PostgreSQL database..."
DB_BACKUP="${DB_DIR}/tasmail_db_${TIMESTAMP}.sql.gz"
if pg_dump "$DATABASE_URL" | gzip > "$DB_BACKUP"; then
    log_info "Database backup complete: $(du -h "$DB_BACKUP" | cut -f1) -> ${DB_BACKUP}"
else
    log_error "Database backup FAILED"
    exit 1
fi

# ----------------------------------------------------------------------------
# Step 2 — Config tarball
# ----------------------------------------------------------------------------
log_info "Backing up configuration files (${CONFIG_SRC})..."
CONFIG_BACKUP="${CONFIG_DIR}/tasmail_config_${TIMESTAMP}.tar.gz"
if [[ -d "$CONFIG_SRC" ]]; then
    if tar -czf "$CONFIG_BACKUP" -C "$(dirname "$CONFIG_SRC")" "$(basename "$CONFIG_SRC")"; then
        log_info "Config backup complete: $(du -h "$CONFIG_BACKUP" | cut -f1)"
    else
        log_error "Config backup FAILED"
        exit 1
    fi
else
    log_warn "Config source ${CONFIG_SRC} not found — skipping"
    CONFIG_BACKUP=""
fi

# ----------------------------------------------------------------------------
# Step 3 — Incremental Maildir rsync (with hardlink snapshot)
# ----------------------------------------------------------------------------
# Why incremental: full nightly tar of a multi-GB maildir is expensive in I/O,
# disk space, and time. rsync --link-dest only transfers/dedupes changed files;
# unchanged files are hardlinked across snapshots so each dated snapshot
# behaves like a full point-in-time view at the cost of only the daily delta.
incremental_rsync() {
    local label="$1" src="$2" current="$3" snapshot="$4"
    if [[ ! -d "$src" ]]; then
        log_warn "${label} source ${src} not found — skipping"
        return 0
    fi
    if ! command -v rsync >/dev/null 2>&1; then
        log_error "rsync is not installed — cannot perform incremental ${label} backup"
        return 1
    fi
    mkdir -p "$current"
    local link_dest_args=()
    if [[ -d "$current" ]] && [[ -n "$(ls -A "$current" 2>/dev/null || true)" ]]; then
        link_dest_args=(--link-dest="$current")
    fi
    log_info "Incremental ${label} rsync: ${src}/ -> ${snapshot}/"
    mkdir -p "$snapshot"
    rsync -aH --delete --numeric-ids \
        "${link_dest_args[@]}" \
        "${src%/}/" "${snapshot%/}/"
    # Refresh the "current" pointer so the next run uses today's snapshot as the
    # link-dest base — cheapest way to keep the deduped chain going.
    rm -rf "$current"
    cp -al "$snapshot" "$current"
    log_info "${label} snapshot size (apparent): $(du -sh "$snapshot" | cut -f1)"
}

incremental_rsync "maildir" "$MAILDIR_SRC" "$MAILDIR_CURRENT" "$MAILDIR_SNAPSHOT" || exit 1

# ----------------------------------------------------------------------------
# Step 4 — Incremental attachment store rsync (same pattern)
# ----------------------------------------------------------------------------
incremental_rsync "attachments" "$ATTACHMENT_DIR" "$ATTACH_CURRENT" "$ATTACH_SNAPSHOT" || exit 1

# ----------------------------------------------------------------------------
# Step 5 — Build a transportable combined tarball
# ----------------------------------------------------------------------------
# The combined tarball is what's pushed off-site and what backup-verify.sh
# exercises. It contains the day's pg_dump + config tar + tar of the day's
# maildir/attachments snapshots (tar follows hardlinks, so this is a real
# point-in-time bundle even though the on-disk snapshot is hardlink-deduped).
log_info "Creating combined archive for off-site transport..."
COMBINED_BUILD=$(mktemp -d "${COMBINED_DIR}/.build-XXXXXX")
COMBINED="${COMBINED_DIR}/tasmail_backup_${TIMESTAMP}.tar.gz"
cp "$DB_BACKUP" "$COMBINED_BUILD/"
[[ -n "$CONFIG_BACKUP" ]] && cp "$CONFIG_BACKUP" "$COMBINED_BUILD/"
if [[ -d "$MAILDIR_SNAPSHOT" ]]; then
    tar -czf "${COMBINED_BUILD}/tasmail_maildir.tar.gz" -C "$MAILDIR_BASE" "${DATE_TAG}"
fi
if [[ -d "$ATTACH_SNAPSHOT" ]]; then
    tar -czf "${COMBINED_BUILD}/tasmail_attachments.tar.gz" -C "$ATTACH_BASE" "${DATE_TAG}"
fi
# Manifest helps offsite + verify scripts know what to expect.
{
    echo "timestamp=${TIMESTAMP}"
    echo "date=${DATE_TAG}"
    echo "host=$(hostname -f 2>/dev/null || hostname)"
    echo "db_dump=$(basename "$DB_BACKUP")"
    [[ -n "$CONFIG_BACKUP" ]] && echo "config=$(basename "$CONFIG_BACKUP")"
    [[ -d "$MAILDIR_SNAPSHOT" ]] && echo "maildir=tasmail_maildir.tar.gz"
    [[ -d "$ATTACH_SNAPSHOT" ]] && echo "attachments=tasmail_attachments.tar.gz"
} > "${COMBINED_BUILD}/MANIFEST"

if tar -czf "$COMBINED" -C "$COMBINED_BUILD" .; then
    log_info "Combined archive: ${COMBINED} ($(du -h "$COMBINED" | cut -f1))"
else
    log_error "Combined archive creation FAILED"
    rm -rf "$COMBINED_BUILD"
    exit 1
fi
rm -rf "$COMBINED_BUILD"

# ----------------------------------------------------------------------------
# Step 6 — Off-site push (optional, env-driven)
# ----------------------------------------------------------------------------
push_offsite() {
    local archive="$1"
    case "$BACKUP_OFFSITE_TYPE" in
        none|"")
            log_info "Off-site push disabled (BACKUP_OFFSITE_TYPE=none) — skipping"
            return 0
            ;;
        rclone)
            if ! command -v rclone >/dev/null 2>&1; then
                log_error "BACKUP_OFFSITE_TYPE=rclone but rclone is not installed"
                return 1
            fi
            if [[ -z "$BACKUP_OFFSITE_DEST" ]]; then
                log_error "BACKUP_OFFSITE_DEST is empty — cannot rclone push"
                return 1
            fi
            log_info "Pushing ${archive} to rclone remote ${BACKUP_OFFSITE_DEST}..."
            # shellcheck disable=SC2086
            rclone copy ${BACKUP_OFFSITE_OPTS} "$archive" "$BACKUP_OFFSITE_DEST"
            log_info "Off-site rclone push complete"
            ;;
        rsync)
            if ! command -v rsync >/dev/null 2>&1; then
                log_error "BACKUP_OFFSITE_TYPE=rsync but rsync is not installed"
                return 1
            fi
            if [[ -z "$BACKUP_OFFSITE_DEST" ]]; then
                log_error "BACKUP_OFFSITE_DEST is empty — cannot rsync push"
                return 1
            fi
            log_info "Pushing ${archive} to rsync remote ${BACKUP_OFFSITE_DEST}..."
            # shellcheck disable=SC2086
            rsync -av ${BACKUP_OFFSITE_OPTS} "$archive" "$BACKUP_OFFSITE_DEST"
            log_info "Off-site rsync push complete"
            ;;
        *)
            log_error "Unknown BACKUP_OFFSITE_TYPE=${BACKUP_OFFSITE_TYPE} (expected: none|rclone|rsync)"
            return 1
            ;;
    esac
}
push_offsite "$COMBINED" || log_error "Off-site push failed — local backup retained"

# ----------------------------------------------------------------------------
# Step 7 — Retention pruning
# ----------------------------------------------------------------------------
log_info "Pruning artefacts older than ${RETENTION_DAYS} days..."
PRUNED_DB=$(find "$DB_DIR" -maxdepth 1 -name "tasmail_db_*.sql.gz" -mtime +"$RETENTION_DAYS" -print -delete | wc -l)
PRUNED_CFG=$(find "$CONFIG_DIR" -maxdepth 1 -name "tasmail_config_*.tar.gz" -mtime +"$RETENTION_DAYS" -print -delete | wc -l)
PRUNED_COMBINED=$(find "$COMBINED_DIR" -maxdepth 1 -name "tasmail_backup_*.tar.gz" -mtime +"$RETENTION_DAYS" -print -delete | wc -l)
# Prune dated maildir/attachment snapshots (skip the "current" pointer).
PRUNED_MAIL=$(find "$MAILDIR_BASE" -mindepth 1 -maxdepth 1 -type d ! -name current -mtime +"$RETENTION_DAYS" -print -exec rm -rf {} + | wc -l)
PRUNED_ATTACH=$(find "$ATTACH_BASE" -mindepth 1 -maxdepth 1 -type d ! -name current -mtime +"$RETENTION_DAYS" -print -exec rm -rf {} + | wc -l)
log_info "Pruned: db=${PRUNED_DB} config=${PRUNED_CFG} combined=${PRUNED_COMBINED} maildir=${PRUNED_MAIL} attachments=${PRUNED_ATTACH}"

log_info "========================================="
log_info "TASMail Backup — Complete"
log_info "========================================="
