#!/usr/bin/env bash
# Added: TASMail backup verification script (TMAIL-42)
# PURPOSE: Prove that the most recent combined backup is *restorable* — not
#   just that the file exists. Verifies tar integrity, gzip integrity of the
#   embedded pg_dump, and (when possible) round-trips the SQL into a sandbox
#   PostgreSQL database, asserting the schema_migrations table contains rows.
# USAGE:
#   sudo ./backup-verify.sh                    # auto-pick newest combined archive
#   sudo ./backup-verify.sh /path/to/archive.tar.gz
#   BACKUP_VERIFY_DB_RESTORE=0 ./backup-verify.sh   # skip the DB sandbox step
# EXIT CODES:
#   0 — verification passed
#   1 — verification failed (logged with details)
# TRIGGERED BY: tasmail-backup-verify.timer (weekly, Sunday 03:00)

set -euo pipefail

BACKUP_BASE="${BACKUP_BASE:-/var/backups/tasmail}"
COMBINED_DIR="${BACKUP_BASE}/combined"
LOG_FILE="${BACKUP_VERIFY_LOG_FILE:-/var/log/tasmail/backup-verify.log}"
ENV_FILE="${BACKUP_ENV_FILE:-/etc/tasmail/tasmail.env}"
DO_DB_RESTORE="${BACKUP_VERIFY_DB_RESTORE:-1}"   # 1 = restore into sandbox; 0 = skip
SANDBOX_DB="${BACKUP_VERIFY_SANDBOX_DB:-tasmail_backup_verify}"

mkdir -p "$(dirname "$LOG_FILE")"
exec > >(tee -a "$LOG_FILE") 2>&1

