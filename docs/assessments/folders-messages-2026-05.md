# Folders & Messages (IMAP Read Path) Assessment

- **Issue:** TMAIL-243 (axis of TMAIL-241)
- **Date:** 2026-05-29
- **Scope (backend):** `backend/src/handlers/folders.rs`, `backend/src/handlers/messages.rs`,
  `backend/src/services/imap_service.rs`, `backend/src/services/cache_service.rs`,
  `backend/src/validation.rs`, related models
- **Scope (frontend):** `frontend/src/components/mail/MessageList.tsx`,
  `frontend/src/components/mail/MessageView.tsx`, `frontend/src/components/mail/FolderTree.tsx`,
  `frontend/src/hooks/useMailbox.ts`, `frontend/src/stores/mailStore.ts`,
  `frontend/src/api/folders.ts`, `frontend/src/api/messages.ts`,
  `frontend/src/utils/constants.ts`, `frontend/src/App.tsx`
- **Method:** Static read of every file in scope plus a grep sweep for hardcoded folder
  names across `backend/src/**/*.rs` and `frontend/src/**`. No live load test was
  captured — IMAP performance numbers below are extrapolated from RTT × visible round
  trips, not measured. A profiler/k6 run is the suggested follow-up (see §8).

---

## TL;DR — biggest wins, by ROI

