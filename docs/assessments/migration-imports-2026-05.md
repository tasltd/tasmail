# Migration Imports Assessment — May 2026

**Ticket:** TMAIL-256 (axis of TMAIL-241 backend modularisation review)
**Scope:** `/api/migration/*` flows — IMAP migration, MBOX import, PST import
(TMAIL-115), the shared progress/resume infrastructure, and the
`MigrationManager` + `PstImportManager` frontend surface.
**Method:** static read of every handler, model, service, migration, and
frontend component that touches the three import flows; plus `rg` sweeps
for background workers, trait abstractions, and APPEND paths. No runtime
testing was performed because no worker exists to test.

---

## TL;DR

The three import formats are wired up as separate, parallel branches
with **no shared abstraction**, **no background worker**, and **almost no
production-grade infrastructure**. The HTTP surface (handlers + DB schema +
frontend forms) is reasonable for v0 demos, but as soon as a user actually
clicks "Start Migration", the work disappears — the row is inserted into
`migration_jobs` (or `pst_imports`) with status `pending` and nothing ever
runs it. Two of the three formats (MBOX, IMAP) have zero processing code in
the repo. The third (PST) has a well-shaped `pst_processor` service but
it is never instantiated or called from `main.rs`.

Critical gaps, in priority order:

1. **No worker is started.** `main.rs` spawns `email_scheduler` and
   `queue_processor` but nothing for `migration_jobs` or `pst_imports`.
   Every import sits in `pending` forever.
2. **`MailImporter` trait does not exist.** Three parallel handler
   modules, two separate DB tables with duplicated `*_done/*_total/error_message`
   columns, no shared progress / cancellation / resume contract.
3. **PST upload reads the full file into memory** before writing to disk
   (`field.bytes().await` on a `Multipart`). A real Outlook archive is
   commonly 5–20 GB; this will OOM the backend on any realistic input.
4. **No resume / checkpoint on interruption.** The schema has
   `folders_done / messages_done`, but there is no code that reads them
   to skip already-processed work on restart.
5. **MBOX endpoint accepts a server-side file path from the client.**
   Even if MBOX processing existed, `mbox_file_path` is an
   arbitrary-string field — local file inclusion risk, and unusable for
   the BYOK end-user (who has no shell access to the box).
6. **IMAP source password is not encrypted** despite the column name
   being `source_password_encrypted`. The model binds the raw plaintext
   directly; `models/migration_job.rs:60` even has a `NOTE: Should be
   encrypted in production` comment. `services/encryption.rs` already
   exists for exactly this purpose (used by `payment_provider_config`).
7. **No rate limiting per source server.** Even if IMAP migration was
   implemented, nothing throttles per-source-host APPEND/FETCH calls;
   running 50 concurrent Gmail migrations from one tenant would trip
   Gmail's rate limits and likely the source account's anti-abuse.
8. **No failed-message bookkeeping registry.** A single
   `error_message TEXT` column for the whole job — no per-message error
   table, no resumable "skip these UIDs and retry the rest" semantics.
9. **`pst_imports.status` is still a Postgres ENUM** while CLAUDE.md
   mandates `TEXT + CHECK` (see migrations 063 / 065 for the pattern).
   `bulk_user_imports.status` was already converted (063); this one was
   missed.

---

## What was checked

| Axis | Question | Result |
|---|---|---|
| Abstraction | Three formats behind a `MailImporter` trait? | ❌ Three parallel branches |
| Worker | Does anything actually run pending imports? | ❌ No worker spawned |
| Resumability | Checkpoint on interruption? | ❌ Schema has columns, no code |
| Memory | Streamed parsing vs load-entire-file? | ❌ PST: full file into RAM |
| Rate limiting | Per-source-server throttle for IMAP? | ❌ No IMAP migration code at all |
| Error bookkeeping | Per-message error registry? | ❌ One `error_message` per job |
| Credentials | Source password encrypted at rest? | ❌ Plaintext in `*_encrypted` column |
| Schema consistency | `TEXT + CHECK` vs Postgres ENUM | ❌ `pst_imports` still ENUM |
| Frontend file size | Client-side validation / progress? | ❌ No size check, no upload progress |
| Frontend folder picker | Real folder list? | ❌ Hardcoded INBOX/Archive/Imported |

