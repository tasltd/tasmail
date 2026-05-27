#!/usr/bin/env bash
# Added: TASMail backup end-to-end test (TMAIL-42)
# PURPOSE: Exercises backup.sh + backup-verify.sh against a real (sandboxed)
#          PostgreSQL database with fake maildir/attachment data. Asserts:
#            - backup.sh creates the expected on-disk layout
#            - the combined archive is produced and contains a MANIFEST
#            - incremental rsync produces hardlink-deduped snapshots
#            - backup-verify.sh exits 0 against the produced archive
#            - off-site rsync mode pushes the archive to a local stand-in dest
#            - retention pruning removes artefacts older than the cutoff
#          This is the unit/integration test for the shell scripts — there is
#          no Rust or TS code to test for this task.
#
# USAGE:
#   ./test-backup.sh                          # uses TEST_DATABASE_URL or default
#   TEST_DATABASE_URL=postgres://... ./test-backup.sh
#
# REQUIREMENTS: bash, rsync, tar, gzip, psql/createdb/dropdb (a live postgres
# server reachable via TEST_DATABASE_URL).

set -euo pipefail

# ---- Test config ----------------------------------------------------------
TEST_DATABASE_URL="${TEST_DATABASE_URL:-postgres://tasmail:tasmail@localhost/tasmail_backup_test}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BACKUP_SH="${SCRIPT_DIR}/backup.sh"
VERIFY_SH="${SCRIPT_DIR}/backup-verify.sh"
WORK_ROOT="$(mktemp -d /tmp/tasmail-backup-test-XXXXXX)"

PASS=0
FAIL=0

c_green=$'\e[32m'; c_red=$'\e[31m'; c_blue=$'\e[34m'; c_reset=$'\e[0m'
ok()    { echo "${c_green}[PASS]${c_reset} $1"; PASS=$((PASS+1)); }
bad()   { echo "${c_red}[FAIL]${c_reset} $1"; FAIL=$((FAIL+1)); }
step()  { echo "${c_blue}==>${c_reset} $1"; }

cleanup() {
    # Drop the sandbox DB no matter how we exit.
    dropdb --if-exists "$(echo "$TEST_DATABASE_URL" | sed -E 's|.*/([^?]+).*|\1|')" 2>/dev/null || true
    dropdb --if-exists tasmail_backup_verify 2>/dev/null || true
    rm -rf "$WORK_ROOT"
}
trap cleanup EXIT

require() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required tool: $1" >&2
        exit 2
    fi
}
require psql
require createdb
require dropdb
require rsync
require tar
require gzip

DB_NAME="$(echo "$TEST_DATABASE_URL" | sed -E 's|.*/([^?]+).*|\1|')"

# Added: createdb/dropdb use libpq env vars, not the URL — parse and export so
# the test runs against the same Postgres the URL points to.
PG_USER="$(echo "$TEST_DATABASE_URL" | sed -E 's|.*://([^:/@]+).*|\1|')"
PG_PASS="$(echo "$TEST_DATABASE_URL" | sed -E 's|.*://[^:]+:([^@]+)@.*|\1|')"
PG_HOST="$(echo "$TEST_DATABASE_URL" | sed -E 's|.*@([^:/]+).*|\1|')"
PG_PORT="$(echo "$TEST_DATABASE_URL" | sed -nE 's|.*@[^:/]+:([0-9]+)/.*|\1|p')"
export PGUSER="$PG_USER" PGPASSWORD="$PG_PASS" PGHOST="$PG_HOST"
[[ -n "$PG_PORT" ]] && export PGPORT="$PG_PORT"

# ---- Build sandbox PostgreSQL DB ------------------------------------------
step "Provisioning sandbox database '${DB_NAME}'"
dropdb --if-exists "$DB_NAME" >/dev/null 2>&1 || true
createdb "$DB_NAME"
psql "$TEST_DATABASE_URL" >/dev/null <<'SQL'
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
    success BOOLEAN NOT NULL,
    checksum BYTEA NOT NULL,
    execution_time BIGINT NOT NULL
);
INSERT INTO _sqlx_migrations VALUES
    (1, 'initial schema', now(), TRUE, '\x00'::bytea, 0),
    (2, 'add users',      now(), TRUE, '\x00'::bytea, 0);

CREATE TABLE IF NOT EXISTS test_users (id SERIAL PRIMARY KEY, name TEXT);
INSERT INTO test_users (name) VALUES ('alice'), ('bob');
SQL