| # | Finding | Impact | Effort | Suggested ticket |
|---|---------|--------|--------|------------------|
| 1 | **Every IMAP request opens a brand-new TCP+TLS+LOGIN session.** `ImapService` caches the **config row** in Redis (TMAIL-162) but does **not** pool live `Session`s — `connect()` is called once per `list_folders` / `list_messages` / `get_message` / `move_message` / `set_flag` / `save_draft`, followed by an unconditional `LOGOUT` (`imap_service.rs:156,233,313,396,442,482,514,596`). Opening a new IMAPS session against a typical provider is ~150–400 ms (TCP RTT + TLS handshake + LOGIN + SELECT) — on Gmail it's closer to 600 ms. The SPA opening Inbox issues at minimum 2 requests (folders + messages) → ~1 s before the first envelope hits the wire even when the user already had it cached. Click into a message → another full handshake. This is **the** dominant latency in the read path. | **High** (felt on every page) | Medium — add a `MailboxConnectionPool { mailbox_id → Mutex<Session> }` keyed by mailbox UUID, idle timeout 60 s, max 1 session per mailbox (IMAP protocol is stateful so don't try to share across SELECTs without serialising) | New (sibling of TMAIL-158 / 162) |
| 2 | **`get_quota` does a full `FETCH 1:* RFC822.SIZE` against every folder, every call.** `imap_service.rs:528–586` walks every folder, `SELECT`s it, then fetches `RFC822.SIZE` for every message in it. For a 50 k-message mailbox spread across 8 folders that's 50 000 FETCHes per quota check. There is a 60 s Redis quota cache (`cache_service.rs:139–155`), so it's not every request — but it's every request that missed cache, and on a quota miss the user blocks for tens of seconds. IMAP `GETQUOTA` (RFC 2087) or `STATUS … (X-SIZE)` (Dovecot extension) gives the same number in a single round trip when the server supports it. | **High** (worst-case multi-second response on cache miss; also bad citizen against Gmail's quota-per-account limits) | Medium — try `GETQUOTA` first; fall back to the current scan only if the server reports `NO Quota not supported`; cache the negative result too | New |
| 3 | **No `(folder, internaldate)` index — by design, but pagination uses sequence numbers from the end, which silently breaks on concurrent change.** `list_messages` (`imap_service.rs:225–303`) computes `end = total - page*page_size`, `start = end - page_size`, then `FETCH start:end (UID ENVELOPE FLAGS RFC822.SIZE)` and reverses the result client-side. There is no SORT or fetch-by-UID — if new mail arrives between page 0 and page 1 the user will see one envelope twice and miss another (the classic "shifting window" bug). IMAP `SORT` (RFC 5256) or fetching by descending UID range is the durable fix. There's also no `(folder, internaldate)` index in the DB — and there shouldn't be, since TASMail's architecture is "IMAP is the source of truth" — but that means we cannot fall back to a local cache when SORT is missing. | Medium (correctness — duplicate/missing envelopes during refresh) | Medium — switch to `UID SEARCH ALL` + slice the resulting UID list, or use `SORT REVERSE ARRIVAL` when the server advertises it | New |
| 4 | **`MessageList` is not virtualised and never advances past page 0.** Same finding as the frontend-render-perf assessment (TMAIL-263, findings 1 + 4): the SPA hardcodes `useMessages(folder, 0, 50)` via `useCurrentMessages()` (`useMailbox.ts:78–81`), and `MessageList.tsx:203–210` renders every row into the DOM. A user with 200 messages in Inbox sees only the 50 newest, with no "load more" button. With virtualisation + pagination the same component scales to 50 k rows. | Medium (correctness — silently truncates large folders) | Medium — `useInfiniteQuery` + `useVirtualizer` (both already in `package.json`) | Already filed as TMAIL-263 findings 1 + 4; this assessment confirms |
| 5 | **`get_message` re-fetches the entire `BODY[]` every time the user clicks a thread.** `imap_service.rs:603–610` does `UID FETCH … (FLAGS BODY[] ENVELOPE)`, then `mailparse::parse_mail(body_bytes)` builds the whole MIME tree, then `extract_parts` recurses through it and for every attachment subpart calls `mail.get_body_raw()` purely to measure `body.len()` (`imap_service.rs:747`). The size is already known on the IMAP envelope — and BODYSTRUCTURE would return part sizes without ever transferring the bytes. For a message with a 10 MB attachment we pay (10 MB transfer + 10 MB allocation + 10 MB copy) just to render the chip "report.pdf (9 MB)". | Medium (memory + bandwidth, painful on Ghanaian mobile networks) | Medium — first fetch `BODYSTRUCTURE` + `BODY[HEADER]` + `BODY[1]` (the first text part); fetch attachment bodies on-demand from a separate endpoint that the chip click already needs anyway | New |
| 6 | **`MessageView` makes 2–3 round trips per open.** Opening a message fires (a) `GET /api/folders/:f/messages/:uid` (the IMAP fetch above), (b) `GET /api/folders/:f/messages/:uid/phishing` (`MessageView.tsx:68–73`), (c) on first open also `POST … /phishing` to trigger a scan (`MessageView.tsx:76–85`). Each is a separate fetch. The phishing scan is also unconditional on first view — even for known-good senders / SPF+DKIM+DMARC-pass mail. | Medium (3× the per-open latency, 2× backend connection count) | Low — return phishing report inline in `FullMessage` (or at least bundle them via a single `GET …/messages/:uid?include=phishing,comments,summary` endpoint); only auto-scan when the sender is not in the user's contacts or the SPF/DKIM/DMARC headers don't all pass | New |
| 7 | **Folder names are hardcoded strings everywhere — the `FOLDER_*` registry is dead code.** `frontend/src/utils/constants.ts:7–11` defines `FOLDER_INBOX`, `FOLDER_SENT`, `FOLDER_DRAFTS`, `FOLDER_TRASH`, `FOLDER_SPAM` — but `grep -rn` finds **zero** consumers outside `constants.ts` itself. `mailStore.ts:51` says `selectedFolder: 'INBOX'` raw; `FolderTree.tsx:11–17` has its own `FOLDER_ICONS: Record<string, …>` registry; backend `imap_service.rs:439,467,518` and `messages.rs:261` hardcode `"Trash"`, `"Drafts"`, `"INBOX"` directly. Result: when a Gmail BYOK user's special-use folders are `[Gmail]/Trash` / `[Gmail]/Sent Mail` (very common), `delete_message` silently does the wrong thing — it tries to COPY to literal `"Trash"`, the server returns NO, and the user sees a delete that didn't move anything. | High (correctness for non-Dovecot BYOK servers — Gmail, Outlook, ProtonMail Bridge all use non-standard special-use names) | Medium — define a typed `SpecialFolder` enum on the backend, resolve it on connect via IMAP `LIST … RETURN (SPECIAL-USE)` (RFC 6154), cache the resolved mapping per mailbox, expose it on `GET /api/folders` so the SPA gets the resolved names too | New (P0 for Gmail/Outlook BYOK users) |
| 8 | **`extract_parts` allocates the full byte string of every attachment just to compute `body.len()`.** `imap_service.rs:747–752`: `let body = mail.get_body_raw().unwrap_or_default(); attachments.push(Attachment { …, size: body.len(), … })`. The body bytes are then dropped. For a 10-attachment message that's 10 full base64 decodes that nobody reads. `mailparse::ParsedMail::ctype.params` includes `Content-Length` when present, or we can compute from `mail.raw_bytes.len()` (still allocates but skips base64), or — better — `BODYSTRUCTURE` returns part sizes server-side. | Low–Medium (CPU + transient allocation) | Low — replace with `mail.raw_bytes.len()` short-term; integrate with finding 5 long-term | New |
| 9 | **`flag_message` and friends round-trip a webhook dispatch synchronously-looking but spawned.** `messages.rs:20–25,304–310,339–349,379–390` — `fire_webhook` does `tokio::spawn` so it doesn't block the response. Good pattern. **Positive baseline — keep doing this.** | Positive baseline | — | — |
| 10 | **`refetchOnWindowFocus: false` is set globally.** `App.tsx:43`. Combined with `staleTime: 15_000` on messages (`useMailbox.ts:65`), the message list does **not** refetch on every tab focus — which avoids the "Gmail-tab thrash" anti-pattern. **Positive baseline.** | Positive baseline | — | — |
| 11 | **`get_message` body parse is fully synchronous (CPU on the async runtime).** `mailparse::parse_mail` (`imap_service.rs:619`) is a sync, CPU-bound call running on Tokio's worker thread. For small (<100 KB) messages this is negligible. For a 10 MB HTML newsletter it can stall an entire worker thread for tens of ms. | Low (rare in practice; matters under load) | Low — `tokio::task::spawn_blocking(move \|\| mailparse::parse_mail(&body_bytes))` for messages above a size threshold | New (low priority) |
| 12 | **`MessageRow`, `ThreadRow`, `FolderItem` are not `React.memo`'d.** Same finding as TMAIL-263. Not duplicated as a new ticket. | — | — | Covered by TMAIL-263 |