---

## File map

### Backend

| File | LoC | Role |
|---|---:|---|
| `backend/src/handlers/migration.rs` | 179 | IMAP + MBOX start/list/get/cancel routes |
| `backend/src/handlers/pst_import.rs` | 279 | PST upload/list/get/delete routes |
| `backend/src/handlers/bulk_import.rs` | 435 | **User** bulk-provisioning CSV — unrelated to message imports despite the name; only listed for clarity |
| `backend/src/models/migration_job.rs` | 199 | `migration_jobs` row + create/list/update/cancel |
| `backend/src/models/pst_import.rs` | 239 | `pst_imports` row + create/list/mark/delete |
| `backend/src/services/pst_processor.rs` | 258 | Shells out to `readpst`, collects `.eml` files. **Never called from `main.rs`.** |
| `backend/migrations/011_migration_jobs.sql` | 40 | IMAP + MBOX schema (TEXT + CHECK) |
| `backend/migrations/027_pst_imports.sql` | 25 | PST schema (Postgres ENUM — out of date) |
| `backend/src/router.rs:267-278` | — | Six `/api/migration/*` routes |
| `backend/src/main.rs:70-105` | — | Email scheduler + queue processor + billing rollup spawned; **no migration worker** |

### Frontend

| File | LoC | Role |
|---|---:|---|
| `frontend/src/components/settings/MigrationManager.tsx` | 181 | IMAP / MBOX tabbed form, polls `/api/migration` every 5 s, embeds `<PstImportManager />` |
| `frontend/src/components/settings/PstImportManager.tsx` | 292 | Drag-and-drop `.pst` upload, polls `/api/migration/pst` every 5 s |
| `frontend/src/components/settings/BulkImportManager.tsx` | 274 | **Admin user CSV import**, unrelated — same as backend `bulk_import.rs` |

---

## Detailed findings

### 1. No shared `MailImporter` abstraction

The three formats are wired as independent handlers with their own DB
tables. The columns overlap heavily but are not factored:

```text
migration_jobs:    folders_total, folders_done, messages_total, messages_done, bytes_transferred, error_message
pst_imports:                                    messages_found,  messages_imported,                error_message
```

A `MailImporter` trait shaped like

```rust
#[async_trait]
trait MailImporter {
    fn id(&self) -> Uuid;
    async fn validate(&self) -> Result<(), AppError>;
    async fn estimate(&self) -> Result<ImportEstimate, AppError>;   // folders/messages/bytes
    async fn iter_messages(&self) -> impl Stream<Item = Result<ImportedMessage, ImportError>>;
    async fn checkpoint(&self, progress: ImportProgress) -> Result<(), AppError>;
    async fn resume_from(&self, last: ImportProgress) -> Result<(), AppError>;
}
```

with one impl per format (`ImapImporter`, `MboxImporter`, `PstImporter`)
would let a single worker drive all three. Today the worker is missing
in both directions — there is no trait *and* no engine to consume it.

**Why this matters at scale:** adding a fourth format (Maildir, EML zip,
Apple Mail mbox, takeout-style multi-mbox archives) currently requires
duplicating the entire pipeline. With the trait it's one file.

---

### 2. No background worker spawned

`backend/src/main.rs:70-105` starts three background tasks:

```rust
services::email_scheduler::EmailScheduler::new(...).start();      // outbound emails
services::queue_processor::QueueProcessor::new(...).start();      // BYOK send queue
services::billing_rollup::BillingRollup::new(...).start();        // usage billing
```

There is **no** migration or PST worker. The handler comment in
`handlers/migration.rs:37-39` is aspirational, not implemented:

> NOTE: The actual migration execution is handled by a background worker
> that polls for pending jobs. In production, this would invoke imapsync.
> For now, just create the job record.

