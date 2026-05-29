# Attachments, Shared Files & Quota Assessment

- **Issue:** TMAIL-246 (axis of TMAIL-241)
- **Date:** 2026-05-29
- **Scope (backend):** `backend/src/handlers/attachments.rs` (539 LOC),
  `backend/src/handlers/shared_files.rs` (431 LOC),
  `backend/src/handlers/quota.rs` (95 LOC),
  `backend/src/services/attachment_service.rs` (453 LOC, owns storage + ClamAV),
  `backend/src/models/attachment.rs`, `backend/src/models/shared_file.rs`,
  `backend/src/models/quota.rs`, `backend/src/models/mailbox.rs:quota_bytes`,
  `backend/src/services/cache_service.rs:139–155` (quota cache layer),
  `backend/src/services/phishing_scanner.rs:82–91` (DANGEROUS_EXTENSIONS registry —
  currently only consumed by inbound-mail scanning),
  `backend/migrations/004_quota_usage.sql`,
  `backend/migrations/019_attachment_storage.sql`,
  `backend/migrations/028_shared_files.sql`,
  `backend/migrations/068_phishing_dangerous_attachments.sql`,
  the public route `/api/dl/{token}` (`router.rs:89`),
  `backend/src/config.rs:218–245` (StorageConfig).
- **Scope (frontend):** `frontend/src/api/attachments.ts`,
  `frontend/src/api/shared-files.ts`, `frontend/src/api/quota.ts`,
  `frontend/src/components/mail/LargeFileAttacher.tsx` + `large-file-attacher-utils.ts`
  (TMAIL-138 — recently landed), `frontend/src/components/settings/AttachmentManager.tsx`,
  `frontend/src/components/settings/SharedFileManager.tsx`,
  `frontend/src/components/layout/QuotaBar.tsx`,
  `frontend/src/utils/constants.ts:14–17` (LARGE_FILE_THRESHOLD_BYTES + MAX_SHARED_FILE_BYTES).
- **Method:** Static read of every file in scope. Grep sweep for actual call
  sites of `AttachmentService::scan_file`, `SharedFile::find_by_token`, and
  background-task wiring in `main.rs` to determine which paths run on the
  hot path vs. which are guarded by config. Migrations cross-checked against
  model queries to confirm index coverage. Config defaults in `config.rs`
  cross-checked against constants in `frontend/src/utils/constants.ts` to
  detect drift. No load run was captured — heap/RTT figures cited are
  conservative ballpark numbers derived from the buffered-`Bytes` upload
  pattern, not measured.

---

## TL;DR — biggest wins, by ROI