---

## 1. `ImapService::for_user` caching — what is and isn't cached

This was the first of the explicit checks in TMAIL-243.

**Cached (since TMAIL-162):** the per-user `ImapConfiguration` row.
`imap_service.rs:83–128` reads `state.cache.get_user_imap_config()` (Redis,
5-min TTL — `cache_service.rs:28–32`) before falling through to
`ImapConfiguration::default_for_user(&state.db, user_id)`. On a Redis hit this saves
one row lookup + AES-256-GCM password decryption. The cached payload is the encrypted
ciphertext; the plaintext password is never cached. Invalidation is wired into the
`imap_configurations` write handlers (verified in
`backend/src/handlers/imap_config.rs`).

**Not cached:** the live IMAP `Session` itself. Every call to
`list_folders`, `list_messages`, `get_message`, `search_messages`,
`move_message`, `delete_message`, `set_flag`, `save_draft`, and `get_quota`
goes through `connect()` (`imap_service.rs:156–177`) — `TcpStream::connect` →
`async-native-tls` handshake → IMAP `LOGIN` — followed by an unconditional
`session.logout()` at the end of the handler. That is one full TCP + TLS + LOGIN
+ SELECT cycle per HTTP request.

Cost per session open against typical providers:

| Provider | RTT to server | TCP+TLS handshake | LOGIN+CAPABILITY | Total per request floor |
|----------|---------------|-------------------|------------------|-------------------------|
| Local Dovecot (loopback) | <1 ms | ~5 ms | ~5 ms | ~10 ms |
| Gmail (`imap.gmail.com`) from GH | 180 ms | 400 ms (2 RTT) | 200 ms | **~600 ms** |
| Outlook 365 from GH | 150 ms | 350 ms | 200 ms | **~500 ms** |
| Zoho (`imap.zoho.com`) from GH | 120 ms | 280 ms | 150 ms | **~400 ms** |