Effect: every `POST /api/migration/imap`, `/migration/mbox`, and
`/migration/pst/upload` returns 201 Created with `status: "pending"` and
the row stays `pending` until end-of-universe. The 5 s polling on the
frontend will faithfully show "pending" forever. This is the single
biggest gap.

The fix shape is straightforward and mirrors `queue_processor`:

```rust
let migration_worker = services::migration_worker::MigrationWorker::new(
    std::sync::Arc::new(pool.clone()),
    state.encryption.clone(),
    5,                          // poll seconds
)
.with_concurrency_per_format(/* imap=4, mbox=2, pst=2 */)
.with_per_source_rate_limit(/* gmail=10 req/s, etc */);
migration_worker.start();
```

---

### 3. PST upload loads the entire file into RAM

`backend/src/handlers/pst_import.rs:51-62`:

```rust
let data: axum::body::Bytes = field
    .bytes()
    .await
    .map_err(...)?;
...
file_data = Some(data.to_vec());
```

`field.bytes()` buffers the whole multipart field into a `Bytes`. Then
the handler `to_vec()`s it (a second copy). For a 10 GB PST the backend
needs ~20 GB of RAM during a single upload. With multiple users this is
trivially DoS-able.

**Fix:** use `field.chunk()` in a loop and write each chunk to disk as
it arrives, e.g.

```rust
let mut out = tokio::fs::File::create(&file_path).await?;
while let Some(chunk) = field.chunk().await? {
    out.write_all(&chunk).await?;
}
```

Add a server-side max-size guard (drop the connection once written bytes
exceed `config.migration.pst_max_bytes`) — the multipart layer of axum
has no built-in cap once you start streaming, so the guard has to be
explicit.

**Frontend should also:**
- Show upload progress (XHR `progress` event — current code uses
  `pstImportApi.upload` which does a one-shot `fetch`, no progress UI).
- Reject oversized files client-side before posting.
- Use `tus-js-client` or chunked uploads for >2 GB so a dropped
  connection doesn't restart from 0.

---

### 4. MBOX has no parsing code at all

`handlers/migration.rs::start_mbox_import` accepts a JSON body of the
form `{"mbox_file_path": "/tmp/foo.mbox"}`, inserts a `migration_jobs`
row, and returns. There is:

- no `mbox_processor` service
- no `mbox` crate in `Cargo.toml`
- no streaming parser
- no message-by-message APPEND to the destination IMAP folder

Two ways forward:

- **Library:** the `mbox-reader` crate (or `mailparse` + a `From `-line
  splitter) gives you a `Stream<Item = RawMessage>`. Memory stays flat
  even on 50 GB Google Takeout exports.
- **External:** shell out to `formail` or `mb2md` like the PST processor
  shells out to `readpst`. Adds a system dependency but matches the
  existing pattern.

Either way the input must be a **server-stored** file uploaded through
the same multipart endpoint pattern as PST — *not* a free-text path
chosen by the client.

---

### 5. MBOX path injection risk

`MigrationManager.tsx:111-124` literally renders a `<input type="text">`
for the user to type a server-side path:

```jsx
<input type="text" value={mboxPath}
  onChange={(e) => setMboxPath(e.target.value)}
  placeholder="/path/to/takeout.mbox" required />
```

The backend then stores that string and (if processing ever existed)
would read whatever file it points at. An end-user has no way to
populate this field with a real path — they don't have shell access. A
hostile authenticated user could try `/etc/passwd`, the Dovecot mail
spool of another tenant, the JWT signing key file, etc.

**Fix:** delete the path input. Replace with a file-upload widget that
reuses the same multipart pipeline as PST. The backend reads from
`/var/lib/tasmail/imports/{user_id}/{import_id}.mbox` (or wherever the
configured upload dir points), and `mbox_file_path` becomes a
server-controlled internal detail.

---

### 6. IMAP password "encryption" is a noop

`models/migration_job.rs:46-64`:

```rust
sqlx::query_as::<_, MigrationJob>(
    "INSERT INTO migration_jobs (mailbox_id, job_type, source_host, source_port,
                                 source_user, source_password_encrypted, source_use_ssl)
     VALUES ($1, 'imap', $2, $3, $4, $5, $6) ..."
)
.bind(&req.source_password) // NOTE: Should be encrypted in production
```

The column is named `*_encrypted`, but the value is the raw plaintext.
The repo *already has* `services::encryption::EncryptionService` derived
from the JWT secret (used to encrypt
`payment_provider_config.api_key_encrypted`). Wiring is one extra line
plus passing `state.encryption.clone()` into the model call.

`models/imap_config.rs::ImapConfiguration` (the BYOK config) already
does this correctly — copy that pattern.

---

### 7. No per-source-server rate limiting

If `MigrationWorker` ever lands, it has to throttle outbound IMAP calls
against the *source* server, not just total concurrency. Gmail's
documented IMAP limits are ~2,500 connections/minute and 15 simultaneous
per account. Yahoo, Outlook, Zoho all publish similar limits.

Design shape:

```rust
struct PerSourceLimiter {
    inner: dashmap::DashMap<String, governor::RateLimiter<...>>,  // key = source_host
}
```

Plus a per-source-account ceiling (max 4 concurrent IMAP sessions per
`source_user@source_host`) so a single source account can't burn its own
rate budget across multiple migration jobs.

---

### 8. No failed-message bookkeeping

The schema has `error_message TEXT` — that's the *one* error string for
the whole job. A 20,000-message Gmail migration that fails on 47
specific messages (large attachments, malformed headers, oversized
folders) has no way to report *which 47*, no way to retry just them, and
no way to resume past them on the next attempt.

Recommended addition:

```sql
CREATE TABLE migration_message_errors (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id       UUID NOT NULL REFERENCES migration_jobs(id) ON DELETE CASCADE,
    source_uid   TEXT,                       -- source IMAP UID or mbox offset
    source_folder TEXT,
    error_class  TEXT NOT NULL CHECK (error_class IN
        ('size_limit','parse_error','append_failed','source_unavailable','target_quota','unknown')),
    error_detail TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_migration_message_errors_job ON migration_message_errors(job_id);
```

Same shape works for `pst_imports`. Frontend can then surface "47
messages skipped" with a drill-down list and a "retry skipped" button.

The classes themselves should be a registry-driven `enum ErrorClass`
that the worker writes to and the frontend reads from — exactly the
"data-driven configuration over hardcoded logic" pattern in
`~/.claude/rules/all-rules.md`.

---

### 9. Schema drift: `pst_imports.status` still on Postgres ENUM

`migrations/027_pst_imports.sql`:

```sql
CREATE TYPE pst_import_status AS ENUM ('pending', 'processing', 'completed', 'failed');
```

CLAUDE.md says:

> When adding a status/type column, prefer `TEXT + CHECK` over a Postgres
> ENUM — see migrations 061/063/065 for the pattern.

Migration 063 already did this for `bulk_user_imports.status`. A
follow-up migration `073_pst_imports_status_to_text.sql` should drop the
ENUM and convert the column to `TEXT + CHECK (status IN
('pending','processing','completed','failed'))`. The model field is
already `String`, so no Rust code changes — purely a schema cleanup that
lets sqlx decode it without the ENUM dance and matches the rest of the
codebase.

---

### 10. `/tmp` hardcoded for PST uploads

`handlers/pst_import.rs:100` and `services/pst_processor.rs` both
hardcode `/tmp/pst_uploads`. On Linux, `/tmp` is

- often `tmpfs` (RAM-backed) → uploading a 10 GB PST OOMs the host
- wiped on reboot → a `processing` import is dead after any restart
- no per-user quotas → one user can fill the partition for everyone
- not on the BYOK encrypted volume

Should come from config:

```toml
[migration]
upload_dir = "/var/lib/tasmail/imports"
pst_max_bytes = 21_474_836_480   # 20 GB
mbox_max_bytes = 53_687_091_200  # 50 GB
imap_concurrent_per_user = 2
```

