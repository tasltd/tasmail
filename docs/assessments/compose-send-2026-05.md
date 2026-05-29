# Compose, Drafts, Send & Scheduled Send Assessment

- **Issue:** TMAIL-244 (axis of TMAIL-241)
- **Date:** 2026-05-29
- **Scope (backend):** `backend/src/handlers/messages.rs` (covers both
  `POST /api/messages/send` and `POST /api/drafts` — the ticket called these out
  as `handlers/send.rs` + `handlers/drafts.rs`, but they live in one file),
  `backend/src/handlers/scheduled.rs`, `backend/src/handlers/snooze.rs`,
  `backend/src/handlers/queue.rs`, `backend/src/services/smtp_service.rs`,
  `backend/src/services/email_scheduler.rs`,
  `backend/src/services/queue_processor.rs` (the ticket called this
  `services/queue.rs`), `backend/src/models/scheduled_email.rs`,
  `backend/src/models/email_queue.rs`,
  `backend/migrations/005_scheduled_emails.sql`,
  `backend/migrations/018_email_queue.sql`,
  `backend/migrations/066_email_queue_priority_and_bounced.sql`,
  `backend/src/main.rs:60–97`.
- **Scope (frontend):** `frontend/src/components/mail/Composer.tsx`,
  `themes/shadcn-prototype/src/features/email/ComposeModal.tsx` (alt-UI),
  `frontend/src/api/messages.ts`, `frontend/src/api/scheduled.ts`,
  `frontend/src/utils/background-sync.ts`.
- **Method:** Static read of every file in scope, plus a grep sweep for actual
  call sites of `EmailQueueItem::enqueue`, `sendMessage`,
  `scheduledApi.scheduleSend`, and `messages.rs::send_message` to discover which
  paths are dead code vs live. Migration files cross-checked against model code
  to confirm indexes exist for the polling queries. No load run was captured —
  the SMTP RTT numbers cited are conservative ballpark figures, not measured.

---

## TL;DR — biggest wins, by ROI