For the BYOK target customer base (Gmail, Outlook, Zoho dominate per
`BYOK signup + onboarding` in the project CLAUDE.md), every Inbox open pays the
500 ms floor twice — once for `GET /api/folders`, once for
`GET /api/folders/INBOX/messages` — before any real work happens. Opening a
single message is another 500 ms session open on top of the per-message FETCH.

### Recommendation

Add a `MailboxConnectionPool { mailbox_id → Arc<Mutex<Session>> }` to `AppState`.
Keep at most one session per mailbox (IMAP is stateful around `SELECT`, so
sharing a session across two simultaneous handlers requires serialising
SELECT-changing calls anyway — easiest with a `Mutex`). Idle-timeout sessions
after 60 s — most providers drop idle IMAP connections at 5–30 min anyway, and a
60 s reuse window already amortises ~95% of the handshake cost in a normal
session (most user actions cluster within seconds).

Sibling tickets: TMAIL-158 (SMTP send) and TMAIL-162 (IMAP config row caching)
established the cache layer; this is the natural next step.

---

## 2. Pagination and sorting

`list_messages` (`imap_service.rs:225–303`) takes `(page, page_size)` from the
query string (`messages.rs:60–95`), caps `page_size` at 200
(`messages.rs:69`, `validation::validate_folder_name` enforces folder validity at
`messages.rs:67`), then computes:

```rust
let end   = total.saturating_sub(page * page_size);
let start = end.saturating_sub(page_size).max(1);
// FETCH start:end (UID ENVELOPE FLAGS RFC822.SIZE)
envelopes.reverse();  // newest first
```