log_info()  { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [INFO] $1"; }
log_warn()  { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [WARN] $1"; }
log_error() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] [ERROR] $1"; }

log_info "========================================="
log_info "TASMail Backup Verify — Starting"
log_info "========================================="

# Argument: explicit archive path, else newest combined archive
if [[ $# -ge 1 ]]; then
    ARCHIVE="$1"
else
    ARCHIVE=$(find "$COMBINED_DIR" -maxdepth 1 -name "tasmail_backup_*.tar.gz" -printf '%T@ %p\n' 2>/dev/null \
        | sort -nr | head -1 | cut -d' ' -f2-)
fi

if [[ -z "${ARCHIVE:-}" || ! -f "$ARCHIVE" ]]; then
    log_error "No backup archive found (looked in ${COMBINED_DIR})"
    exit 1
fi
log_info "Verifying: ${ARCHIVE} ($(du -h "$ARCHIVE" | cut -f1))"

# ----------------------------------------------------------------------------
# 1. Tar integrity — listing must succeed end-to-end.
# ----------------------------------------------------------------------------
log_info "[1/4] Checking tar integrity..."
if ! tar -tzf "$ARCHIVE" > /dev/null; then
    log_error "Tar integrity check FAILED — archive is corrupted"
    exit 1
fi
log_info "      tar -tzf OK"

# ----------------------------------------------------------------------------
# 2. Manifest + gzip integrity of embedded artefacts.
# ----------------------------------------------------------------------------
STAGE=$(mktemp -d /tmp/tasmail-verify-XXXXXX)
trap 'rm -rf "$STAGE"' EXIT
tar -xzf "$ARCHIVE" -C "$STAGE"

MANIFEST="${STAGE}/MANIFEST"
if [[ ! -f "$MANIFEST" ]]; then
    log_warn "No MANIFEST in archive (older format) — continuing with discovery"
fi

DB_DUMP=$(find "$STAGE" -maxdepth 1 -name "tasmail_db_*.sql.gz" -o -name "tasmail_db.sql.gz" | head -1)
if [[ -z "$DB_DUMP" ]]; then
    log_error "No pg_dump archive found inside combined backup"
    exit 1
fi

log_info "[2/4] Checking gzip integrity of pg_dump..."
if ! gunzip -t "$DB_DUMP"; then
    log_error "gunzip -t failed on ${DB_DUMP} — dump is corrupted"
    exit 1
fi
log_info "      gunzip -t OK ($(du -h "$DB_DUMP" | cut -f1))"

# Check the inner SQL has plausible content.
log_info "[3/4] Checking pg_dump content..."
HEAD_BYTES=$(gunzip -c "$DB_DUMP" | head -c 4096 || true)
if ! echo "$HEAD_BYTES" | grep -q "PostgreSQL database dump"; then
    log_error "pg_dump header missing 'PostgreSQL database dump' marker — not a valid dump?"
    exit 1
fi
log_info "      pg_dump header OK"

# ----------------------------------------------------------------------------
# 3. Optional: round-trip the SQL into a sandbox DB and assert it loads.
#    Skipped when BACKUP_VERIFY_DB_RESTORE=0 or when no env file is present.
# ----------------------------------------------------------------------------
if [[ "$DO_DB_RESTORE" == "1" ]]; then
    if [[ -f "$ENV_FILE" ]]; then
        # shellcheck disable=SC1090
        set -a; source "$ENV_FILE"; set +a
    fi
    if [[ -z "${DATABASE_URL:-}" ]]; then
        log_warn "DATABASE_URL not set — skipping sandbox restore step"
    elif ! command -v psql >/dev/null 2>&1 || ! command -v createdb >/dev/null 2>&1; then
        log_warn "psql/createdb not on PATH — skipping sandbox restore step"
    else
        log_info "[4/4] Restoring into sandbox database '${SANDBOX_DB}'..."
        # Build a sandbox URL by swapping the database segment of DATABASE_URL.
        # URL shape: scheme://user[:pass]@host[:port]/dbname[?args]
        if [[ "$DATABASE_URL" =~ ^(.*//[^/]+/)([^?]+)(.*)$ ]]; then
            SANDBOX_URL="${BASH_REMATCH[1]}${SANDBOX_DB}${BASH_REMATCH[3]}"
        else
            log_error "Could not parse DATABASE_URL to build sandbox URL: ${DATABASE_URL}"
            exit 1
        fi
        # Drop any leftover sandbox from a previous failed run.
        psql "$DATABASE_URL" -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '${SANDBOX_DB}' AND pid <> pg_backend_pid();" >/dev/null 2>&1 || true
        dropdb --if-exists "$SANDBOX_DB" >/dev/null 2>&1 || true
        createdb "$SANDBOX_DB"
        if ! gunzip -c "$DB_DUMP" | psql "$SANDBOX_URL" >/dev/null; then
            log_error "Sandbox restore FAILED — backup is not restorable"
            dropdb --if-exists "$SANDBOX_DB" >/dev/null 2>&1 || true
            exit 1
        fi
        # Assert the migrations table is present and populated — proves it's a
        # real TASMail DB, not just empty SQL that happened to apply cleanly.
        ROW_COUNT=$(psql "$SANDBOX_URL" -At -c "SELECT COUNT(*) FROM _sqlx_migrations;" 2>/dev/null || echo "0")
        if [[ "${ROW_COUNT:-0}" -lt 1 ]]; then
            log_error "Sandbox DB has no _sqlx_migrations rows — restore looks empty"
            dropdb --if-exists "$SANDBOX_DB" >/dev/null 2>&1 || true
            exit 1
        fi
        log_info "      Sandbox restore OK (_sqlx_migrations rows: ${ROW_COUNT})"
        dropdb --if-exists "$SANDBOX_DB" >/dev/null 2>&1 || true
    fi
else
    log_info "[4/4] Sandbox DB restore skipped (BACKUP_VERIFY_DB_RESTORE=0)"
fi

log_info "========================================="
log_info "TASMail Backup Verify — PASS"
log_info "========================================="
exit 0