---

### 11. Frontend polls every 5 s

`MigrationManager.tsx:21` and `PstImportManager.tsx:25` both use
`refetchInterval: 5000`. For 1,000 concurrent migrations that's 200
req/s just to display progress bars. The repo already has a WebSocket
endpoint (`/ws`) — `useWebSocket` could push per-job progress events and
let the polling drop to a single refresh on disconnect/reconnect.

Lower priority — only matters at real scale. Polling is fine for v0.

---

### 12. Hardcoded target-folder dropdown

`PstImportManager.tsx:151-155`:

```jsx
<option value="INBOX">INBOX</option>
<option value="Archive">Archive</option>
<option value="Imported">Imported</option>
```

The user's actual folder tree is already fetched by
`useMailbox().folders`. Replace the three hardcoded options with a real
folder picker (the same one the move/copy dialogs use). Also add a "New
folder…" option that creates the destination folder via IMAP CREATE
before the import starts — common ask for "import everything into
'Gmail Archive 2024'".

---

## Recommended ordering

Anything before `[A]` is a precondition for the rest. The fixes are
independent within each numbered group.

**[A] Make imports actually run (blocker for everything below)**

- `[A1]` Create `services::migration_worker::MigrationWorker` that polls
  `migration_jobs` and `pst_imports` for `status='pending'`, transitions
  to `running`/`processing`, and dispatches to a format-specific
  importer.
- `[A2]` Spawn it from `main.rs` alongside `queue_processor`.
- `[A3]` Wire `pst_processor::extract_emails` from the worker for PST
  (single-stage: extract → APPEND → cleanup → mark completed).

**[B] Trait extraction (after [A])**

- `[B1]` Introduce `MailImporter` trait, refactor `pst_processor` to
  implement it.
- `[B2]` Implement `ImapImporter` (async-imap source session → APPEND
  to local Dovecot).
- `[B3]` Implement `MboxImporter` (mbox-reader crate, streamed).

**[C] Hardening (parallel to [B])**

- `[C1]` Stream PST upload to disk (chunked write).
- `[C2]` Replace MBOX path input with file upload widget.
- `[C3]` Encrypt `source_password` via `EncryptionService`.
- `[C4]` Add `migration_message_errors` table + registry-driven
  `ErrorClass`.
- `[C5]` Migrate `pst_imports.status` to `TEXT + CHECK`.
- `[C6]` Config-driven upload dirs and size caps.

**[D] Scale + UX polish (last)**

- `[D1]` Per-source-host + per-source-account IMAP rate limiter.
- `[D2]` Replace 5 s polling with `/ws` push.
- `[D3]` Replace hardcoded folder dropdown with real folder picker +
  "create new" option.
- `[D4]` Chunked / resumable upload for >2 GB (tus, or roll-your-own
  range-based upload).

`[A]` and `[C5]` are the only items small enough to land in this
assessment's follow-up commits without dragging in a worker design. The
rest are scoped issues to file against TMAIL-241 / TMAIL-115.

---

## Sources read for this assessment

- `backend/src/handlers/migration.rs` (179 lines)
- `backend/src/handlers/pst_import.rs` (279 lines)
- `backend/src/models/migration_job.rs` (199 lines)
- `backend/src/models/pst_import.rs` (239 lines)
- `backend/src/services/pst_processor.rs` (258 lines)
- `backend/src/main.rs` (143 lines — confirmed no migration worker)
- `backend/src/router.rs:267-278` (route wiring)
- `backend/migrations/011_migration_jobs.sql`
- `backend/migrations/027_pst_imports.sql`
- `backend/migrations/029_bulk_imports.sql` (for the CSV/ENUM pattern)
- `frontend/src/components/settings/MigrationManager.tsx` (181 lines)
- `frontend/src/components/settings/PstImportManager.tsx` (292 lines)

No changes were made to runtime code as part of this assessment. The
follow-ups will land as separate scoped commits under TMAIL-241's
modularisation series and against the open `[A]`/`[C5]` items above.