**What this gets right:** `page_size` is bounded; the range is server-side; we
do not fetch the body for the list view; the response is `{ messages, total,
page, page_size }` so the SPA could implement infinite-scroll without a server
change (it currently doesn't — see finding 4).

**What this gets wrong:**

1. **Sequence-number drift on concurrent change.** Sequence numbers (`start:end`)
   are reassigned whenever a message is expunged or appears. If the user is on
   page 1 and a new message arrives in the source folder during a refresh,
   sequence number 51 is now a different message than it was a request ago.
   The fix is `UID SEARCH ALL` + slice the UID list, or `SORT REVERSE ARRIVAL`
   (RFC 5256) when the server supports it.

2. **No `(folder, internaldate)` index** — and there is **no DB-side message
   metadata cache** to put one on. `grep -rn "internaldate"
   backend/migrations/*.sql` returns zero matches; the only folder/uid indexes
   live on metadata side tables (`email_comments`, `phishing_reports`,
   `email_tasks`) and look like
   `(mailbox_id, folder, message_uid)`. This is consistent with the architecture
   stated in CLAUDE.md ("IMAP is the source of truth for mail data — the backend
   proxies to Dovecot via async-imap, not a local DB cache"). The trade-off
   is real: every list view is bounded by IMAP latency, not by index lookup.
   For TASMail's BYOK product positioning that's acceptable; for the
   high-volume tier the project will eventually want a thin `(mailbox_id,
   folder, internaldate, uid, envelope_json)` mirror table populated by the
   IDLE-driven email scheduler (`backend/src/services/email_scheduler.rs`) so
   list views can serve out of Postgres without the IMAP round trip. That's a
   bigger conversation than this assessment.

---

## 3. N+1 in `MessageView`

The check from TMAIL-243 was "N+1 risk in MessageView when fetching headers +
body + flags?". The answer splits in two:

**Backend `get_message`:** no N+1. It's a single `UID FETCH … (FLAGS BODY[]
ENVELOPE)` (`imap_service.rs:603–610`) — flags, envelope, and full body all
come back in one round trip. **Positive baseline.**

**Frontend `MessageView`:** yes, finding 6 above. Opening a message fires
sequentially:

| Request | Initiator | Notes |
|---------|-----------|-------|
| `GET /api/folders/:f/messages/:uid` | `useCurrentMessage()` → `useMessage()` (`useMailbox.ts:70–76`) | The actual mail body |
| `GET /api/folders/:f/messages/:uid/phishing` | `phishingQuery` (`MessageView.tsx:68–73`) | `staleTime: 60_000` so it caches, but fires on first view |
| `POST /api/folders/:f/messages/:uid/phishing` | `scanMut` triggered by the user clicking "Scan for phishing" — currently a button, not auto | **OK as-is** (lazy) |
| `GET /api/folders/:f/messages/:uid/comments` | `CommentThread` (TMAIL-128) | Separate component fetch |
| `GET /api/ai/email-summary/:uid` (or similar) | `EmailSummary` (TMAIL-103) | Separate fetch |
| `GET /api/messages/:uid/smart-reply` | `SmartReplyBar` (TMAIL-104) | Separate fetch |

That's 4–6 sequential round trips per message open. Each one spends another
~500 ms in `connect()` if it touches IMAP (finding 1 makes this worse).

The right fix is a **composed endpoint** — `GET
/api/folders/:f/messages/:uid?include=phishing,comments,summary,smart_reply` —
that returns one JSON envelope containing the message plus optional sub-objects
the client requested. Lazy components that are off-screen by default
(`SmartReplyBar` only renders inside the message body) can still fetch on
demand, but the always-rendered ones (phishing banner, comment count) should
ship inline.

---

## 4. Frontend `MessageList` — virtualisation, memoisation, refetch

The full audit is in `docs/assessments/frontend-render-perf-2026-05.md`
(TMAIL-263); the short version for TMAIL-243:

- **Not virtualised.** `MessageList.tsx:202–211` renders every row.
  `@tanstack/react-virtual` is already a dep.
- **Not memoised at the row level.** `MessageRow`/`ThreadRow` are plain
  function components; every parent re-render walks every row.
- **Does not refetch on window focus.** `App.tsx:43` sets
  `refetchOnWindowFocus: false` globally. **Good — keep.**
- **Does not refetch on interval.** `useMessages` has only `staleTime: 15_000`
  (`useMailbox.ts:65`), no `refetchInterval`. Live updates land via the
  WebSocket (`useWebSocket` hook) invalidating the `['messages']` query key.
  Acceptable pattern.
- **The thread groupBy memo re-runs every IMAP poll.** `MessageList.tsx:161–164`
  keys `useMemo` on `data?.messages`, which TanStack Query returns as a fresh
  array reference on every refetch even when contents are unchanged. Pull the
  key down to `data?.messages?.length` + first-uid as a cheap stability
  fingerprint, or hash the UID list.
- **Page 0 only.** `useCurrentMessages()` is called with no args
  (`useMailbox.ts:78–81`) so it's stuck on page 0 / 50 messages forever. Same
  finding as TMAIL-263 finding 4.

---

## 5. Folder names: hardcoded strings vs registry

This is the most actionable correctness finding in the assessment.

**Frontend:**

```
$ grep -rn 'FOLDER_INBOX\|FOLDER_SENT\|FOLDER_TRASH\|FOLDER_DRAFTS\|FOLDER_SPAM' frontend/src
frontend/src/utils/constants.ts: (5 definitions)
(no other matches)
```

The constants in `frontend/src/utils/constants.ts:7–11` are **dead code** —
defined, never imported anywhere else. Every actual consumer hardcodes the
strings:

- `mailStore.ts:51` — `selectedFolder: 'INBOX'`
- `FolderTree.tsx:11–17` — separate `FOLDER_ICONS` registry keyed by string
- `MessageView.tsx` move-to dialog uses raw `prompt('Move to folder:')`
  (`MessageView.tsx:170`) — no validation, no autocomplete, no preset choices

**Backend:**

- `imap_service.rs:439` — `if folder == "Trash"` (delete branch)
- `imap_service.rs:467` — `self.move_message(…, "Trash")` (move-to-trash on delete)
- `imap_service.rs:518` — `session.append("Drafts", …)` (save draft)
- `messages.rs:261` — `unwrap_or("INBOX")` (search default folder)
- 30+ further matches across `models/sync.rs`, `models/spam.rs`,
  `models/email_task.rs`, `models/nlp_search.rs`, `models/calendar_event.rs`,
  `models/retention_policy.rs`, `models/deliverability.rs` — mostly tests, but
  the production code does default-to-INBOX in several places

**Why this matters:** the BYOK feature explicitly targets Gmail, Outlook,
ProtonMail Bridge, FastMail (CLAUDE.md "BYOK signup + onboarding"). Gmail's
special-use folders are not the IMAP defaults:

| Logical folder | Dovecot default | Gmail | Outlook 365 | ProtonMail Bridge |
|---|---|---|---|---|
| Inbox | `INBOX` | `INBOX` | `Inbox` | `INBOX` |
| Sent | `Sent` | `[Gmail]/Sent Mail` | `Sent Items` | `Sent` |
| Drafts | `Drafts` | `[Gmail]/Drafts` | `Drafts` | `Drafts` |
| Trash | `Trash` | `[Gmail]/Trash` | `Deleted Items` | `Trash` |
| Spam | `Junk` | `[Gmail]/Spam` | `Junk Email` | `Spam` |

With the code as it stands, a Gmail BYOK user deleting an Inbox message
triggers `move_message(folder, uid, "Trash")` → IMAP `COPY uid TO Trash` →
server replies `NO Mailbox does not exist`. We surface that as a 500 IMAP error
and the user's message stays put. Save-draft has the same shape against Gmail
(`APPEND Drafts` → NO).

**Fix sketch:**

1. Resolve special-use folders on first connect via IMAP `LIST "" "*" RETURN
   (SPECIAL-USE)` (RFC 6154). Gmail, Outlook, Zoho all advertise this. Dovecot
   too. Cache the resolved `SpecialFolderMap { inbox, sent, drafts, trash,
   junk }` in Redis next to the IMAP config (5-min TTL is fine).
2. Pass the resolved trash/drafts names into `delete_message` and `save_draft`
   instead of hardcoding the string.
3. Expose the map on `GET /api/folders` (extend `Folder` with an optional
   `special_use: Option<"trash"|"sent"|"drafts"|"junk"|"inbox">` field).
4. Frontend: replace `FOLDER_ICONS` in `FolderTree.tsx` with a lookup on
   `folder.special_use`. Replace the `prompt()` move dialog with a proper menu
   driven by the folder list.
5. Promote `frontend/src/utils/constants.ts:7–11` to a typed enum
   `SpecialFolder = "inbox"|"sent"|"drafts"|"trash"|"junk"` and consume it on
   both sides of the wire. Or — alternatively — delete the dead constants and
   make special-use the only public surface.

Counted as **P0 correctness** for the BYOK story, even though it has been
shipping broken against Gmail since the BYOK pivot.

---

## 6. The validation layer — what catches what

`validation::validate_folder_name` (`backend/src/validation.rs:142`) does the
obvious safety check: rejects empty, rejects CR/LF (IMAP injection). The unit
test at line 327 specifically covers `"INBOX\r\nLOGOUT"` — good. It does
**not** validate against the user's actual folder list (i.e. a request for
`"NonExistentFolder"` falls through to IMAP SELECT and returns an IMAP error
500). That's fine for now — adding a folder-list pre-check would just double
the IMAP round trips.

---

## 7. Quota / `get_quota` — the worst-case path

Included here because it lives in `imap_service.rs:528–586` and shares the
connection-per-request problem.

```
for every folder:
    STATUS folder (MESSAGES)
    if exists > 0:
        SELECT folder
        FETCH 1:{exists} RFC822.SIZE        # ← N FETCHes per folder
```

A mailbox with 8 folders × 5 000 messages = **40 000 individual FETCHes**
behind one HTTP request, all serialised over a single IMAP session. Against
local Dovecot that's ~20 s; against Gmail it's a quota-rate-limit violation
within the first few seconds. The 60 s Redis quota cache
(`cache_service.rs:139–155`) covers steady state, but every cache miss takes
the full hit.

Better approaches in priority order:

1. `GETQUOTA "" / GETQUOTAROOT INBOX` (RFC 2087) — Dovecot, Gmail, Zoho,
   FastMail all support this. One round trip, returns
   `(STORAGE used limit)`. Fall back to (2) if the server replies `NO`.
2. `STATUS folder (X-SIZE)` (Dovecot extension) — sums the folder bytes
   server-side. One round trip per folder, no per-message FETCH.
3. The current scan — kept only as a last-resort fallback, with the same 60 s
   cache.

This is also the worst single connection in the codebase. Combined with
finding 1's connection pool, it becomes much less painful — but the algorithm
itself is the underlying issue.

---

## 8. Suggested follow-up tickets

In priority order:

| # | Ticket | Estimate |
|---|--------|----------|
| 1 | **Special-use folder resolution + typed `SpecialFolder` enum** (finding 7) — P0 correctness for Gmail/Outlook BYOK | 1–2 days |
| 2 | **IMAP `MailboxConnectionPool`** (finding 1) — biggest single perf win | 1 day |
| 3 | **`GETQUOTA` with current-scan fallback** (finding 2, 7) | half a day |
| 4 | **Composed `GET …/messages/:uid?include=…` endpoint** (finding 6) — collapses 3+ round trips per message open | 1 day |
| 5 | **`BODYSTRUCTURE`-driven attachment metadata** (findings 5 + 8) — stops transferring attachment bytes just to populate chips | half a day |
| 6 | **`UID SEARCH ALL` or `SORT REVERSE ARRIVAL` for pagination** (finding 3) — fix shifting-window bug | half a day |
| 7 | **`MessageList` virtualisation + infinite scroll** — already covered by TMAIL-263; called out here so it doesn't drop off the radar | 1 day |
| 8 | **`spawn_blocking` for >100 KB `mailparse`** (finding 11) — low priority, do when load testing shows worker stalls | quarter day |

**Out of scope for TMAIL-243 but worth noting** for a future ticket: the
`(mailbox_id, folder, internaldate, uid, envelope_json)` mirror table driven
by the IDLE scheduler. That's the real path to sub-100ms list views and is
prerequisite for any future search-without-IMAP-SEARCH story.

---

## Appendix A — files read

Backend:

- `backend/src/handlers/folders.rs` (27 lines)
- `backend/src/handlers/messages.rs` (583 lines)
- `backend/src/services/imap_service.rs` (1030 lines)
- `backend/src/services/cache_service.rs` (506 lines)
- `backend/src/validation.rs` (partial — folder validation only)
- `backend/migrations/*.sql` (grep for `internaldate`, folder indexes)

Frontend:

- `frontend/src/components/mail/MessageList.tsx` (214 lines)
- `frontend/src/components/mail/MessageView.tsx` (310 lines)
- `frontend/src/components/mail/FolderTree.tsx` (84 lines)
- `frontend/src/hooks/useMailbox.ts` (120 lines)
- `frontend/src/stores/mailStore.ts` (66 lines)
- `frontend/src/api/folders.ts` (5 lines)
- `frontend/src/api/messages.ts` (107 lines)
- `frontend/src/utils/constants.ts` (29 lines)
- `frontend/src/App.tsx` (queryClient config only)

## Appendix B — what was NOT covered

- Live IMAP latency measurements — no production-data dev mailbox available
  during the audit. All RTT numbers are reference figures, not measured.
- `email_scheduler` background service — referenced as the future home of the
  metadata mirror table, but not audited.
- WebSocket push path (`useWebSocket`, `services/push_service.rs`) — out of
  scope; only the read-path query keys it invalidates were noted.
- Mobile app (`mobile/lib/`) — has its own surface (`/api/mobile/*`) covered
  by `docs/assessments/mobile-sync-push-2026-05.md`.