| # | Finding | Severity | Effort | Suggested ticket |
|---|---------|----------|--------|------------------|
| 1 | **`EmailScheduler` sends every scheduled email with the literal string `"placeholder"` as the SMTP password.** `services/email_scheduler.rs:87` reads `smtp.send(&mailbox.username, "placeholder", &request)`. There is a `// NOTE: In production, stored encrypted credentials would be used` comment above it from the pre-BYOK era. Every send the user makes from the SPA flows through this code path (see finding 2), so **every send silently fails after the 10-second undo banner disappears.** The row gets marked `status='failed'`, no error column is populated (see finding 15), no notification reaches the user, no retry. The SPA shows a successful undo-send toast and walks away. | **P0 — production-breaking** | Low — swap the broken send_email helper for the BYOK + Redis-cache logic already proven by `QueueProcessor::process_item` (`services/queue_processor.rs:206–254`); decrypt with `derive_encryption_key(&jwt.secret)` exactly the way the queue processor does | New (P0 hotfix; subsumes the rest of #2 once landed) |
| 2 | **`POST /api/messages/send` — the synchronous BYOK send handler — is fully implemented but never called from any frontend.** `frontend/src/api/messages.ts:37` exports `sendMessage`, but `grep -rn 'sendMessage('` returns **zero call sites**. Both the classic SPA's `Composer.tsx:112` and the alt-UI's `ComposeModal.tsx:37` use `scheduledApi.scheduleSend` (with `delay_seconds: 10` and `delay_seconds: 0` respectively). The classic SPA does this so the undo banner works; the alt-UI does it to "use the same code path" (per the comment at `ComposeModal.tsx:15–18`). Net effect: the entire input-validation pass at `messages.rs:130–150`, the per-user SMTP-config cache lookup at `messages.rs:163–180`, the contact auto-collect at `messages.rs:200–222`, and the `EmailSent` webhook fire at `messages.rs:225–235` are dead in production. They run only in the unit-test paths and the background-sync replay loop. | **P0 — architectural** | Medium — either (a) gut `EmailScheduler` and make `scheduleSend` enqueue into the working `email_queue` (which already has retry / bounce / priority — see finding 3), or (b) revert the SPA to `sendMessage` for synchronous send and switch the undo flow to a debounced client-side hold. Option (a) is the more scalable answer | New (P0; pair with finding 1 + 3) |
| 3 | **The entire `email_queue` infrastructure — `claim_batch` with `FOR UPDATE SKIP LOCKED`, Prometheus metrics, hard-bounce NDR classifier, priority queue (urgent/normal/bulk), spec retry backoff (5s/30s/300s), Redis SMTP-config cache — is disconnected from the send path.** `EmailQueueItem::enqueue` (`models/email_queue.rs:64`) and `enqueue_with_priority` (line 83) have **zero call sites in the entire codebase** (verified with grep). `QueueProcessor` (`services/queue_processor.rs`) starts in `main.rs:89–97` and polls `email_queue` every 5 seconds — against a table no production code ever writes to. The migrations (TMAIL-58, `018_…` + `066_…`) shipped with index, CHECK constraint, priority column, and bounced state — all carrying weight in the schema, none being exercised. Meanwhile the path that IS used (`scheduled_emails` via `EmailScheduler`) has none of this — no retry, no priority, no bounce classification, broken SMTP password. | **P0 — architectural** | Medium — make `schedule_send` enqueue into `email_queue` with `priority=PRIORITY_NORMAL` and `next_retry_at = scheduled_at`, then rip out `EmailScheduler`. The `claim_batch` query already filters on `next_retry_at <= NOW()` so it'll naturally hold scheduled items until their time | New (P0; the obvious convergence of #1 + #2) |
| 4 | **`save_draft` APPENDs a fresh IMAP message on every 5-second autosave** instead of replacing the previous draft. `Composer.tsx:67–70` debounces 5s; `handlers/messages.rs:455–462` calls `imap_service.save_draft(...)`; `imap_service.rs:516–518` does `session.append("Drafts", Some("(\\Draft \\Seen)"), None, raw_message)`. There is no `UID STORE \Deleted` on the previous draft and no `EXPUNGE`. A 10-minute compose session generates ~120 draft messages. For a chatty user the IMAP Drafts folder becomes the largest folder in the mailbox within a week, and the SPA's draft list (sorted DESC by UID) shows 120 near-identical rows. Multiplies the IMAP server's storage and, on Gmail BYOK, may push the account past the free-tier quota. | **P0 — correctness + storage** | Medium — track the last draft UID in localStorage (or a `drafts` DB table keyed by `(mailbox_id, client_draft_id)`), send it on every autosave, have the backend `UID STORE old_uid \Deleted` + `EXPUNGE` + `APPEND` in one IMAP session. Alternative: use IMAP `UIDPLUS APPENDUID` to learn the new UID and keep the chain in the SPA | New |
| 5 | **`smtp_service.rs` routes both `"ssl"` (implicit-TLS, port 465) and `"starttls"` (port 587) through `starttls_relay`.** `messages.rs:189` does `tls: matches!(smtp_cfg.encryption.as_str(), "ssl" \| "starttls")`, then `smtp_service.rs:104–109` branches on `self.config.tls`: if true → `AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host).port(port).credentials(creds).build()`. There is no `smtps_relay` branch. BYOK users with implicit-TLS-only SMTP (Yahoo on 465, some legacy corporate Exchange, anything that refuses STARTTLS on 587) will hit a TLS handshake failure: the client connects, immediately tries to send `STARTTLS` as plaintext, the server's TLS port rejects it as garbage, and lettre returns an error. Onboarding's "Test SMTP" check (`POST /api/imap-configs/test`'s SMTP equivalent if it exists) won't catch this if it tests with `starttls_relay` either. | **P0 — BYOK correctness** | Low — add a third branch: `match encryption { "ssl" => smtps_relay, "starttls" => starttls_relay, _ => builder_dangerous }`. Audit all four send sites (`send`, `send_imip_request`, `send_imip_reply`, `send_notification`) and extract a shared `build_transport(&self, creds: Option<Credentials>) -> AsyncSmtpTransport<Tokio1Executor>` helper to keep them in lockstep | New (P0 once a Yahoo / port-465 BYOK customer signs up) |
| 6 | **Every send opens a fresh TCP + TLS + AUTH session against the BYOK SMTP server.** `smtp_service.rs:32–123` builds `AsyncSmtpTransport` per call. lettre's `AsyncSmtpTransportBuilder::pool_config(PoolConfig::default())` would maintain a small persistent connection pool keyed by `(host, port, creds)`, but it's not used. Per-send overhead against typical providers: Gmail ~600 ms TCP+TLS+AUTH+EHLO, Outlook ~500 ms, Zoho ~400 ms (mirrors the IMAP measurements in `folders-messages-2026-05.md` §1 — same RTT × handshake math). For a user forwarding 5 messages in a row that's 5×500 ms = 2.5 s of pure handshake. Once the queue is wired up properly (findings 1–3) and `QueueProcessor` batches start filling, this becomes the dominant per-item cost. | **P1 — performance** | Medium — add an `Arc<DashMap<(host, port, user_hash), AsyncSmtpTransport<Tokio1Executor>>>` to `AppState`; reuse the transport across calls; idle-timeout after 60 s. lettre's pool handles per-transport concurrency so a single transport per BYOK config is fine | New |
| 7 | **`cancel_scheduled` does not verify mailbox ownership.** `handlers/scheduled.rs:77–89` takes the `cancel_token` from the URL path, ignores `claims.sub`, and calls `ScheduledEmail::cancel_by_token(&state.db, cancel_token)` which only checks `WHERE cancel_token = $1 AND status = 'pending'`. The token IS a UUIDv4, so it's probabilistically unguessable, but the auth check is missing. A leaked cancel_token (logged in browser history, copied into a chat, included in a Sentry breadcrumb) becomes a permanent capability for anyone with a valid TASMail JWT. Compare with `snooze.rs:48–64` which DOES check `mailbox_id` on cancel. | **P1 — security** | Low — add `AND mailbox_id = $2` to the cancel query, pass `claims.sub` through. Return 404 (not 403) on mismatch so the endpoint doesn't leak token existence | New |
| 8 | **`save_draft` hardcodes the folder name `"Drafts"`.** `imap_service.rs:517` does `session.append("Drafts", ...)`. Gmail BYOK reports its drafts folder as `[Gmail]/Drafts`, Outlook 365 as `Drafts` (lucky), Yahoo as `Draft` (singular), iCloud as `Drafts`. For Gmail BYOK users — explicitly listed in TASMail's BYOK preset list — `save_draft` fails silently against the server with `NO [TRYCREATE]`. Same finding shape as `folders-messages-2026-05.md` finding 7 about `"Trash"`, this time on the write path. | **P1 — BYOK correctness** | Medium — once finding 7 of folders-messages-2026-05 lands (`SpecialFolder` enum resolved via `LIST … RETURN (SPECIAL-USE)`), wire the same lookup here. Until then, the fix is a `imap_service.resolve_special("\\Drafts").await?` helper with the same in-memory mapping | New (P1; covered jointly with folders-messages finding 7) |
| 9 | **Background-sync replay sends with no idempotency key.** `utils/background-sync.ts:113–125` queues `{type: 'send', payload: {…}}` and replays via `scheduledApi.scheduleSend`. If the browser tab is killed mid-replay (the `executeAction` call succeeds but the `remove(id)` doesn't run), the queue keeps the action and re-fires it next tick. The user gets two scheduled emails, two undo windows, two real sends. There's a `retries` counter capped at 3 to prevent infinite duplication, but the duplication still happens 0–3 times per killed tab. `POST /api/messages/schedule` does not accept an `Idempotency-Key` header. | **P1 — correctness** | Medium — generate a UUID per queue entry (already happens via IDB autoIncrement), send it as `Idempotency-Key` header, have the backend store it on `scheduled_emails` with a UNIQUE constraint and return the existing row on collision | New |
| 10 | **`Composer.tsx` is a 393-line component owning 13 stateful slots + the TipTap editor instance + 3 panel modals + the undo countdown + the auto-save loop + the API calls + the presentation.** State: `to`, `cc`, `subject`, `sending`, `error`, `draftStatus`, `undoState`, `showSchedulePicker`, `scheduleDate`, `showAiCompose`, `showLargeFile`, `showMeetingModal`, `draftTimerRef`, `undoTimerRef`. Direct violation of the modular-implementation rule (Composer is the canonical "500-line component" the rule names as an anti-pattern). | **P2 — modularisation** | Medium — `useCompose()` hook for the state + autosave + send logic; `ComposeRecipients`, `ComposeEditor`, `ComposeToolbar`, `ComposeSchedulePicker`, `ComposeUndoToast` as presentational children; `ComposePanels` for the AI/LargeFile/Meeting toggles | New |
| 11 | **TipTap (StarterKit + Link + Placeholder), `AiComposePanel`, `LargeFileAttacher`, `RecipientAutocomplete`, and `ScheduleMeetingModal` are all eagerly imported at the top of `Composer.tsx`.** TipTap + StarterKit alone is ~150 KB gzipped, AiComposePanel pulls in the AI client + smart-reply machinery, LargeFileAttacher pulls in the chunked uploader. A user who never composes (read-only check-mail) still pays the bundle cost. The TanStack-Query message list loads first, so this lands in the **main** bundle, not a route-split chunk. | **P2 — frontend bundle** | Low — `const Composer = lazy(() => import('./components/mail/Composer'))` at the route boundary, `Suspense` fallback, and inside Composer use `lazy()` for `AiComposePanel` / `LargeFileAttacher` / `ScheduleMeetingModal` which are gated behind toggle state anyway | New (overlaps with TMAIL-259 frontend-bundle) |
| 12 | **`save_draft` builds RFC 2822 by hand instead of reusing lettre's `Message::builder`.** `messages.rs:433–453` does raw string concatenation: `From:`, `To:`, `Cc:`, `Subject:`, `Date:`, `MIME-Version:`, optional `multipart/alternative` with manual boundary. The compose-send path at `messages.rs:194` and the iMIP path at `smtp_service.rs:130–168` both use the lettre builder. Two different MIME builders for what should be the same compose↔draft round-trip means the draft and the sent message can have subtly different framing (Content-Transfer-Encoding, header folding, Date format). | **P2 — code dedupe** | Low — build a `lettre::Message` from `SaveDraftRequest`, then call `message.formatted()` (returns `Vec<u8>`) and pass that to `imap_service.save_draft`. Reuses the same MIME generator as send | New |
| 13 | **`smtp_service.rs` duplicates the transport-build logic four times.** `send` (lines 102–115), `send_imip_request` (lines 191–204), `send_imip_reply` (lines 261–274), `send_notification` (lines 330–358) each branch on `self.config.tls`, build credentials, and call `starttls_relay` / `builder_dangerous`. Combined with finding 5 (missing `smtps_relay` branch), any fix has to be applied in 4 places. | **P2 — code dedupe** | Low — `fn build_transport(&self, creds: Option<Credentials>) -> Result<AsyncSmtpTransport<Tokio1Executor>, AppError>` once, call from all four send sites | New (pair with finding 5 to land both in one commit) |
| 14 | **`EmailScheduler::process_pending` sends sequentially.** `services/email_scheduler.rs:50–61` does `for email in emails { match self.send_email(&smtp, &email).await ... }`. With `find_ready_to_send` returning up to 50 rows and a fixed SMTP RTT of ~500 ms, one cycle takes up to 25 s. The poll interval is 5 s. Once finding 1 unblocks the send path, lag builds immediately under any send burst. Compare with `QueueProcessor::tick` (lines 167–193) which uses `FuturesUnordered` with `worker_concurrency=4` — drains the same 50 in ~6 s. | **P2 — performance (becomes P1 once #1 lands)** | Low (moot if finding 3 is taken — the queue processor replaces this loop entirely) | New (or moot; depends on finding 3 outcome) |
| 15 | **`scheduled_emails.mark_failed` discards the error string.** `models/scheduled_email.rs:107–117` runs `UPDATE … SET status='failed' WHERE id=$1`, then `tracing::error!("Scheduled email {} failed: {}", id, error)`. The error never lands in the DB. The migration (`005_scheduled_emails.sql`) has no `error_message` / `failure_reason` column. So a user looking at their scheduled-emails list sees "failed" with no diagnostic. The UI also has nowhere to read it from. Compare with `email_queue.last_error TEXT` (migration `018_…`) which IS persisted. There is also no retry: one failure → permanent `'failed'`. | **P2 — observability + UX** | Low — add `error_message TEXT` column via new migration, persist via the UPDATE, surface in `ScheduledEmail` model + `list_scheduled` endpoint, render the chip in the SPA's "Scheduled" view. Moot if finding 3 is taken (queue already has this) | New (or moot) |
| 16 | **`list_snoozed` is unpaginated.** `handlers/snooze.rs:34–45` calls `SnoozedEmail::list_by_mailbox` (no limit) and returns the whole list. For a heavy snooze user that's unbounded. Compare with `list_for_mailbox` on scheduled_emails which has `LIMIT 100`. | **P3 — scalability** | Low — add `LIMIT $N` + page param | New |
| 17 | **Positive baselines (keep doing this):** `messages.rs:130–150` validates ALL user-controlled headers + body BEFORE building the message (TMAIL-37 — defence-in-depth against CRLF injection); `messages.rs:206–222` auto-collects contacts via `tokio::spawn` so DB hiccups don't block send (TMAIL-119); `messages.rs:225–235` fires the `EmailSent` webhook via spawn (TMAIL-131); `messages.rs:163–180` reads SMTP config via Redis cache then falls through to DB (TMAIL-158); `queue_processor.rs` is genuinely production-grade infrastructure — the only issue is it's not connected (see finding 3); `is_hard_bounce` NDR classifier has solid coverage including transient/case-insensitive tests; `cancel_by_token` keeps the UUID secret out of the URL (it's the path param, but the token is a UUIDv4 with 122 bits of entropy). | Positive baseline | — | — |

---

## 1. The send-path inversion (findings 1 + 2 + 3 in detail)

This is the assessment's headline finding and warrants a longer write-up. It is
the result of three independent design decisions interacting badly:

1. **TMAIL-58** built the production-grade `email_queue` table + `QueueProcessor`
   for outgoing mail with retry, backoff, priority, NDR classification, and
   Prometheus metrics. This is the path the CLAUDE.md describes as "the queue".
2. **TMAIL-undo-send** (the scheduled_emails table + EmailScheduler poller +
   cancel_token round-trip) was built earlier as a quick way to power Gmail-style
   "Undo send (10 s)". It was never converted to enqueue via the new queue; it
   kept its own simple poller.
3. **The classic SPA's `Composer.tsx:112`** and **the alt-UI's
   `ComposeModal.tsx:37`** both route through `scheduledApi.scheduleSend` —
   classic because it wants the undo banner, modern because the commit message
   on `ComposeModal.tsx:15–18` says it wants to "use the same code path the
   production SPA's composer does".

The result: **every** send the user makes goes through the
`scheduled_emails` table → `EmailScheduler` poller → broken `"placeholder"`
password. Synchronous `POST /api/messages/send` is fully implemented (BYOK SMTP
load, AES-GCM decrypt, validation, contact auto-collect, webhook fire) but
never hit. The queue (`email_queue`) is never written to.

### Live impact

For each send the user thinks succeeded:
- T+0 s: SPA shows "Message sent (10s) [Undo]" toast.
- T+10 s: `EmailScheduler::process_pending` picks up the row, calls
  `smtp.send(&mailbox.username, "placeholder", &request)`.
- T+10.4 s: SMTP server responds `535 5.7.8 Authentication failed`. lettre
  returns `Err`.
- `mark_failed` updates `status='failed'`, logs the error to stdout, returns.
  No notification, no email column, no retry.
- The recipient never sees the message. The user sees no error.

### Repro

1. Sign up via `/signup`, attach a Gmail BYOK config.
2. Compose a message to a known good external address. Click Send.
3. Wait 15 seconds. Watch nothing arrive.
4. `SELECT status, sent_at, scheduled_at FROM scheduled_emails ORDER BY created_at DESC LIMIT 1`
   → `status='failed'`, `sent_at IS NULL`.
5. `tail -f` the backend logs:
   `ERROR Scheduled email <uuid> failed: SMTP send failed: …Authentication failed…`.

### Recommended consolidation

The cleanest fix is to converge the two paths into the queue:

1. **`schedule_send` enqueues into `email_queue` with `next_retry_at = scheduled_at`**.
   The queue's existing `claim_batch` filter (`status IN ('pending','failed') AND next_retry_at <= NOW()`)
   means the row stays invisible to workers until the scheduled time. Priority
   stays at `PRIORITY_NORMAL`. The cancel_token semantics move to a small
   `email_queue_cancel_tokens` join table or — simpler — a `cancel_token UUID`
   column on `email_queue` itself.
2. **`cancel_scheduled` deletes (or sets `status='cancelled'`, requires adding
   that to the CHECK constraint) the queue row.** Wins ownership check for free
   from finding 7.
3. **Delete `services/email_scheduler.rs` and `models/scheduled_email.rs`.**
   Drop the `scheduled_emails` table in a follow-up migration (after a
   one-time copy of any unsent pending rows).
4. **`messages.rs::send_message` stays for synchronous sends** but now also
   enqueues if the request has `Idempotency-Key` or `delay_seconds`. Or — the
   simpler call — always enqueue, with `delay_seconds=0` going to the front of
   the priority queue (`PRIORITY_URGENT`).

This removes the duplicated infrastructure, the broken password path, the
sequential poll loop, the missing retry/bounce coverage on scheduled mail, and
the `cancel_scheduled` ownership bug — all in one architectural decision.

---

## 2. Composer modularisation

`Composer.tsx` violates two of the project's STANDARD RULES:

- **Modularize implementation code** — 393 LOC, 13 state slots, single
  component owning fetching + state + presentation + 3 panel toggles + the
  TipTap editor instance + the undo countdown timer + the autosave debounce.
- **Scalability & long-term performance first** — TipTap + 3 sub-panels eager
  in the main bundle even for users who never compose.

### Suggested decomposition

```
Composer.tsx                  ←  orchestrator (50 LOC)
├── useCompose.ts             ←  state + autosave + send + undo logic
├── ComposeRecipients.tsx     ←  To / Cc rows with autocomplete
├── ComposeEditor.tsx         ←  TipTap mount (lazy)
├── ComposeToolbar.tsx        ←  Send / Schedule / AI / Attach / Meeting buttons
├── ComposeSchedulePicker.tsx ←  datetime-local + Schedule Send
├── ComposeUndoToast.tsx      ←  countdown + Undo button
└── ComposePanels.tsx         ←  lazy-mounted Ai / LargeFile / Meeting modals
```

Plus:
- `Composer` itself becomes `lazy()` at the route boundary in `AppShell`.
- `AiComposePanel` / `LargeFileAttacher` / `ScheduleMeetingModal` become
  `lazy()` inside `Composer` since they're gated behind toggle state.

Estimated bundle savings: ~180 KB gzipped off the main bundle, deferred until
the user first clicks Compose. (Cross-references TMAIL-259 frontend-bundle
finding for the same components.)

### TipTap extensions registry

The `useEditor` call hardcodes `[StarterKit, Link, Placeholder]`. Adding
bold-only / lists / mentions / attachments-via-paste means editing this file.
Lift to:

```ts
// frontend/src/components/mail/compose-extensions.ts
export const COMPOSER_EXTENSIONS = [
  StarterKit,
  Link.configure({ openOnClick: false }),
  Placeholder.configure({ placeholder: 'Write your email...' }),
];
```

— same pattern as the FOLDER_ICONS registry in the folders-messages-2026-05
report (finding 7), and lets the alt-UI's `ComposeModal` reuse the same
extension list without copy-paste.

---

## 3. Drafts — the autosave amplification problem (finding 4 in detail)

The current shape:

```
Composer        Backend                    IMAP
  │                                          │
  │ keystroke (5s debounce)                  │
  │  POST /api/drafts                        │
  │ ─────────────────────► save_draft        │
  │                       │ APPEND Drafts    │
  │                       │ ────────────────►│  ← new UID created
  │  201 Created          │ EXPUNGE? no.     │
  │ ◄─────────────────────│                  │
  │                                          │
  │ another keystroke (5s)                   │
  │  POST /api/drafts                        │
  │ ─────────────────────► save_draft        │
  │                       │ APPEND Drafts    │
  │                       │ ────────────────►│  ← another new UID
```

After a 10-minute compose, the Drafts folder contains ~120 near-identical
near-duplicates. The user's "Drafts (124)" sidebar count reflects this. On
Gmail BYOK with the free 15 GB tier, a chatty user can saturate quota faster
than legitimate sent mail.

### Recommended shape

Track the previous draft UID in the SPA (Composer-local state, no persistence
needed because a tab close ends the autosave loop anyway):

```ts
const [draftUid, setDraftUid] = useState<number | null>(null);
…
const result = await saveDraft({
  …,
  replace_uid: draftUid,
});
setDraftUid(result.new_uid);
```

Backend:

```rust
pub async fn save_draft(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SaveDraftRequest>,
) -> Result<Json<SaveDraftResponse>, AppError> {
    // …validation…
    let new_uid = imap_service
        .replace_draft(_imap_user, _imap_pass, body.replace_uid, raw_msg.as_bytes())
        .await?;
    Ok(Json(SaveDraftResponse { uid: new_uid }))
}
```

Inside `ImapService::replace_draft`:

```rust
let mut session = self.connect(username, password).await?;
session.select(&drafts_folder).await?;  // ← uses resolved special-use folder
if let Some(old_uid) = replace_uid {
    session.uid_store(format!("{}", old_uid), "+FLAGS (\\Deleted)").await?;
}
session.append(&drafts_folder, Some("(\\Draft \\Seen)"), None, raw_message).await?;
if replace_uid.is_some() {
    session.expunge().await?;
}
// Use UIDPLUS APPENDUID to learn the new UID without a follow-up SEARCH.
let new_uid = session.append_response.uidplus_uid.unwrap_or(0);
session.logout().await.ok();
Ok(new_uid)
```

Cuts draft accumulation from O(autosaves) to O(1) per compose session.

---

## 4. SMTP / IMAP connection reuse (finding 6 in detail)

The compose-send-drafts path opens **at minimum 2 fresh sessions** per send-with-draft:

| Operation | Fresh session? | RTT cost (Gmail BYOK) |
|-----------|----------------|----------------------|
| Autosave draft (per keystroke burst) | Yes — IMAP | ~400 ms |
| Final autosave on Send click | Yes — IMAP | ~400 ms |
| `POST /api/messages/schedule` row insert | No (DB only) | <5 ms |
| 10 s later: `EmailScheduler::send_email` | Yes — SMTP | ~600 ms |
| Webhook fire | No (spawned, async HTTP) | — |

For a single send-with-autosaved-draft: ~1.4 s of pure handshake. With pooling
(IMAP per-mailbox + SMTP per-config) it drops to <100 ms after warm-up.

The fix is the same shape as `folders-messages-2026-05.md` finding 1 — add a
`MailboxConnectionPool { mailbox_id → Arc<Mutex<Session>> }` for IMAP, and an
`SmtpTransportPool { (host, port, user_hash) → AsyncSmtpTransport }` for SMTP.
lettre's `pool_config` does the SMTP side natively.

---

## 5. Snooze — minor

Snooze handlers are correct and ownership-checked, but:

- `list_snoozed` is unpaginated (finding 16).
- `cancel_snooze` returns `404` on not-found-or-not-owned, which is the right
  shape — POSITIVE.
- No background worker exists to actually un-snooze (move back to Inbox at
  `snooze_until`). `snoozed_emails` table is written to, but who reads it?
  grep for `snooze_until` in services returns nothing besides the model itself.
  **Possible separate finding** for the snooze-feature assessment if there is
  one (TMAIL-244 isn't formally the snooze ticket — that's out of scope for
  this report). Flagging for triage.

---

## 6. Migrations — schema-only observations

`005_scheduled_emails.sql` indexes:

- `idx_scheduled_emails_mailbox` on `(mailbox_id)` — covers list-by-mailbox.
- `idx_scheduled_emails_status_time` on `(status, scheduled_at)` — covers the
  poller's `WHERE status='pending' AND scheduled_at <= NOW()` correctly. The
  index can be made a **partial** index `WHERE status = 'pending'` for a
  ~10× smaller index, matching the pattern in
  `018_email_queue.sql` (now `idx_email_queue_ready`).
- `idx_scheduled_emails_cancel_token` on `(cancel_token)` — covers cancel.
  Could be UNIQUE (the column already is via `DEFAULT uuid_generate_v4()`
  giving collision-resistant uniqueness; a UNIQUE constraint would catch
  application bugs that re-use a token).

No `error_message` column — see finding 15.

`018_email_queue.sql` + `066_…` are good production-grade schema. No notes.

---

## 7. Validation coverage (positive baseline)

`messages.rs:130–150` validates **every** user-controlled byte before lettre
sees it (TMAIL-37):

- `validate_subject` — CRLF, length cap.
- `validate_recipient_list` — per-address email format, CRLF in display name.
- `validate_body_size` — caps both `text_body` and `html_body`.

`save_draft` does the same (lines 406–416). This is defence-in-depth because
lettre rejects most malformed headers natively, but the validation pass
short-circuits at the API boundary, so hostile input never reaches the SMTP
transport.

**Keep doing this** — when finding 3 lands and `email_queue` is the only
write path, validate at enqueue time, not at dequeue (because by dequeue
the row is already persisted with the bad bytes).

---

## 8. Severity rollup

| Severity | Findings | Notes |
|----------|----------|-------|
| **P0 — production-breaking** | 1, 2, 3, 4, 5 | findings 1–3 are the same problem, three angles. Fix together. |
| **P1 — load-bearing pre-GA** | 6, 7, 8, 9 | SMTP perf, security on cancel, BYOK drafts folder, idempotency |
| **P2 — cleanup** | 10, 11, 12, 13, 14, 15 | refactor + dedupe + observability |
| **P3 — backlog** | 16 | snooze pagination |
| **Positive baseline** | 17 | validation, webhooks, contact auto-collect, Redis cache, NDR classifier, queue infra (when connected) |

The beta-launch runbook (`docs/BETA-LAUNCH-RUNBOOK.md`) should be informed of
findings 1–5 before the closed beta ramps — the broken-send path is a
ship-blocker. The auto-fix queue can land the consolidation (findings 1+2+3)
as a single PR per the spec-first development rule because the spec is
"converge on `email_queue`, retire `scheduled_emails` + `EmailScheduler`".

---

## 9. Follow-up tasks

Filed as siblings of TMAIL-244 (priority preserved from severity rollup):

1. **TMAIL-NEW (P0 hotfix)** — replace `EmailScheduler` placeholder password
   with BYOK SMTP load (covers findings 1, 14, 15) — OR — go straight to #2.
2. **TMAIL-NEW (P0 architectural)** — converge `scheduled_emails` onto
   `email_queue`, retire `EmailScheduler`, wire `schedule_send` to
   `EmailQueueItem::enqueue_with_priority(…, PRIORITY_NORMAL)` with
   `next_retry_at = scheduled_at`; transfer cancel_token semantics; backfill
   pending rows; drop the table in a follow-up migration. Covers findings 1,
   2, 3, 7, 14, 15 in one commit.
3. **TMAIL-NEW (P0 BYOK)** — `smtp_service.rs` add `smtps_relay` branch +
   extract `build_transport` helper, audit all four send sites. Covers
   findings 5 + 13.
4. **TMAIL-NEW (P0 storage)** — draft replace-by-UID flow (`UIDPLUS` +
   `STORE \Deleted` + `EXPUNGE` in one IMAP session). Covers finding 4.
5. **TMAIL-NEW (P1 perf)** — SMTP transport pool via lettre `pool_config`
   keyed by `(host, port, user_hash)`. Covers finding 6.
6. **TMAIL-NEW (P1 BYOK)** — `SpecialFolder::Drafts` resolution via
   `LIST … RETURN (SPECIAL-USE)`. Covers finding 8 (joint with the open
   folders-messages-2026-05 finding 7 on `\Trash` / `\Sent`).
7. **TMAIL-NEW (P1 correctness)** — `Idempotency-Key` header on
   `POST /api/messages/schedule` and `POST /api/messages/send`, persisted on
   the queue row. Covers finding 9.
8. **TMAIL-NEW (P2 modular)** — Composer.tsx decomposition + lazy-load.
   Covers findings 10, 11.
9. **TMAIL-NEW (P2 dedupe)** — `save_draft` builds via lettre `Message`,
   passes `formatted()` to IMAP APPEND. Covers finding 12.
10. **TMAIL-NEW (P3)** — paginate `list_snoozed`. Covers finding 16.

Plus the **non-scope** flag from §5: the snooze table appears to have no
worker that un-snoozes back to Inbox. Belongs in TMAIL-247 (Calendar &
Contacts) only loosely; more likely a missed Compose/Snooze service. Flag
for triage at the parent epic.

---

*Generated under TMAIL-241. See [INDEX.md](INDEX.md) for the other
per-feature reports.*