# ---- Build fake source tree ------------------------------------------------
step "Building synthetic /var/mail and /var/lib/tasmail/attachments"
MAIL_SRC="${WORK_ROOT}/var/mail"
ATTACH_SRC="${WORK_ROOT}/var/lib/tasmail/attachments"
CONFIG_SRC="${WORK_ROOT}/etc/tasmail"
BACKUP_BASE="${WORK_ROOT}/var/backups/tasmail"
LOG_FILE="${WORK_ROOT}/var/log/tasmail/backup.log"
mkdir -p "$MAIL_SRC/alice@example.com/cur" "$ATTACH_SRC/2026/04" "$CONFIG_SRC"
echo "MAIL-A-original" > "$MAIL_SRC/alice@example.com/cur/m1.eml"
echo "MAIL-B-original" > "$MAIL_SRC/alice@example.com/cur/m2.eml"
dd if=/dev/urandom of="$ATTACH_SRC/2026/04/file1.bin" bs=1k count=8 status=none
echo "TASMAIL_HOST=127.0.0.1" > "$CONFIG_SRC/tasmail.env"

# ---- Build env file the backup script sources -----------------------------
ENV_FILE="${WORK_ROOT}/tasmail.env"
cat > "$ENV_FILE" <<EOF
DATABASE_URL=${TEST_DATABASE_URL}
EOF

# ---- Off-site test dest is just a local directory we treat as 'remote' ----
OFFSITE_DEST="${WORK_ROOT}/offsite"
mkdir -p "$OFFSITE_DEST"

run_backup() {
    BACKUP_BASE="$BACKUP_BASE" \
    BACKUP_LOG_FILE="$LOG_FILE" \
    BACKUP_ENV_FILE="$ENV_FILE" \
    MAILDIR_SRC="$MAIL_SRC" \
    ATTACHMENT_DIR="$ATTACH_SRC" \
    CONFIG_SRC="$CONFIG_SRC" \
    BACKUP_OFFSITE_TYPE="$1" \
    BACKUP_OFFSITE_DEST="$OFFSITE_DEST" \
    BACKUP_RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-30}" \
    bash "$BACKUP_SH"
}

# ---- TEST 1: First-run backup --------------------------------------------
step "Test 1 — first-run backup with off-site rsync to local dir"
run_backup rsync

DB_GZ=$(find "$BACKUP_BASE/db" -maxdepth 1 -name "tasmail_db_*.sql.gz" | head -1)
[[ -f "$DB_GZ" ]] && ok "pg_dump artefact produced" || bad "pg_dump artefact missing"

CFG_GZ=$(find "$BACKUP_BASE/config" -maxdepth 1 -name "tasmail_config_*.tar.gz" | head -1)
[[ -f "$CFG_GZ" ]] && ok "config artefact produced" || bad "config artefact missing"

SNAPSHOT_TODAY=$(find "$BACKUP_BASE/maildir" -mindepth 1 -maxdepth 1 -type d ! -name current | head -1)
if [[ -n "$SNAPSHOT_TODAY" && -f "$SNAPSHOT_TODAY/alice@example.com/cur/m1.eml" ]]; then
    ok "maildir snapshot contains expected file"
else
    bad "maildir snapshot missing expected file"
fi

ATTACH_TODAY=$(find "$BACKUP_BASE/attachments" -mindepth 1 -maxdepth 1 -type d ! -name current | head -1)
if [[ -n "$ATTACH_TODAY" && -f "$ATTACH_TODAY/2026/04/file1.bin" ]]; then
    ok "attachments snapshot contains expected file"
else
    bad "attachments snapshot missing expected file"
fi

COMBINED=$(find "$BACKUP_BASE/combined" -maxdepth 1 -name "tasmail_backup_*.tar.gz" | head -1)
if [[ -f "$COMBINED" ]]; then
    ok "combined archive produced"
    if tar -tzf "$COMBINED" | grep -q '^./MANIFEST$'; then
        ok "combined archive contains MANIFEST"
    else
        bad "combined archive missing MANIFEST"
    fi
else
    bad "combined archive missing"
fi

# Off-site push (rsync mode) should have copied the archive to OFFSITE_DEST.
if [[ -f "$OFFSITE_DEST/$(basename "$COMBINED")" ]]; then
    ok "off-site rsync delivered archive"
else
    bad "off-site rsync did not deliver archive"
fi

# ---- TEST 2: Incremental rsync hardlinks ----------------------------------
step "Test 2 — second run produces hardlinked snapshot for unchanged files"

# Force a different snapshot date by overriding the per-day directory name.
# Easiest way: bump file mtimes to "yesterday" so the same-day rerun is a no-op
# and we can then run a third backup with a tweaked source.