| # | Finding | Severity | Effort | Suggested ticket |
|---|---------|----------|--------|------------------|
| 1 | **ClamAV fail-open when socket is unconfigured.** `attachment_service.rs:71–78` — if `clamav_socket` is `None`, returns `("clean", "ClamAV not configured")`. The upload handler then persists that status verbatim via `Attachment::update_scan_status(... "clean", ...)`. The download handler blocks only `scan_status == "infected"` (`handlers/attachments.rs:204`). Net effect: a production deploy that forgets to set `CLAMAV_SOCKET` (or whose ClamAV unit isn't running) silently passes every upload as virus-clean. The dev fallback bled into the prod policy. The `Default` impl for `StorageConfig` (`config.rs:240–244`) explicitly seeds `clamav_socket: None`, so the unsafe default is the default. | **P0 — security policy** | Low — change the unconfigured branch to return `("pending", "ClamAV not configured")` (so unscanned uploads stay blocked), AND add a `download_attachment` gate `if scan_status != "clean" { return Forbidden }` keyed off a new `storage.require_clean_scan` config flag (default true in prod, false for tests). Surface a startup log warning if `clamav_socket` is None and `require_clean_scan` is true. | New (P0 — pair with TMAIL-59 follow-up) |
| 2 | **Shared-files SPA cap (500 MB) is 20× the backend cap (25 MB).** `frontend/src/utils/constants.ts:17` exports `MAX_SHARED_FILE_BYTES = 500 * 1024 * 1024`. `backend/src/config.rs:234` defaults `max_file_size = 25 * 1024 * 1024`. The same `max_file_size` gate is applied to BOTH the attachments handler (`attachments.rs:72`) AND the shared-files handler (`shared_files.rs:73`). So any file the `LargeFileAttacher` accepts between 25 MB and 500 MB **completes the multipart upload**, the progress bar reaches 100%, the server hands it back to the user as `400 BadRequest("File size N bytes exceeds maximum allowed 26214400 bytes (25 MB)")`. Worst-case UX: a user spends 30 minutes uploading a 400 MB video over a flaky link and gets a "too big" toast at the very end. The product positioning ("BYOK — TASMail handles overflow via large-file links") makes this even more painful: the user explicitly picked TASMail because their mail provider couldn't carry the bytes. | **P0 — UX + sizing** | Low — pick one number per product surface: `max_attachment_bytes = 25 MB` (mail attachments) and `max_shared_file_bytes = 500 MB` (cloud relay). Add both to `StorageConfig`, expose via a new `GET /api/config/limits` endpoint, have the SPA fetch on boot and stop hardcoding. Until that lands, at minimum patch the backend default to 500 MB for `max_file_size` AND add a separate `max_shared_file_bytes` field that the shared-files handler reads. | New (P0) |
| 3 | **No expiry sweeper for shared files.** `shared_file.is_expired()` (`models/shared_file.rs:124–138`) is checked **only** at download time (`handlers/shared_files.rs:254`). Once `expires_at < NOW()` OR `download_count >= max_downloads`, the database row and the on-disk file stay forever. `grep -rn 'shared_files' backend/src/services/` returns zero matches — no background task touches the table. For a product priced GHS 1.00 / GB · month, expired files become permanent unbilled dead weight; the disk fills until an operator notices the partition is full. The migration also doesn't index `expires_at` or `(max_downloads, download_count)`, so once the sweeper is added the query will seq-scan. | **P0 — data hygiene + cost** | Medium — add a `SharedFileSweeper` background task in `main.rs` next to `EmailScheduler` (poll every hour). For each row matching `expires_at < NOW() OR (max_downloads IS NOT NULL AND download_count >= max_downloads)`, `tokio::fs::remove_file(storage_path)` then `DELETE` the row. Use a Postgres advisory lock so multiple replicas don't race. Add `CREATE INDEX idx_shared_files_expires ON shared_files(expires_at) WHERE expires_at IS NOT NULL;` and `idx_shared_files_exhausted ON shared_files(max_downloads, download_count) WHERE max_downloads IS NOT NULL;` in the same migration. Also add a one-shot orphan reaper that flags `shared_files` rows whose `storage_path` no longer exists on disk (and vice-versa). | New (P0) |
| 4 | **`GET /api/dl/{token}` reads the entire shared file into memory before responding.** `handlers/shared_files.rs:279` does `let data = tokio::fs::read(&shared_file.storage_path).await?;`. The body builder then puts that `Vec<u8>` into `Body::from(data)`. Once finding 2 is resolved and the real cap is 500 MB, two concurrent public downloads pin 1 GB of heap. No Range support either (mirror this against the attachments handler at `attachments.rs:217–267` which does Range correctly with 12 unit tests). A single leaked token with no `max_downloads` becomes a one-line DoS: anyone can ask for the file repeatedly and each request allocates the whole file. | **P0 — perf + scalability** | Low — swap to streaming: `let file = tokio::fs::File::open(...).await?; let stream = ReaderStream::new(file); Body::from_stream(stream)`. Lift the `parse_byte_range` helper out of `handlers/attachments.rs:311` into a shared `services/range.rs` and reuse on this path. Drops peak memory to ~64 KiB per concurrent download. | New (P0; reuses attachments-side range parser) |
| 5 | **Upload pipeline buffers the entire multipart body in memory before disk write.** `handlers/attachments.rs:67` and `handlers/shared_files.rs:67` both do `let data: Bytes = field.bytes().await?` — axum buffers the whole field into a contiguous `Bytes` before either handler sees it. For the 25 MB attachment cap today this is tolerable (`max_file_size * concurrent_uploads`); for the 500 MB shared-file cap (post finding 2 resolution) this is 500 MB of heap per concurrent upload, with the size check happening AFTER the buffer is fully populated. The size-cap check at `:72` rejects after the bytes are already in the address space. | **P1 — perf** | Medium — switch to streaming write inside the field loop: `let mut file = tokio::fs::File::create(temp_path).await?; let mut hasher = Sha256::new(); let mut total = 0u64; while let Some(chunk) = field.chunk().await? { total += chunk.len() as u64; if total > max_size { /* abort + delete temp */ } hasher.update(&chunk); file.write_all(&chunk).await?; }`. Peak memory ~64 KiB regardless of file size, AND the size cap aborts early. Same checksum-first refactor folds in finding 6. | New (P1; pair with finding 6) |
| 6 | **Attachment upload writes file, THEN checksums, THEN dedups.** `attachments.rs:110–121`: `service.store_file(...)` writes the bytes to disk and returns `(storage_path, checksum)`; `Attachment::find_by_checksum(...)` then asks "did we already have this?"; if yes, `service.delete_file(storage_path)` reverses the write. Wasted disk I/O on every duplicate upload — typical for chains where the same PDF gets forwarded around a team. For a 25 MB PDF that's 25 MB written-then-deleted on every dupe. | **P1 — I/O + correctness** | Low — checksum FIRST (hash the in-memory buffer or the streaming chunks from finding 5), look up `find_by_checksum`, return existing row if hit, only write to disk if new. The dedup-then-write order also lets ClamAV results be reused across mailboxes IF the scan is moved up a layer (worth considering: per-checksum scan results instead of per-attachment). | New (P1; pair with finding 5) |
| 7 | **No MIME-type or extension validation on attachment / shared-file upload.** `handlers/attachments.rs:62–64` and `handlers/shared_files.rs:62–65` accept whatever `content_type` the client puts in the multipart header verbatim; no allowlist, no extension check. `DANGEROUS_EXTENSIONS` (`services/phishing_scanner.rs:82–86` — `exe / bat / cmd / com / scr / pif / msi / msp / hta / js / jse / vbs / vbe / wsf / wsh / ps1 / psm1 / jar / jnlp / lnk / reg / scf / inf / iso / img`) is consumed only by inbound-mail phishing scanning. A logged-in user can upload `payload.exe` to either endpoint, it bypasses every check, and (per finding 1) the ClamAV scan may not even run. Outlook Safe Attachments has the same registry and DOES gate uploads. | **P1 — security + Outlook parity** | Medium — lift `DANGEROUS_EXTENSIONS` (and the `DECEPTIVE_DOUBLE_EXTENSIONS` array beneath it) out of `phishing_scanner.rs` into `services/dangerous_attachments.rs::{DANGEROUS_EXTENSIONS, DECEPTIVE_DOUBLE_EXTENSIONS, is_dangerous(filename)}`. Have the phishing scanner consume from there, AND have the upload handlers reject (or mark `scan_status='dangerous_type'` and require admin override) when `is_dangerous` returns true. Make the policy configurable via `storage.block_dangerous_extensions = true` so an enterprise customer with an internal use case can opt out per tenant. Same fix lets shared-files apply the policy (today shared-files runs no scanning at all — see finding 17). | New (P1) |
| 8 | **Quota write paths don't invalidate the Redis cache.** `handlers/quota.rs:43` sets the cache on `get_quota`; `sync_quota` (line 91–92) invalidates + refreshes. But neither `handlers/attachments.rs::upload_attachment` (which adds to used storage) nor `delete_attachment` (which frees it) calls `state.cache.invalidate_quota(&claims.sub).await`. Neither does any IMAP write path (delete / move / mark-read have no quota touch). Net effect: the QuotaBar in the SPA shows stale data for up to 60 s after a meaningful storage change. For passive display this is fine; for hard-cap enforcement it doesn't matter because `upload_attachment` bypasses the cache anyway (`:96–101` does fresh `Mailbox::find_by_id` + `Attachment::total_size_for_mailbox`). It's a write-path consistency cleanup. | **P1 — UX freshness** | Low — `state.cache.invalidate_quota(&claims.sub).await` at the end of `upload_attachment` and `delete_attachment` (and any future shared-files write path). The SPA's TanStack-Query `['quota']` key should also be invalidated from the upload mutation `onSuccess` so the bar refreshes without waiting for the 5-min poll. | New (P1) |
| 9 | **Upload-time quota enforcement is partial — it sums attachments only, not IMAP usage.** `attachments.rs:96–108` reads `mailbox.quota_bytes` and compares it against `Attachment::total_size_for_mailbox(...)` (sum of `attachments.size_bytes`). It does NOT add the `quota_usage.used_bytes` figure (which is the IMAP-reported size from `sync_quota`). A user with a 1 GB quota who has 950 MB of IMAP messages and 60 MB of attachments would pass the attachment check (60 MB < 1 GB) but is actually at 1.01 GB true usage. Same gap for shared files — they're stored on TASMail disk but counted in neither denominator. | **P1 — correctness** | Medium — `would_exceed_quota` (which already uses `saturating_add` correctly) should be called with `used_bytes = attachment_total + quota_usage.used_bytes + shared_file_total`. Add `SharedFile::total_size_for_user(pool, user_id)` mirroring the attachment helper. Open product question: should `shared_files` count against the mail quota, or have a separate "TASMail Cloud" quota? The BYOK GHS 1.00 / GB · month pricing suggests one number; CLAUDE.md's "Pricing" section doesn't separate them. | New (P1; needs product decision) |
| 10 | **Attachment download relies on RLS for ownership, not an explicit `mailbox_id` filter.** `attachments.rs:189–202` calls `Attachment::find_by_id(&state.db, id)` with no mailbox-id filter, then the comment at `:198` says "RLS enforces ownership". This works if and only if the auth middleware reliably calls `SET app.current_user_id = $1` on every authenticated DB acquisition AND the RLS policy is engaged. The `find_by_token` path on shared-files explicitly bypasses RLS (`shared_files.rs:248`) because the public download endpoint has no auth context; the policy `USING (user_id = current_setting('app.current_user_id')::uuid)` would error on an unset GUC (casting empty string to uuid). Verifying that the public path works at runtime today is a follow-up. Defense-in-depth says always re-check. | **P1 — defense-in-depth** | Low — change `attachments.rs::download_attachment` and `delete_attachment` to call a `find_by_id_for_mailbox(pool, id, mailbox_id)` that adds `AND mailbox_id = $2` to the WHERE. Investigate the public `download_by_token` path: if RLS is in force, add a dedicated `tasmail_public` Postgres role with `BYPASSRLS` and route the unauth query through it; if RLS isn't being engaged today, document the assumption explicitly with a regression test. | New (P1; also worth a security-scanner sweep against the live deploy) |
| 11 | **Shared-file tokens cannot be rotated.** Once generated, the 64-hex-char `download_token` is permanent until the row is deleted. If the token leaks (forwarded email, logged-in browser history, shared screenshot, Sentry breadcrumb), the only revocation path is delete-and-recreate — which breaks anyone who's bookmarked the legitimate copy. Incident-response can't keep the file alive at a new URL. | **P2 — security ops** | Low — `POST /api/shared-files/{id}/rotate-token` that issues a new `download_token`, persists it, returns the new URL. Add `previous_token_revoked_at` so an audit log can answer "when was X rotated?". Standard secrets-rotation playbook. | New (P2) |
| 12 | **No rate-limit on `/api/dl/{token}`.** Public endpoint, no auth, reads file from disk, increments counter. Brute-force enumeration is not a concern (64 hex chars = 256 bits of entropy) but a single leaked token with no `max_downloads` and no `expires_at` lets an attacker grind disk I/O at line-rate. Combine with finding 4 (whole-file into memory): leaked 500 MB token = trivial one-liner that allocs 500 MB per concurrent request. | **P2 — DoS hardening** | Low — wire this route through the existing `middleware/rate_limit.rs` with a per-IP bucket of e.g. 20 / minute. Add a per-token concurrency cap (4 simultaneous, return 429 above). Worth pairing with the rate-limit work the security-scanners report (TMAIL-248) called out. | New (P2; pair with TMAIL-248 follow-up) |
| 13 | **`Attachment::storage_stats` aggregates the whole table per request.** `models/attachment.rs:145–168` runs one query with `COUNT`, `SUM`, `COUNT FILTER (WHERE scan_status='pending')`, `COUNT FILTER (WHERE scan_status='infected')` filtered by `mailbox_id`. The migration has `idx_attachments_mailbox(mailbox_id)` and a partial `idx_attachments_scan ON (scan_status) WHERE scan_status='pending'` — so the pending count is fast, but the other three aggregates do a partial index scan per call. For a heavy-attachment mailbox at 10K+ rows this becomes meaningful (>50 ms per call). Endpoint is called by `AttachmentManager.tsx` on mount and on every refresh. | **P2 — perf** | Low — composite `(mailbox_id, scan_status)` index covers all four aggregates. Long-term, a small `attachment_storage_summary(mailbox_id, count, size_bytes, pending_count, infected_count, updated_at)` rollup table maintained via insert/delete triggers would make this `O(1)`. Defer the rollup until row counts grow; the index is the cheap win. | New (P2) |
| 14 | **Shared-file content-type comes from the multipart header and is echoed back as `Content-Type` on download.** `handlers/shared_files.rs:62–65` accepts the client-provided `content_type` verbatim; `:292` echoes it on the public download response. The `Content-Disposition: attachment` header always being set saves the day for browsers, but the principle-of-least-authority answer is to allowlist a small set of known-safe types and normalize the rest to `application/octet-stream`. Same registry as finding 7. | **P2 — defense-in-depth** | Low — shared `services/mime_allowlist.rs::normalize(content_type) -> &'static str`. Reuse on the attachments handler too. Same commit as finding 7 fix. | New (P2; folds into finding 7) |
| 15 | **`upload_attachment` and `upload_shared_file` duplicate 50 lines of multipart parsing.** `handlers/attachments.rs:52–83` and `handlers/shared_files.rs:53–104` both do `while let Some(field) = multipart.next_field()` + a `match field_name` switch + `field.text() / field.bytes()`. The shared-files handler adds `max_downloads / expires_at / password` fields on top, but the file-extraction loop is byte-identical. Direct violation of the "modularise repeated logic" axis. | **P2 — modularisation** | Low — `services/multipart_helper.rs::extract_file_and_meta(multipart, &expected_text_fields) -> ParsedUpload`. Or adopt `axum-typed-multipart` (one extra crate) so the handlers become `Json<UploadRequest>`-shaped. Lets the handler bodies shrink from ~50 lines to ~15 of business logic. | New (P2) |
| 16 | **ClamAV protocol details live in `AttachmentService::scan_via_socket`.** `attachment_service.rs:121–135` connects to the Unix socket, writes `SCAN <path>\n`, reads the response. Plus the response parser at `:81–106` (looks for "OK" / "FOUND" / "ERROR"). It's 60 lines of protocol-specific code embedded in the attachment service. The phishing scanner has its own `scan_attachments` (signature-only, no daemon call) — distinct path. A future shared-files scanner (finding 17), DLP scanner (already separate but uses its own daemon connection), and any admin "ping ClamAV" health check would each re-implement the socket protocol. | **P2 — modularisation** | Low — extract into `services/clamav_client.rs::ClamAvClient::{scan(path), ping()}`. Inject via `AppState`. Reuse from `attachment_service`, `shared_files_service` (new — finding 17), `dlp_scanner`, and an admin `/api/admin/health/clamav` endpoint. The 4-test set in `attachment_service.rs:344–363` moves to `clamav_client.rs`. | New (P2; pair with finding 17) |
| 17 | **Shared files are not virus-scanned at all.** `upload_shared_file` never invokes ClamAV, never sets a `scan_status`, never blocks infected files on download. If a user treats TASMail's `/api/dl/{token}` as generic file hosting (it functionally is), they can host malware on a TASMail subdomain. Reputation risk for `mail.techatscale.io`. | **P2 — security policy** | Medium — same `tokio::spawn` post-upload scan pattern as attachments. Add `scan_status TEXT DEFAULT 'pending'`, `scan_result TEXT`, `scanned_at TIMESTAMPTZ` to `shared_files`. Block `download_by_token` if `scan_status != 'clean'`. Reuses the extracted `ClamAvClient` from finding 16. | New (P2; pair with finding 16) |
| 18 | **Frontend download reads `response.blob()` for whole-file in-memory.** `frontend/src/api/attachments.ts:90`. For a 25 MB attachment that's ~25 MB on the heap during the click-to-save round-trip. Tolerable today, breaks immediately if the cap goes up per finding 2. | **P2 — frontend perf** | Low — use `<a href="${API_BASE_URL}/attachments/${id}/download" download>` with `Authorization` via fetch-then-blob-URL pattern, or the `streamsaver` pattern (`ReadableStream` → `FileSystemWritableFileStream`). Server-streaming server-side (finding 4) only matters end-to-end if the client streams too. | New (P2; pair with finding 4) |
| 19 | **`QuotaBar` polls every 5 min and ignores recent mutations.** `frontend/src/components/layout/QuotaBar.tsx:16` `refetchInterval: 5 * 60 * 1000`, `staleTime: 2 * 60 * 1000`. An upload that just succeeded does not invalidate `queryKey: ['quota']` — the user sees stale-by-up-to-5-min usage after a meaningful write. Tolerable for a passive widget. | **P3 — UX freshness** | Low — `queryClient.invalidateQueries({ queryKey: ['quota'] })` from `attachmentsApi.upload`, `attachmentsApi.delete`, and the upcoming shared-files mutations. Folds into finding 8 (server-side cache invalidation) so the refetch hits a fresh quota. | New (P3; pair with finding 8) |
| 20 | **Positive baselines (keep doing this).** (a) **Attachments Range download is solid** — `attachments.rs:217–267` correctly implements RFC 7233 with 12 unit tests covering closed / open-ended / suffix / multi-range-first / EOF clamp / unsatisfiable / malformed / empty-file (`:436–502`), and the 416 path correctly emits `Content-Range: bytes */N` per §4.4. (b) **Filename sanitization** (`attachment_service.rs:231–243`) covers `/ \ \0 : * ? " < > \|` + control chars + leading-dot stripping; 5 unit tests including path-traversal (`../../etc/passwd → _.._etc_passwd`). (c) **Argon2id on shared-file passwords** (`shared_files.rs:131–144`) — proper salt generation, library defaults. (d) **`download_token` entropy** — 32 random bytes hex-encoded = 256 bits. URL-safe. Indexed UNIQUE. (e) **`would_exceed_quota` uses `saturating_add`** (`attachments.rs:359–361`) with a regression test for i64 overflow. (f) **RLS policies on `attachments`, `shared_files`, `quota_usage`** — `USING (mailbox_id = current_setting('app.current_user_id')::uuid)`. (g) **Checksum-based dedup** is the right call (just in the wrong order — see finding 6). (h) **`AttachmentService` is `Clone` + `Debug`** — easy to share into the `tokio::spawn` scan task. (i) **Quota cache key prefix** (`tasmail:quota:<mailbox_id>`) is namespaced consistently with the rest of the cache layer. | Positive baseline | — | — |

---

## 1. ClamAV fail-open by default (finding 1 in detail)

The relevant code:

```rust
// backend/src/services/attachment_service.rs:71–78
pub async fn scan_file(&self, path: &str) -> anyhow::Result<(String, Option<String>)> {
    let socket_path = match &self.clamav_socket {
        Some(s) => s.clone(),
        None => {
            tracing::debug!("ClamAV not configured, skipping scan for '{}'", path);
            return Ok(("clean".to_string(), Some("ClamAV not configured".to_string())));
        }
    };
    ...
}
```

And the call site:

```rust
// backend/src/handlers/attachments.rs:140–169
tokio::spawn(async move {
    match scan_service.scan_file(&scan_path).await {
        Ok((status, result)) => {
            Attachment::update_scan_status(&scan_pool, scan_id, &status, result.as_deref()).await
            // ...
        }
        ...
    }
});
```

And the gate on the read path:

```rust
// backend/src/handlers/attachments.rs:203–208
if attachment.scan_status == "infected" {
    return Err(AppError::Forbidden(
        "Cannot download file flagged as infected by virus scanner".to_string(),
    ));
}
```

The chain: missing ClamAV config → `Ok(("clean", "ClamAV not configured"))` → row marked
`scan_status = 'clean'` → download check `"clean" != "infected"` → file served.
The dev branch ("nice to not require ClamAV locally") quietly became the prod
policy ("we have no ClamAV configured, ship it anyway").

`StorageConfig::default()` (`config.rs:240–244`) seeds `clamav_socket: None`, so a
production deploy needs to set `CLAMAV_SOCKET` to opt into scanning. The
runbook in `BETA-LAUNCH-RUNBOOK.md` covers many things; nothing in the
build / deploy pipeline asserts that `clamav_socket.is_some()` in production.

### Fix shape

Two changes, one commit:

1. **Default-deny on missing config.** When `clamav_socket` is `None`, return
   `("pending", "ClamAV not configured")` instead of `"clean"`. The row stays
   `pending` and the download is blocked.
2. **Block all non-clean downloads.** Change the download check from
   `if scan_status == "infected"` to `if scan_status != "clean"`. Gate via a new
   `storage.require_clean_scan: bool` config (default true in prod, false in
   tests / dev). When `require_clean_scan = false`, also block "infected" but
   allow "pending" / "error" through.
3. **Startup warning.** In `main.rs`, log a `warn!` if
   `config.storage.clamav_socket.is_none() && config.storage.require_clean_scan`
   so the operator sees the mis-config the first second the server boots.

Same set of changes should be reflected on shared-files once finding 17 lands.

---

## 2. Frontend ↔ backend cap drift (finding 2 in detail)

The two numbers:

```ts
// frontend/src/utils/constants.ts:14–17
export const LARGE_FILE_THRESHOLD_BYTES = 25 * 1024 * 1024; // 25 MB
export const MAX_SHARED_FILE_BYTES = 500 * 1024 * 1024;     // 500 MB hard cap
```

```rust
// backend/src/config.rs:234–236
fn default_max_file_size() -> u64 {
    25 * 1024 * 1024 // 25 MB
}
```

`max_file_size` is consumed by BOTH upload handlers:

```rust
// backend/src/handlers/shared_files.rs:73–80
if data.len() as u64 > max_size {
    return Err(AppError::BadRequest(format!(
        "File size {} bytes exceeds maximum allowed {} bytes ({} MB)",
        ...
    )));
}
```

The SPA's `LargeFileAttacher.tsx:59` rejects files > `MAX_SHARED_FILE_BYTES`
(500 MB) client-side. Files between 25 MB and 500 MB pass the client gate,
get fully uploaded (multipart progress bar reaches 100%), then the backend
hands back a 400.

The product positioning makes this worse than a typical drift:

> _"TASMail handles overflow via large-file links — when an email attachment
> exceeds 25 MB, the Composer automatically routes it to the shared-files
> store instead."_

A user picking TASMail to escape Gmail's 25 MB cap and uploading a 200 MB
video file gets the SAME error as Gmail — except after a 5-minute upload
instead of immediately.

### Fix shape (two-tier limits)

Replace the single `max_file_size` field with:

```rust
pub struct StorageConfig {
    pub attachment_dir: String,
    pub max_attachment_bytes: u64,    // default 25 MB (RFC 5322 / Gmail parity)
    pub max_shared_file_bytes: u64,   // default 500 MB (BYOK overflow)
    pub clamav_socket: Option<String>,
    pub require_clean_scan: bool,
}
```

Have `attachments.rs` read `max_attachment_bytes` and `shared_files.rs` read
`max_shared_file_bytes`. Add a `GET /api/config/limits` endpoint
(`{ max_attachment_bytes, max_shared_file_bytes, large_file_threshold_bytes }`)
that the SPA's bootstrap fetches into a Zustand store; replace the constants in
`frontend/src/utils/constants.ts` with reads from that store. Same endpoint
serves the mobile app (currently mobile hardcodes the same 25 MB / 500 MB
pair).

Until the endpoint lands, at minimum patch `default_max_file_size` to
`500 * 1024 * 1024` so the user-visible behaviour matches the SPA's
expectation.

---

## 3. The missing shared-files sweeper (finding 3 in detail)

The expiry check today:

```rust
// backend/src/handlers/shared_files.rs:253–258
if shared_file.is_expired() {
    return Err(AppError::BadRequest(
        "This download link has expired".to_string(),
    ));
}
```

`is_expired()` is **only** called from `download_by_token`. The row and the
on-disk file stay forever. The `delete_shared_file` handler exists but only
runs on a manual `DELETE /api/shared-files/{id}` from the SPA's
`SharedFileManager.tsx`.

Grep confirms no background task touches the table:

```bash
$ grep -rn 'shared_files\|SharedFile' backend/src/services/ backend/src/main.rs
# (empty)
```

For a product priced GHS 1.00 / GB · month, expired files are pure unbilled
overhead — they consume disk and back up into the daily `pg_dump` from the
`BACKUP-RESTORE.md` runbook, but they generate no revenue. Worse, the disk
fills until the partition is full; the `tasmail-backend.service` then starts
returning 500s on every upload because `tokio::fs::write` fails with
`ENOSPC`.

Two missing indexes will hurt the sweeper:

```sql
-- backend/migrations/028_shared_files.sql
CREATE INDEX idx_shared_files_user ON shared_files(user_id);
CREATE UNIQUE INDEX idx_shared_files_token ON shared_files(download_token);
-- expires_at, max_downloads, download_count are not indexed
```

A sweep cycle `WHERE expires_at < NOW() OR ...` will seq-scan the whole
table.

### Fix shape

A new `services/shared_files_sweeper.rs`, started from `main.rs` next to
`EmailScheduler`:

```rust
pub struct SharedFilesSweeper { /* ... */ }

impl SharedFilesSweeper {
    pub fn start(self) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(3600));
            loop {
                tick.tick().await;
                // Postgres advisory lock so replicas don't race
                if let Err(e) = self.sweep_once().await {
                    tracing::error!("shared_files sweeper: {}", e);
                }
            }
        });
    }
    async fn sweep_once(&self) -> anyhow::Result<()> {
        let expired = sqlx::query_as::<_, SharedFile>(
            "SELECT * FROM shared_files
              WHERE expires_at < NOW()
                 OR (max_downloads IS NOT NULL AND download_count >= max_downloads)
              LIMIT 1000"
        ).fetch_all(&self.pool).await?;
        for f in expired {
            let _ = tokio::fs::remove_file(&f.storage_path).await;
            sqlx::query("DELETE FROM shared_files WHERE id = $1").bind(f.id)
                .execute(&self.pool).await?;
        }
        Ok(())
    }
}
```

Paired with a new migration:

```sql
-- backend/migrations/074_shared_files_sweeper_indexes.sql
CREATE INDEX idx_shared_files_expires ON shared_files(expires_at)
    WHERE expires_at IS NOT NULL;
CREATE INDEX idx_shared_files_exhausted ON shared_files(max_downloads, download_count)
    WHERE max_downloads IS NOT NULL;
```

Add a one-shot `--reap-orphans` admin command that scans `attachment_dir/`
and `shared-files/*` for files with no matching DB row (and DB rows with no
matching file), reports counts, optionally deletes. Useful after botched
deploys.

---

## 4. The shared-files download memory hot spot (finding 4 in detail)

```rust
// backend/src/handlers/shared_files.rs:279–299
let data = tokio::fs::read(&shared_file.storage_path).await.map_err(|e| { ... })?;
SharedFile::increment_download_count(&state.db, shared_file.id).await?;
let response = Response::builder()
    .header(header::CONTENT_TYPE, &shared_file.content_type)
    .header(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", shared_file.filename),
    )
    .header(header::CONTENT_LENGTH, data.len())
    .body(Body::from(data))
    ...
```

The whole file enters the address space before the first byte hits the wire.
For a 500 MB file, that's 500 MB of heap per request. Two concurrent
downloads of the same file pin 1 GB; ten pin 5 GB. The tunnel out to
`mail.techatscale.io` is the bottleneck — typical home-ISP upload bandwidth
on the workstation (10–50 Mbit/s) means a 500 MB download takes 80–400
seconds wall-clock, during which the 500 MB allocation stays alive.

Compare with `attachments.rs`'s `download_attachment` (`:189–289`) which:

1. Stats the file once for `Content-Length`.
2. Honours `Range` headers — for a 25 MB attachment, a Range request
   pulls only the requested slice into memory.
3. Falls back to whole-file read only when the client doesn't send `Range`,
   and even then 25 MB is within budget.

The attachments path has 12 unit tests for `parse_byte_range` exhaustively
covering RFC 7233. None of that is reused on shared-files.

### Fix shape

1. Lift `parse_byte_range` + `RangeError` from `handlers/attachments.rs:311`
   into `backend/src/services/range.rs`. Move the 12 unit tests with it.
2. Switch the read path to streaming:
   ```rust
   let file = tokio::fs::File::open(&shared_file.storage_path).await?;
   let stream = tokio_util::io::ReaderStream::new(file);
   Body::from_stream(stream)
   ```
3. Add Range handling to `download_by_token` using the shared helper.
4. Increment `download_count` BEFORE serving (otherwise a cancelled mid-stream
   download still consumes a download credit but doesn't count — pick one
   policy). The current "after successful read" semantics break under
   streaming because the read finishes when the user closes the tab.

End-to-end, peak memory per concurrent download drops from `file_size` to
~64 KiB.

---

## 5. Upload buffering, dedup order, MIME validation (findings 5–7 in detail)

These three findings overlap on the upload path; fixing them in one commit
is cleaner than three.

**The current order** in `upload_attachment`:

1. Read entire multipart field into `Bytes` (buffers in memory).
2. Check size against `max_size`.
3. Look up mailbox, sum existing attachment bytes, compare to `quota_bytes`.
4. `store_file` writes to disk, returns `(storage_path, checksum)`.
5. `find_by_checksum` checks for dedup. If hit, `delete_file` reverses step 4.
6. `Attachment::create` inserts the row.
7. `tokio::spawn` the ClamAV scan.

**The proposed order:**

1. Stream multipart chunks (`field.chunk()` loop) — never buffer >64 KiB.
2. Update a streaming SHA-256 hasher and a running size counter as chunks arrive.
3. Reject if size exceeds `max_size` mid-stream (delete temp file + return 400).
4. After last chunk, finalize hash → `checksum`.
5. `find_by_checksum`. If hit, return existing row — DON'T write the temp
   file's final path (just `tokio::fs::remove_file(temp)`).
6. Validate extension / MIME against `services/dangerous_attachments.rs`:
   - If `is_dangerous` and `block_dangerous_extensions = true`, return 400.
   - Otherwise normalize `content_type` via `services/mime_allowlist.rs::normalize`.
7. Quota check: `existing_attachment_total + quota_usage.used_bytes + size > quota_bytes` → 400.
8. `tokio::fs::rename(temp, final_path)` (atomic move on same FS).
9. `Attachment::create`.
10. `state.cache.invalidate_quota(&claims.sub).await`.
11. `tokio::spawn` ClamAV scan.

That sequence:

- Caps peak upload memory at the chunk size (~64 KiB).
- Avoids the disk write-then-delete dance on duplicate uploads (typical for
  forwarded PDFs).
- Validates extension before allocating a final path.
- Folds in the quota cache invalidation (finding 8).

Same shape applies to `upload_shared_file` minus the dedup step (shared-files
have no checksum column — adding one would let cross-user dedup work
eventually, but is out of scope for this report).

---

## 6. Cache + quota math drift (findings 8 + 9 in detail)

The cache is currently a one-way street: read-through populates, only
`sync_quota` invalidates. Every write path through attachments / messages /
IMAP move-and-delete bypasses it.

```rust
// backend/src/handlers/quota.rs:24–27
if let Some(cached) = state.cache.get_quota::<QuotaStatus>(&claims.sub).await {
    return Ok(Json(cached));
}
// ... fresh fetch from DB + IMAP ...
state.cache.set_quota(&claims.sub, &status).await;  // 60s TTL
```

The 60s TTL caps the staleness, so this isn't a correctness bug per se — but
it IS a UX bug: a user who just uploaded a 50 MB attachment and watches the
QuotaBar sees no movement for up to 65 s (60 s cache TTL + 5 s SPA refetch
debounce). Worse on the SPA side: `useQuery({ queryKey: ['quota'],
refetchInterval: 5 * 60 * 1000 })` means the bar can be 5 min stale even with
a fresh server.

Quota arithmetic is also incomplete. `upload_attachment` enforces:

```rust
// backend/src/handlers/attachments.rs:96–108
let mailbox = Mailbox::find_by_id(&state.db, mailbox_id).await?.ok_or(...)?;
if mailbox.quota_bytes > 0 {
    let used = Attachment::total_size_for_mailbox(&state.db, mailbox_id).await?;
    if would_exceed_quota(used, size_bytes, mailbox.quota_bytes) { return 400; }
}
```

`used` here is _attachments only_. The IMAP-reported usage from `quota_usage`
table is ignored. The shared-files-stored bytes are ignored. So:

- 950 MB IMAP messages + 60 MB attachments + 100 MB new attachment → check
  passes (60 + 100 = 160 MB < 1 GB), real usage is 1.11 GB. Over quota.
- 950 MB IMAP messages + 60 MB attachments + 400 MB new shared file → not
  checked at all (shared-files handler doesn't call this helper).

### Fix shape

A central `QuotaService` whose `current_usage(mailbox_id) -> i64` returns
`quota_usage.used_bytes + Attachment::total_size + SharedFile::total_size`.
Every write path (upload attachment, upload shared file, delete same, IMAP
sync) calls `cache.invalidate_quota` on success. The SPA invalidates the
`['quota']` query key from its upload mutations.

There's a real product question here: do shared-files count against the
TASMail mailbox quota, or do they get a separate "TASMail Cloud" bucket?
The BYOK pricing page (`/pricing`, calculator) doesn't separate them, but
the BYOK positioning makes the question more interesting — a user's IMAP
size is constrained by THEIR mail provider, while TASMail's disk is the
constraint on shared-files. Two-bucket accounting may be the right answer.
This decision needs product input before the implementation.

---

## 7. Severity ladder + suggested ticket ordering

Suggested order of attack, descending ROI:

| Order | Ticket | Findings |
|------|--------|---------|
| 1 | New P0 — "Fix ClamAV fail-open" | 1 |
| 2 | New P0 — "Align shared-file size caps" | 2 |
| 3 | New P0 — "Add shared-files expiry sweeper + indexes" | 3 |
| 4 | New P0 — "Stream `/api/dl/{token}` + Range support" | 4, (lifts the helper used by 18) |
| 5 | New P1 — "Streaming upload + checksum-first dedup" | 5, 6 |
| 6 | New P1 — "MIME / extension validation registry" | 7, 14 |
| 7 | New P1 — "Quota: cache invalidation + full-tank enforcement" | 8, 9 |
| 8 | New P1 — "Attachment defense-in-depth ownership filter" | 10 |
| 9 | New P2 — "Shared-file token rotation API" | 11 |
| 10 | New P2 — "Rate-limit + concurrency cap on `/api/dl`" | 12 |
| 11 | New P2 — "Storage stats composite index" | 13 |
| 12 | New P2 — "Extract multipart helper + ClamAV client" | 15, 16 |
| 13 | New P2 — "ClamAV scan on shared-files" | 17 |
| 14 | New P2 — "Streaming client downloads + QuotaBar invalidation" | 18, 19 |

Each is sized for a single PR with tests; #1–#4 should ship before opening
shared-files signup to anyone outside the closed beta. #5–#8 should ship
before GA. #9–#14 are cleanup.

---

## Acceptance

The accompanying commit adds this report and updates `INDEX.md` to mark
TMAIL-246 as In Review with the report linked. Each finding above maps to
a new ticket the queue picks up next; this assessment is the input doc,
not the implementation. Per the parent epic acceptance (TMAIL-241):

- [x] TMAIL-246 has a child report
- [ ] Follow-up tickets raised by the per-area report (this one) — pending,
      to be filed by the queue per the ordering above
