# TASMail Backup & Restore (TMAIL-42)

This guide covers TASMail's daily backup pipeline, weekly verification, and
restore procedure. The pipeline is built on standard Unix tooling — `pg_dump`,
`rsync`, `tar`, `gzip`, `systemd` timers — so it has no runtime dependency on
the TASMail binaries and can be operated even when the app is down.

## Overview

| Concern | Mechanism |
|---|---|
| PostgreSQL data | Full `pg_dump` once per day, gzipped, retained 30 days |
| Maildir (`/var/mail`) | Incremental `rsync --link-dest` snapshots, retained 30 days |
| Attachments (`/var/lib/tasmail/attachments`) | Same rsync snapshot pattern |
| Config (`/etc/tasmail`) | Small tarball each run |
| Off-site transport | Optional `rclone` or `rsync` push of the daily combined archive |
| Verification | Weekly script that exercises tar + gzip + sandbox `psql` restore |
| Schedule | `tasmail-backup.timer` (daily 02:00) + `tasmail-backup-verify.timer` (weekly Sun 03:00) |

## Why incremental Maildir

A working TASMail deployment will accumulate gigabytes of Maildir mail. A full
nightly `tar.gz` is expensive in I/O and storage and grows unboundedly. Instead
`backup.sh` uses `rsync --link-dest=<previous>` so:

- Each daily snapshot directory looks like a complete point-in-time view of
  the Maildir.
- Files unchanged since the previous snapshot are hardlinks — only one copy
  on disk, but visible in every snapshot they apply to.
- The transferred bytes per run are the daily delta, not the full Maildir.
- Pruning a snapshot does not delete any file still referenced by another
  snapshot — disk reclamation is automatic via the link count.

The same pattern is used for the attachment store.

## On-disk layout

```
/var/backups/tasmail/
├── db/
│   └── tasmail_db_<TIMESTAMP>.sql.gz
├── config/
│   └── tasmail_config_<TIMESTAMP>.tar.gz
├── maildir/
│   ├── current/                    # latest mirror; rsync link-dest base
│   └── YYYY-MM-DD/                 # dated snapshot (hardlink-deduped)
├── attachments/
│   ├── current/
│   └── YYYY-MM-DD/
└── combined/
    └── tasmail_backup_<TIMESTAMP>.tar.gz   # transportable bundle for off-site
```

The **combined archive** is the artefact you push off-site and the one the
verify script exercises. It contains the day's pg_dump, the config tarball,
gzipped tar of the day's Maildir + attachment snapshot, and a `MANIFEST` file
listing what's inside.

## Setup

### Install systemd units

```bash
sudo install -o root -g root -m 0644 \
  deploy/systemd/tasmail-backup.service \
  deploy/systemd/tasmail-backup.timer \
  deploy/systemd/tasmail-backup-verify.service \
  deploy/systemd/tasmail-backup-verify.timer \
  /etc/systemd/system/

sudo systemctl daemon-reload
sudo systemctl enable --now tasmail-backup.timer
sudo systemctl enable --now tasmail-backup-verify.timer
```

Confirm both timers:

```bash
systemctl list-timers tasmail-*
```

### Configure off-site storage (optional but recommended)

By default no off-site copy is made (`BACKUP_OFFSITE_TYPE=none`). Set the
variables below in `/etc/tasmail/tasmail.env` to enable. Both modes push the
single combined tarball — restore-ready, single file per day.

#### Option A — rclone (S3, B2, Wasabi, Backblaze, etc.)

```bash
BACKUP_OFFSITE_TYPE=rclone
BACKUP_OFFSITE_DEST=tasmail-offsite:tasmail-backups
# Optional extra flags (server-side encryption, bandwidth limit, etc.)
BACKUP_OFFSITE_OPTS=--s3-server-side-encryption=AES256
```

Pre-configure your rclone remote once: `rclone config` and verify with
`rclone lsd tasmail-offsite:`.

#### Option B — rsync over SSH

```bash
BACKUP_OFFSITE_TYPE=rsync
BACKUP_OFFSITE_DEST=backup@offsite.example.com:/srv/tasmail-backups/
BACKUP_OFFSITE_OPTS=--bwlimit=10000
```

The backup runs as root so it uses root's SSH key (`/root/.ssh/`); make sure
that key is authorized on the off-site host.

## Manual operations

```bash
# Run an ad-hoc backup
sudo /opt/tasmail/deploy/scripts/backup.sh

# Verify the most recent combined archive
sudo /opt/tasmail/deploy/scripts/backup-verify.sh

# Verify a specific archive
sudo /opt/tasmail/deploy/scripts/backup-verify.sh /var/backups/tasmail/combined/tasmail_backup_20260415_020000.tar.gz

# Restore from a combined archive (DESTRUCTIVE)
sudo /opt/tasmail/deploy/scripts/restore.sh /var/backups/tasmail/combined/tasmail_backup_20260415_020000.tar.gz
```

## Verification

`backup-verify.sh` runs four checks:

1. `tar -tzf` — proves the outer archive isn't truncated or corrupted.
2. `gunzip -t` — proves the embedded `pg_dump` gzip stream is intact.
3. Header inspection — confirms the dump begins with the PostgreSQL marker.
4. Sandbox restore — `createdb tasmail_backup_verify` then `psql < dump`,
   asserts `_sqlx_migrations` has rows. Can be skipped with
   `BACKUP_VERIFY_DB_RESTORE=0` (e.g. on small VMs).

Exit code is `0` only when every applicable check passes. Failures land in
`/var/log/tasmail/backup-verify.log` and in the systemd journal via
`journalctl -u tasmail-backup-verify`.

## Retention

`BACKUP_RETENTION_DAYS` (default `30`) controls pruning. Each daily run removes:

- `db/tasmail_db_*.sql.gz` older than the cutoff
- `config/tasmail_config_*.tar.gz` older than the cutoff
- `combined/tasmail_backup_*.tar.gz` older than the cutoff
- Dated `maildir/YYYY-MM-DD/` and `attachments/YYYY-MM-DD/` snapshot dirs
  older than the cutoff (the `current/` pointer is preserved)

## Disaster recovery procedure

1. Provision a fresh host with the same PostgreSQL major version.
2. Install TASMail (`deploy/scripts/setup-all.sh`) but do not start it yet.
3. Copy the chosen combined archive from off-site to the new host.
4. `sudo systemctl stop tasmail-backend` (if running).
5. `sudo deploy/scripts/restore.sh /path/to/tasmail_backup_*.tar.gz`.
6. Restore script: drops/recreates DB, loads dump, rsyncs Maildir into
   `/var/mail/`, rsyncs attachments, replaces `/etc/tasmail`, restarts the
   backend, and verifies `systemctl is-active tasmail-backend`.
7. Run `deploy/scripts/backup-verify.sh` against the same archive on the
   new host to confirm it loaded cleanly.

## Testing

`deploy/scripts/test-backup.sh` is an end-to-end shell test against a real
local PostgreSQL. It builds a synthetic Maildir + attachment tree, runs the
backup twice (validating that rsync hardlinks unchanged files), then runs
the verify script against the produced archive and against a corrupt
archive to prove failure-mode detection works:

```bash
TEST_DATABASE_URL='postgres://user:pass@localhost/tasmail_backup_test' \
  deploy/scripts/test-backup.sh
```

The test cleans up after itself; the sandbox database is dropped on exit.