# Simulate a new day: rename today's snapshot dir to "yesterday".
TODAY_TAG=$(basename "$SNAPSHOT_TODAY")
YESTERDAY_TAG=$(date -d "yesterday" '+%Y-%m-%d' 2>/dev/null || date -v-1d '+%Y-%m-%d')
mv "$BACKUP_BASE/maildir/$TODAY_TAG"     "$BACKUP_BASE/maildir/$YESTERDAY_TAG"
mv "$BACKUP_BASE/attachments/$TODAY_TAG" "$BACKUP_BASE/attachments/$YESTERDAY_TAG"

# Modify ONE file before the second run so we can prove m2 hardlinks across
# snapshots while m1 changes.
echo "MAIL-A-changed" > "$MAIL_SRC/alice@example.com/cur/m1.eml"

run_backup none

# Today's snapshot (whatever DATE_TAG resolved to inside backup.sh) is any
# dated dir that is NOT the yesterday-tagged one we just renamed.
NEW_SNAPSHOT=$(find "$BACKUP_BASE/maildir" -mindepth 1 -maxdepth 1 -type d \
    ! -name current ! -name "$YESTERDAY_TAG" | head -1)
if [[ -n "$NEW_SNAPSHOT" && -d "$NEW_SNAPSHOT" ]]; then
    ok "second-run created a new dated snapshot ($(basename "$NEW_SNAPSHOT"))"
    OLD_M2="$BACKUP_BASE/maildir/$YESTERDAY_TAG/alice@example.com/cur/m2.eml"
    NEW_M2="$NEW_SNAPSHOT/alice@example.com/cur/m2.eml"
    OLD_INODE=$(stat -c '%i' "$OLD_M2")
    NEW_INODE=$(stat -c '%i' "$NEW_M2")
    if [[ "$OLD_INODE" == "$NEW_INODE" ]]; then
        ok "unchanged file (m2.eml) is hardlinked across snapshots (inode=${NEW_INODE})"
    else
        bad "unchanged file did NOT hardlink (old=${OLD_INODE} new=${NEW_INODE})"
    fi
    NEW_M1_CONTENT=$(cat "$NEW_SNAPSHOT/alice@example.com/cur/m1.eml")
    [[ "$NEW_M1_CONTENT" == "MAIL-A-changed" ]] \
        && ok "changed file (m1.eml) updated in new snapshot" \
        || bad "changed file content wrong: '$NEW_M1_CONTENT'"
else
    bad "no new snapshot directory after second run"
fi

# ---- TEST 3: backup-verify.sh exits 0 -------------------------------------
step "Test 3 — backup-verify.sh against produced archive"
LATEST_COMBINED=$(find "$BACKUP_BASE/combined" -maxdepth 1 -name "tasmail_backup_*.tar.gz" -printf '%T@ %p\n' \
    | sort -nr | head -1 | cut -d' ' -f2-)
if BACKUP_BASE="$BACKUP_BASE" \
   BACKUP_VERIFY_LOG_FILE="${WORK_ROOT}/var/log/tasmail/backup-verify.log" \
   BACKUP_ENV_FILE="$ENV_FILE" \
   BACKUP_VERIFY_SANDBOX_DB="tasmail_backup_verify" \
   bash "$VERIFY_SH" "$LATEST_COMBINED" >/dev/null; then
    ok "backup-verify.sh PASS"
else
    bad "backup-verify.sh FAIL"
fi

# ---- TEST 4: corrupted archive must FAIL verify ---------------------------
step "Test 4 — corrupted archive must FAIL verify"
CORRUPT="${WORK_ROOT}/corrupt.tar.gz"
head -c 4096 "$LATEST_COMBINED" > "$CORRUPT"     # truncated tarball
if BACKUP_VERIFY_LOG_FILE="${WORK_ROOT}/var/log/tasmail/backup-verify-corrupt.log" \
   BACKUP_VERIFY_DB_RESTORE=0 \
   bash "$VERIFY_SH" "$CORRUPT" >/dev/null 2>&1; then
    bad "verify wrongly PASSED on corrupted archive"
else
    ok "verify correctly FAILED on corrupted archive"
fi

# ---- TEST 5: retention pruning -------------------------------------------
step "Test 5 — retention pruning removes stale combined archives"
STALE="${BACKUP_BASE}/combined/tasmail_backup_20200101_000000.tar.gz"
touch -d "60 days ago" "$STALE"
echo "dummy" > "$STALE"
touch -d "60 days ago" "$STALE"
BACKUP_RETENTION_DAYS=30 run_backup none
if [[ -e "$STALE" ]]; then
    bad "stale archive not pruned"
else
    ok "stale archive (>30 days) pruned"
fi

echo
echo "==========================================="
echo " Results: ${PASS} passed, ${FAIL} failed"
echo "==========================================="
[[ $FAIL -eq 0 ]] || exit 1
exit 0
