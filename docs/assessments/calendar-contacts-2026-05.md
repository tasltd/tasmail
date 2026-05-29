# Calendar & Contacts Assessment

- **Issue:** TMAIL-247 (axis of TMAIL-241)
- **Date:** 2026-05-29
- **Scope (backend):** `backend/src/handlers/calendar.rs`,
  `backend/src/handlers/public_calendar.rs`,
  `backend/src/handlers/contacts.rs`, `backend/src/handlers/contact_groups.rs`,
  `backend/src/handlers/groups.rs`, `backend/src/handlers/dav_config.rs`,
  `backend/src/services/ics_generator.rs`,
  `backend/src/services/caldav_freebusy.rs`,
  `backend/src/services/imip_parser.rs`,
  `backend/src/services/slot_suggester.rs`,
  `backend/src/services/vcard_service.rs`,
  `backend/src/models/calendar_event.rs`, `backend/src/models/contact.rs`,
  `backend/src/models/contact_group.rs`, `backend/src/models/dav_config.rs`,
  `backend/src/models/distribution_group.rs`,
  `backend/migrations/002_signatures_and_contacts.sql`,
  `backend/migrations/031_calendar_events.sql`,
  `backend/migrations/043_contact_groups.sql`,
  `backend/migrations/048_caldav_config.sql`,
  `backend/migrations/071_calendar_public_scheduling.sql`,
  `backend/migrations/072_calendar_events_per_organizer_ics_uid.sql`
- **Scope (frontend):** `frontend/src/components/settings/CalendarManager.tsx`,
  `frontend/src/components/settings/CalendarView.tsx`,
  `frontend/src/components/settings/ContactsApp.tsx`,
  `frontend/src/components/settings/ContactManager.tsx`,
  `frontend/src/components/settings/GroupManager.tsx`,
  `frontend/src/components/mail/SuggestSlotsPanel.tsx`,
  `frontend/src/api/calendar.ts`, `frontend/src/api/contacts.ts`,
  `frontend/src/api/contact-groups.ts`, `frontend/src/api/groups.ts`,
  `frontend/src/utils/calendar-helpers.ts`,
  `themes/shadcn-prototype/src/features/calendar/CalendarView.tsx`
- **Method:** Static read of every file in scope, grep across handlers + models
  for RRULE / recurrence handling, migration sweep for indexes and unique
  constraints, line-count audit against the modularisation 250-LOC guideline.
  No load test or live IMAP/SMTP capture — performance figures are reasoned
  from the round-trip count and SQL shape, not measured.

---

## TL;DR — biggest wins, by ROI

| # | Finding | Impact | Effort | Suggested ticket |
|---|---------|--------|--------|------------------|
| 1 | **`recurrence_rule` is a string pass-through; nothing expands it.** Backend stores `recurrence_rule: Option<String>` (`models/calendar_event.rs:19`), accepts arbitrary text in the request body, persists it to TEXT, and never reads it again. `ics_generator.rs:85–138` does **not** emit `RRULE:` in the ICS output, so even invitations exported from TASMail look like one-off events to Google/Outlook/Apple Calendar. `services/imip_parser.rs:15` explicitly excludes RRULE on the inbound side ("Anything more exotic … is preserved as raw text on the event but not interpreted here"). The frontend admits the gap too — `CalendarView.tsx:1–6` says the @fullcalendar/rrule plugin is bypassed due to an ESM interop bug, and line 43 forces every event to render once at its master `start_time`. **Net result:** a user setting up a "weekly standup" sees it on the first Monday and never again, and the iMIP invite they send to colleagues doesn't recur in their calendars either. This is a feature gap, not a perf bug — but it's the headline answer to TMAIL-247's "typed parsing or string pass-through?" question. | **High** (correctness — recurring events advertised but broken end-to-end) | Large — pick `rrule@^2.8` (or migrate to `rrule-rust`), add expansion in `list_for_user`/`list_events`, emit `RRULE` in `generate_ics`, parse in `imip_parser`. Fix the FullCalendar plugin ESM at the same time | New — child of TMAIL-247 |
| 2 | **iMIP invite fan-out is one full SMTP session per attendee, serial.** `handlers/calendar.rs:204–224` — `for attendee in attendees { smtp_service.send_imip_request(…).await }`. Each call dials the user's BYO-SMTP server, does STARTTLS+AUTH, sends one message, then closes. 50 attendees → 50 TCP+TLS+AUTH handshakes, ~500 ms each on a typical hosted SMTP → **~25 s** for one event creation against Gmail SMTP, plus a near-certain rate-limit hit (Gmail SMTP submission caps at 100/day for free, 2 000/day paid — and 50 messages in 25 s is also above the per-second throttle). Same shape repeats for the iMIP REPLY path (`send_imip_reply_for_user`, `:618`) but only one recipient so it doesn't compound. | **High** (perf + deliverability — a 20-attendee invite can take 10 s before the response returns to the SPA, and many SMTP providers will start refusing) | Medium — single SMTP session per event (lettre supports keep-alive); send all attendee envelopes through one connection. Better still, hand off to the existing `email_queue` so it never blocks the response. The queue path also gives free retries on transient failures | New |
| 3 | **`EventAttendee::create_bulk` is sequential N+1 INSERTs.** `models/calendar_event.rs:336–356` — `for attendee in attendees { INSERT … RETURNING * }`. A 50-attendee event books 50 round-trips against Postgres. For local PG that's ~25 ms; over the workstation's loopback it's lower, but it's still per-attendee. The same `create_bulk` is called by `accept_imip` (`:507`) where every iMIP REQUEST re-inserts the entire attendee list **with no dedupe** — and there's no `UNIQUE (event_id, email)` index (see #4) — so re-accepting the same invite duplicates every attendee row. | Medium (perf + correctness on re-accept) | Low — single multi-row `INSERT … VALUES (…),(…),… ON CONFLICT (event_id, email) DO NOTHING RETURNING *` once the unique index is in place | New |
| 4 | **No `UNIQUE (event_id, email)` on `event_attendees`.** Migration 031:26–33 only creates `idx_event_attendees_event`. The absence is called out in `models/calendar_event.rs:403–405` ("event_attendees doesn't have a unique index on (event_id, email), so we can't rely on ON CONFLICT here") — and the public RSVP upsert (`upsert_public_rsvp`, `:396–448`) compensates with a `SELECT … FOR UPDATE` + manual update-or-insert inside a transaction, which is two extra round trips on every external RSVP. Adding the constraint lets that helper collapse to a one-shot `ON CONFLICT`, and unblocks the batched insert in #3, and makes `accept_imip`'s re-accept path idempotent. | Medium (perf + correctness + simplifies three call sites) | Low — single Flyway migration, plus removing the manual dance | New |
| 5 | **`list_for_user` returns up to 100 events with no offset/pagination, sorted ASCENDING.** `models/calendar_event.rs:144–155` — when start/end are absent (the default call from `CalendarManager.tsx:396`, which uses `listEvents()` with no args), the query is `SELECT * FROM calendar_events WHERE organizer_id = $1 ORDER BY start_time ASC LIMIT 100`. For a power user with 300+ historical events this means the list view shows the **oldest 100**, not the newest. Most users see only events from 2023–early 2024 with no way to scroll forward. The range-bounded path (start+end) has no LIMIT at all and could return thousands of rows for a heavy 14-day window. | **Medium** (correctness — list view points at wrong end of the data) | Low — flip to `ORDER BY start_time DESC`, add `LIMIT/OFFSET`, expose page args on `GET /api/calendar/events` | New |
| 6 | **`accept_imip` opens its own ad-hoc IMAP session, duplicating connect+TLS+LOGIN.** `handlers/calendar.rs:565–612` (`fetch_message_rfc822`) hand-rolls the TCP/TLS/LOGIN dance instead of going through `ImapService::for_user` — the comment says it's "Replicated locally rather than reusing imap_service.get_message because that helper decodes MIME parts; we need the bytes intact". That's a valid reason to need a different fetch *spec* — `RFC822` instead of decoded MIME — but it's not a reason to bypass the service entirely. The duplicated `connect()` will eventually drift from the shared one (e.g. it skips the `SmartFolders` resolution from `folders-messages-2026-05.md` finding 7, so the implicit `body.folder` strings won't translate for Gmail). | Medium (perf + correctness drift over time) | Small — add `ImapService::fetch_raw_rfc822(folder, uid)` and call it here. Bonus: when the connection pool from TMAIL-243 lands it benefits this path too | New |
| 7 | **`accept_imip` fetches the entire `RFC822` payload just to read the iCalendar part.** `calendar.rs:596` — `session.uid_fetch(uid.to_string(), "RFC822")` pulls the whole message body, then `find_calendar_part` recurses through the parsed MIME tree (`services/imip_parser.rs:53–63`). For a 10 MB invitation with a PDF agenda attachment that's a 10 MB transfer + full base64 decode just to extract ~2 KB of text/calendar. IMAP `BODYSTRUCTURE` lists every part with its content-type and byte offset; fetching `BODY[<part-id>]` only for the `text/calendar` subpart cuts that to a couple of KB. | Medium (memory + bandwidth, especially painful on Ghanaian mobile network test users) | Medium — `BODYSTRUCTURE`, walk for `text/calendar`, partial `BODY[…]` fetch | New |
| 8 | **CalDAV freebusy fan-out is serial.** `handlers/calendar.rs:1086–1148` (`fetch_user_caldav_busy`) — `for cfg in configs { query_caldav_freebusy(…).await }`. With 2–3 configured CalDAV servers (TMAIL-117 explicitly supports multiple — Radicale + Apple iCloud + Fastmail is the documented scenario) and a 15 s timeout each, the worst case is a 45 s blocking free-busy request. Should be `futures::future::join_all` so the 15 s ceiling applies once. | Medium (latency tail for users with multiple DAV servers) | Low — swap loop for `join_all`/`try_join_all`; failures already isolated per server | New |
| 9 | **`list_group_contacts` is N+1.** `handlers/contact_groups.rs:120–143` — `let contact_ids = …list_contact_ids_in_group(…); for cid in contact_ids { Contact::find_by_id(…) }`. For a 500-member contact group that's 501 round trips. Should be one JOIN. | Medium | Low — single SELECT with JOIN through `contact_group_members` | New |
| 10 | **`merge_contacts` is "delete-the-rest", not "merge fields".** `handlers/contact_groups.rs:349–373` keeps the primary row exactly as-is and `Contact::delete`s the secondaries. Any non-null `phone`/`company`/`notes` on the secondaries is lost, and the secondaries' `contact_group_members` rows are CASCADE-removed instead of being re-pointed at the primary. So a user merging two duplicates loses both the richer data and the group memberships of whichever they picked second. The function name promises field-level merge; the behaviour is delete-then-keep. | Medium (data loss in the most common dedupe flow) | Low — replace with a single `UPDATE contacts SET … = COALESCE(primary.col, secondaries.col)` + `UPDATE contact_group_members SET contact_id = primary_id WHERE contact_id = ANY(secondaries) ON CONFLICT DO NOTHING`, then delete the secondaries in one DELETE | New |
| 11 | **CSV import is row-by-row, sequential.** `handlers/contact_groups.rs:244–254` — `for row in rows { Contact::create(…).await }`. A 10 000-row CSV (a single Gmail Takeout) becomes 10 000 sequential INSERTs over the same connection — minutes-long on a real network. Same pattern as #3. Batch into `INSERT … VALUES …(…) ON CONFLICT (mailbox_id, email) DO NOTHING` 500 at a time. The existing unique index on `(mailbox_id, email)` already supports the conflict target — this is purely a code change. | Medium (perf on the BYOK migration story — the marketed "Gmail Takeout import" path) | Low — chunked multi-row INSERT | New |
| 12 | **vCard parser only handles `FN`/`EMAIL`/`TEL`/`ORG` and only keeps the first of each per VCARD.** `services/vcard_service.rs:66–90` — every property assignment is `current_email = Some(value)`, overwriting previous values. Real-world exports (Gmail, Outlook, iCloud) routinely carry `EMAIL;TYPE=WORK:`, `EMAIL;TYPE=HOME:`, multiple `TEL` lines, structured `N:`, `ADR:`, `BDAY:`, `NICKNAME:`, `NOTE:`, `URL:`, `PHOTO:`. None of those survive import — only the *last* email and phone per card, no birthday, no address, no notes, no photo. Roundtrip tests pass only because `export_vcard` emits only what `parse_vcard` keeps. For the BYOK migration story this is real data loss. **Also:** the parser doesn't unfold continuation lines (RFC 6350 §3.2 folding is identical to iCalendar — long `NOTE` and `PHOTO` lines from real exports come folded), so any line >75 chars silently corrupts. | **High** (BYOK story claims "import your existing contacts"; a Gmail export round-trips with data loss) | Medium — switch to a real vCard crate (e.g. `vcard4`), or extend the hand-rolled parser to handle folding + multi-value + N/ADR/NOTE. Either way the model has to grow extra columns (`first_name`, `last_name`, `additional_emails`, `address`, etc.) — also a migration | New (P1 for closed-beta — see `BETA-CUSTOMER-RECRUITMENT-RUNBOOK.md`) |
| 13 | **`dav_configurations` has sync columns that nothing writes.** Migration 048 + `models/dav_config.rs` define `sync_interval_minutes`, `last_sync_at`, `sync_status`, `sync_error`. The handler exposes CRUD + test + a `/sync` route (router.rs:901), but only `fetch_user_caldav_busy` reads CalDAV — and only for free-busy. There is **no background sync scheduler** turning incoming events from Radicale/Apple/Fastmail into rows in `calendar_events`, and no CardDAV sync surfacing them into `contacts`. TMAIL-117 was scoped as full bi-directional DAV sync; the landed implementation is "read-only free-busy lookup against one CalDAV calendar per request". The columns and the `/sync` endpoint advertise a feature that doesn't run. | Medium (advertises a feature that doesn't work; also blocks the BYOK calendar story for users who already keep events in iCloud/Fastmail) | Large — full job, out of scope for this audit. Track separately | New (continuation of TMAIL-117) |
| 14 | **CalDAV sync is full-refresh; no sync-token / RFC 6578 incremental.** Even within the current freebusy-only scope, every request to `query_caldav_freebusy` is a fresh REPORT for the whole date range. RFC 6578 (`{DAV:}sync-collection`) would let us issue a delta query and persist only changes. Pre-requisite for finding #13 anyway. | Low (only an issue once #13 lands) | — | Captured under #13 |
| 15 | **CalDAV password is decrypted per request.** `fetch_user_caldav_busy` calls `decrypt_api_key` for each `dav_configurations` row on every free-busy lookup (`calendar.rs:1117`). Decryption is fast (AES-256-GCM, no PBKDF), but unnecessary — caching the decrypted bytes in a `tokio::sync::Mutex<HashMap<config_id, Arc<String>>>` keyed by config-id + last_updated_at avoids the cost. Only matters once finding 8 lands and we're hitting CalDAV in tight loops. | Low | Low | New (low priority) |
| 16 | **Calendar route is in a 1500-line `handlers/calendar.rs`.** Eleven public handlers + four private helpers + free-busy + slot suggestion + iMIP accept + iMIP invite fan-out, plus 350 lines of tests, all in one file. Per the workspace modularisation rule, split into `handlers/calendar/{events,freebusy,imip}.rs`. | Low (cleanup) | Low | New (modularisation follow-up) |
| 17 | **`ContactsApp.tsx` recomputes `findDuplicates` on every render.** `frontend/src/components/settings/ContactsApp.tsx:113–129,179` — `findDuplicates` is an O(n) pass over `allContacts`, but it's called twice per render (`duplicateCount` on `:179`, and inside `handleMergeAll`) without `useMemo`. For 5 000 contacts that's ~5 ms wasted on every state change. **Note:** TMAIL-247's prompt asked "O(n²) or hashed?" — it's hashed (Map-based linear pass), so the algorithm is fine. The fix is just memoisation. | Low | Trivial | New |
| 18 | **ICS download is buffered, not streamed.** `download_ics` (`calendar.rs:343–393`) returns the full ICS body as a `String` via Axum's `Json`-shape tuple. For a one-shot event with no recurrence that's a few KB — fine. But the same code path will host the future "export entire calendar as `.ics`" feature, and a 10K-event export would currently materialise into a single `String` before sending. Move to `axum::response::Response` with a `tokio_stream::wrappers::ReceiverStream<Bytes>` once recurrence + export-all lands. | Low (only matters once bulk export lands) | Low | Captured under #1 follow-up |
| 19 | **vCard export is buffered too.** `services/vcard_service.rs:97–116` — `export_vcard` builds the whole vCard payload into a `String` then the handler returns it. A 10K-contact `.vcf` is ~1.2 MB — fine for now, but the streaming path is straightforward enough to do alongside #12. | Low | Low | Captured under #12 |
| 20 | **`/api/dav/*` route prefix is `/api/dav/configs`, not `/api/caldav/*`.** The project CLAUDE.md ("API Route Structure" section) lists `/api/caldav/*` as a route group, but the actual router (`backend/src/router.rs:890`) uses `/api/dav/configs[/{id}/{sync,test}]`. Either CLAUDE.md is stale or the router should be renamed. The prefix is irrelevant to behaviour but it's a documentation drift the next person will trip over. | Low (docs) | Trivial | Doc fix only |
| 21 | **`calendar-helpers.ts` is dead code.** `frontend/src/utils/calendar-helpers.ts` defines `getDaysInMonth`, `getWeekDays`, `getHoursOfDay`, `isSameDay`, `formatMonthYear`, `formatWeekdayShort`, `getHourFromIso` — written for the pre-FullCalendar custom grid (TMAIL-118 swapped in FullCalendar). Only consumer left is its own `*.test.ts`. Delete. | Trivial | Trivial | New cleanup |
| 22 | **Positive baselines** — keep these. (a) **FullCalendar is lazy-loaded** behind `React.lazy` + `Suspense` (`CalendarManager.tsx:27,460`) so the ~600 kB chunk only hits the wire when the user clicks Grid mode — answers TMAIL-247's FullCalendar question. (b) **Events ARE paginated to viewport** in Grid mode — `datesSet` re-fetches when the user navigates (`CalendarView.tsx:89–95`). (c) **CalDAV free-busy parser handles every edge** — folding, FBTYPE=FREE skip, comma-separated periods, multistatus XML wrap, LF-only endings (`services/caldav_freebusy.rs:55–98` + 250 lines of tests). (d) **`/api/calendar/free-busy` only fans out CalDAV for the auth user, never for other attendees** — documented privacy decision (`calendar.rs:978–990`). (e) **Public-token RLS policy is defense-in-depth** (migration 071:25–30). (f) **`UNIQUE (organizer_id, ics_uid)` correctly per-organizer, not global** (migration 072) so two mailboxes can accept the same invitation. (g) **`merge_busy` algorithm is correct** — well-tested sorted-and-collapse (`services/slot_suggester.rs` tests). (h) **`busy_intervals_for_organizer` returns only `(start_time, end_time)` tuples** — privacy-preserving by construction (`models/calendar_event.rs:298–309`). (i) **Alt-UI `themes/shadcn-prototype/src/features/calendar/CalendarView.tsx` is actually wired to the real backend** — the project CLAUDE.md still claims it's "still on `mockData.ts`", but the header comment confirms TMAIL-235/236/237 wired it to `/api/calendar/events`. Doc fix only. | Positive baselines | — | — |

---

## 1. RRULE — typed parsing or string pass-through? (TMAIL-247 explicit check)

**Answer: string pass-through, never expanded, broken end-to-end.**

Trace:

```
CreateEventRequest.recurrence_rule: Option<String>      // models/calendar_event.rs:55
  ↓
INSERT INTO calendar_events (… recurrence_rule …)       // :106-122
  ↓
list_for_user → SELECT * FROM calendar_events           // :126-155  (single row)
  ↓
generate_ics(IcsEventData { …status, attendees, ... })  // services/ics_generator.rs:85-138
  ↓                                                    // NO RRULE emitted
text/calendar payload sent over iMIP
```

There are **three** independent ways the system advertises a feature it doesn't ship:

1. **Frontend stores it.** `frontend/src/api/calendar.ts:14` declares `recurrence_rule: string | null` on the response type, and the create form *would* round-trip it through the API. The composer doesn't surface a UI for it yet (`CalendarManager.tsx:62–168` has no RRULE field), so the only writes today are from iMIP accept (which preserves but doesn't parse — see below).

2. **FullCalendar plugin is half-installed.** `package-lock.json` does not include `@fullcalendar/rrule`. `CalendarView.tsx:1–6` admits the ESM bug ("RRule + rrulestr exports come back undefined") and bypasses recurrence rendering. Line 43–46: "Always render as a single non-recurring event for now". So even if the backend expanded RRULE into multiple rows, the grid wouldn't render them.

3. **iMIP parser drops it.** `services/imip_parser.rs:15` explicitly excludes RRULE expansion: "Anything more exotic (RRULE expansion, VTIMEZONE resolution, X-* props) is preserved as raw text on the event but not interpreted here." Re-read carefully — "preserved as raw text on the event" is also a lie: I grepped the parser and **RRULE is not in the `ParsedInvite` struct at all** (`:23–38`), so an inbound recurring invite gets persisted as a single occurrence with the master start/end and the RRULE simply dropped.

### Fix sketch

1. Pull in `rrule@2.8.x` (Rust) — or `rrule-rust` if the maintained-ness becomes an issue — and add `RecurrenceRule::parse(&str)` returning a typed struct.
2. On the read path, in `list_for_user(start, end)`, for any row with `recurrence_rule IS NOT NULL`, expand into the date range server-side and return synthetic events. Cap the expansion at, say, 366 days per range to avoid pathological RRULEs.
3. In `generate_ics`, append `RRULE:<frozen-form>` when present.
4. In `imip_parser`, add `recurrence_rule: Option<String>` to `ParsedInvite` and write it through `accept_imip → upsert_by_ics_uid`.
5. Fix the FullCalendar plugin ESM issue (downgrade to rrule@2.7, or vendor a tiny adapter). This is a known frontend chore — track separately.
6. Add a UI in `EventForm` (`CalendarManager.tsx`) for the common recurrence presets (daily/weekly/biweekly/monthly/yearly) plus a "Custom RRULE…" advanced expander.

This is **the** headline finding of TMAIL-247 — every other item below is a quality-of-life tweak by comparison.

---

## 2. ICS streaming vs in-memory (TMAIL-247 explicit check)

**Answer: in-memory.** Both directions.

* **Export**: `download_ics` (`calendar.rs:343–393`) builds the full ICS in a `String` and returns it through Axum's tuple shape. Fine today (one event, a few KB), but the same handler will host the future "export entire calendar" flow. Switch to `axum::body::Body::from_stream(…)` once bulk export lands.
* **Import (iMIP)**: `accept_imip` issues `UID FETCH … RFC822` which pulls the entire message body into a `Vec<u8>` (`calendar.rs:606–609`), then `mailparse::parse_mail` builds the entire MIME tree in memory (`imip_parser.rs:53–63`), then `find_calendar_part` walks the tree looking for `text/calendar`. The whole pipeline is in-memory and synchronous (`mailparse` is sync — runs on a Tokio worker thread). For a normal invite (~10 KB) this is fine; for an invitation with a 10 MB PDF attachment it transfers and allocates 10 MB just to read 2 KB of iCalendar. See finding 7 above for the targeted `BODYSTRUCTURE` fix.

vCard import/export has the same shape (`services/vcard_service.rs:14–116`) — both `parse_vcard` and `export_vcard` materialise the whole string. The bigger issue with vCard parsing is missing fields (finding 12), not memory.

Net: **everything is in-memory**, which is acceptable for the volumes we expect during closed-beta (≤ 10 customers, single-digit events per day per user) but doesn't scale to the "import a 10 000-contact Gmail Takeout" story we sell in `BUSINESS-VALIDATION-GHANA.md`.

---

## 3. Contact dedup logic — O(n²) or hashed? (TMAIL-247 explicit check)

**Answer: hashed (linear), but not memoised.**

`ContactsApp.tsx:113–129`:

```ts
const findDuplicates = (): Map<string, Contact[]> => {
  const emailMap = new Map<string, Contact[]>();
  for (const c of allContacts) {
    const key = c.email.toLowerCase();
    const existing = emailMap.get(key) || [];
    existing.push(c);
    emailMap.set(key, existing);
  }
  // …
}
```

That's a single linear pass — O(n) with a hashed lookup, not O(n²). The actual problem is that the function is called on every render of `ContactsApp`:

* `:179` — `const duplicateCount = findDuplicates().size;` runs unconditionally each render.
* `:172` — `handleMergeAll` also calls it.

For 5 000 contacts a 5–10 ms pass per render is a real waste. Wrap in `useMemo([allContacts])` and the cost drops to once per data change.

There's also a **server-side dedupe finding** worth raising separately: `merge_contacts` is "delete the secondaries, don't merge fields" — see #10 above. The user-facing flow (`handleMergeAll` at `:171–177`) iterates the duplicates map and calls `mergeMutation.mutate(ids)` for each cluster — meaning N parallel mutations fired without coordination. TanStack Query's default mutation isn't deduped — they all hit the backend at once. Fine for a 3-cluster merge; bad for a 200-cluster merge. Worth a `for await` or a batched endpoint.

---

## 4. FullCalendar (TMAIL-247 explicit check)

**Lazy-loaded? Yes. Paginated to viewport? Yes.** Positive baseline both ways.

`CalendarManager.tsx:27` — `const CalendarView = lazy(() => import('./CalendarView').then(...))`. The `@fullcalendar/core` + `daygrid` + `timegrid` + `list` + `interaction` chunk only hits the wire when the user clicks the Grid toggle. Confirmed against the bundle audit (`docs/assessments/frontend-bundle-2026-05.md`).

`CalendarView.tsx:89–95` — `datesSet` handler re-runs the query whenever FullCalendar swaps the visible range (view change, prev/next, today). The `useQuery` key includes `rangeRef.current.{start,end}` so React Query naturally caches per-range. Initial range is the current month, matching what FullCalendar will render on mount.

What's **missing** is a join of these two query keys — the list-mode CalendarManager uses `['calendar-events']` (no args), the grid mode uses `['calendar-events-view', start, end]`. Both fetch the same backend endpoint but the cache is split. The `rescheduleMut` callback invalidates both (`CalendarManager.tsx:418–420`), but `createMut` / `cancelMut` invalidate only `['calendar-events']` — so dragging a new event onto the grid and then clicking back to list shows stale data until the page reloads. Fix: every event-mutating mutation should invalidate both keys, or use a shared key prefix.

---

## 5. CalDAV sync: token-based incremental or full-refresh? (TMAIL-247 explicit check)

**Answer: full-refresh — and only for free-busy.**

There is **no sync at all** for events or contacts. `dav_configurations` has every column the schema needs (`sync_interval_minutes`, `last_sync_at`, `sync_status`, `sync_error` — migration 048), plus a `/api/dav/configs/{id}/sync` route (router.rs:901), but no background job populates `calendar_events`/`contacts` from a configured DAV server. The only DAV read in the codebase is `query_caldav_freebusy`, which is a one-shot REPORT executed inline on every `/api/calendar/free-busy` lookup.

Even within that one-shot use, the REPORT covers the entire requested date range every time — no `{DAV:}sync-token` / RFC 6578 incremental sync. Pre-requisite for any real sync flow anyway.

This is the most significant gap for the BYOK story alongside RRULE: the marketing materials in `BUSINESS-VALIDATION-GHANA.md` and the BETA recruitment runbook both lean on "bring your existing iCloud / Fastmail calendar" — but the only thing TASMail actually does with that calendar today is consult its free-busy when scheduling. Events on iCloud don't appear in the user's TASMail calendar view.

---

## 6. Indexes, constraints, RLS — what catches what

| Table | Indexes | Constraints | RLS | Notes |
|---|---|---|---|---|
| `contacts` | `(mailbox_id)`, `(email)`, `UNIQUE (mailbox_id, email)` | unique composite | (RLS not enabled per migration 002) | The standalone `(email)` index is redundant — tenant-scoped queries never use it; drop. RLS is missing on `contacts` — every other tenant-scoped table has it; add. |
| `contact_groups` | `(user_id)` | — | yes — `user_id = current_setting('app.current_user_id')::uuid` | Good. |
| `contact_group_members` | `(contact_id)`, PRIMARY KEY `(contact_group_id, contact_id)` | composite PK | inherits via FK CASCADE | Good. |
| `calendar_events` | `(organizer_id)`, `(start_time, end_time)` | `UNIQUE (organizer_id, ics_uid)` (migration 072), `UNIQUE (public_token)` (migration 071) | yes — extended for public-enabled rows (migration 071) | The frequent query is `WHERE organizer_id = $1 AND start_time < $3 AND end_time > $2` (`busy_intervals_for_organizer`); a composite `(organizer_id, start_time, end_time)` would beat the two separate indexes. Add. |
| `event_attendees` | `(event_id)` | — | inherits via FK CASCADE | **Missing**: `UNIQUE (event_id, email)` — see finding 4. |
| `dav_configurations` | `(user_id)` | CHECK on `dav_type` + `sync_status` (TEXT-with-CHECK pattern) | yes — `user_id = current_setting('app.current_user_id')::uuid` | Good. |

**Action items (P2):**
- Drop redundant `idx_contacts_email`.
- Enable RLS on `contacts` (and re-issue the `idx_contacts_mailbox_email` covering policy).
- Add composite `(organizer_id, start_time, end_time)` on `calendar_events`.
- Add `UNIQUE (event_id, email)` on `event_attendees` (finding 4).

---

## 7. Static types — Rust

* `recurrence_rule: Option<String>` — see finding 1; should be a typed `RecurrenceRule` struct once expansion lands.
* `partstat: &str` in `generate_imip_reply` (`ics_generator.rs:152–162`) — the caller passes "ACCEPTED" / "DECLINED" / "TENTATIVE" as a stringly-typed value; should be `enum Partstat { Accepted, Declined, Tentative, NeedsAction }`. Same for `status: String` on `CalendarEvent` (the values are bounded to `'tentative' | 'confirmed' | 'cancelled'` by the migration 065 CHECK; surface that as an enum).
* `DavType` already uses a serde-rename'd enum (`models/dav_config.rs:14–22`) — good baseline; the calendar status fields haven't followed suit yet.
* `imip_parser::ParsedInvite.method` is a `String`; it's compared via `to_ascii_uppercase().as_str()` against `"CANCEL" | "REQUEST" | "PUBLISH"` (`calendar.rs:469–473`). Enum.
* `accept_imip`'s `partstat` ladder (`calendar.rs:433–443`) maps strings to strings via `match`. Both ends are bounded; both should be enums.

These are all small, low-risk cleanups but they pay off the next time someone touches a calendar handler — string-to-string conversions hide bugs (e.g. `"Maybe"` vs `"maybe"`, `"tentative"` vs `"TENTATIVE"`).

---

## 8. Static types — TypeScript / Dart

* `CalendarEvent.status` in `frontend/src/api/calendar.ts:15` is `string`, not a literal union. Should be `'tentative' | 'confirmed' | 'cancelled'`. Same for `EventAttendee.rsvp` (`:33`) — should be `'pending' | 'accepted' | 'declined' | 'maybe'`. The frontend already uses these as record keys in `STATUS_COLORS` / `RSVP_COLORS` (`CalendarManager.tsx:29–42`), so a TS error would catch any missing case at compile time.
* `RsvpRequest.status` (`calendar.ts:68`) is already the typed literal union — good baseline; just propagate the same shape to read paths.
* `AttendeeBusy.status` (`calendar.ts:124`) is the literal `'resolved' | 'not_resolved'` — good baseline.
* `Contact.email`/`display_name`/`phone` etc. are loose `string | null`. Once finding 12 lands and the model gains `first_name`, `last_name`, `additional_emails`, `addresses`, the TS shape will need the same widening.

The Dart mobile types in `mobile/lib/models/` aren't in scope here — they're covered by `frontend-types-parity-2026-05.md`.

---

## 9. Modularisation

Per the workspace standard rule ("aim for <250 LOC per file; split when a file accumulates >2 unrelated responsibilities"):

| File | LOC | Responsibilities | Split? |
|---|---|---|---|
| `backend/src/handlers/calendar.rs` | ~1500 (incl. tests) | events CRUD + iMIP invite send + iMIP accept + free-busy + slot-suggest + helpers | **Yes** — split into `handlers/calendar/{events,freebusy,imip}.rs` |
| `backend/src/handlers/contact_groups.rs` | 487 | group CRUD + vCard import/export + CSV import + merge | **Yes** — split into `handlers/contacts/{groups,import,merge}.rs` |
| `backend/src/models/calendar_event.rs` | 649 | CalendarEvent struct + EventAttendee struct + CRUD for both + busy_intervals + upsert_public_rsvp + tests | Borderline — at minimum lift the public RSVP into its own module; consider splitting attendees out |
| `frontend/src/components/settings/CalendarManager.tsx` | 544 | List view + grid toggle + event form + event detail + public-share UI + RSVP buttons + ICS download | **Yes** — `CalendarManager.tsx` (container only), `EventList.tsx`, `EventForm.tsx`, `EventDetail.tsx`, `PublicShareSection.tsx` |
| `frontend/src/components/settings/ContactsApp.tsx` | 468 | Group sidebar + import dialog + contact list + contact detail + dedupe | **Yes** — `ContactsApp.tsx` (container only), `GroupSidebar.tsx`, `ImportDialog.tsx`, `ContactList.tsx`, `ContactDetail.tsx` |
| Everything else | <250 | single concern | — |

The split work isn't urgent (none of these files have an unexplained 1k-line merge conflict pending), but they're the biggest violators of the 250-LOC guideline in the calendar/contacts feature.

---

## 10. Suggested follow-up tickets

Ordered by priority:

| # | Ticket | Estimate |
|---|--------|----------|
| 1 | **Real RRULE end-to-end** (finding 1) — parse, expand on list, emit in ICS, parse in iMIP, UI for common recurrences | 3–5 days |
| 2 | **Background DAV sync** (finding 13) — sync_token-driven CalDAV/CardDAV → `calendar_events`/`contacts`, with the sync_status columns finally read+written | 5–8 days (full TMAIL-117 follow-on) |
| 3 | **vCard import: full property coverage + folding + multi-value** (finding 12) — needs schema columns for first/last name, addresses, additional emails | 2–3 days |
| 4 | **iMIP invite via queue, not per-attendee SMTP fan-out** (finding 2) — collapses event-create latency from O(n × 500 ms) to O(1) | 1 day |
| 5 | **Batched `event_attendees` insert + UNIQUE(event_id, email)** (findings 3, 4) | half day |
| 6 | **list_for_user pagination + DESC ordering** (finding 5) | half day |
| 7 | **`list_group_contacts` JOIN** (finding 9) | half day |
| 8 | **`merge_contacts` actually merges fields + re-points group memberships** (finding 10) | half day |
| 9 | **Batched CSV import** (finding 11) | half day |
| 10 | **CalDAV parallel fan-out** (finding 8) | quarter day |
| 11 | **`fetch_message_rfc822` consolidation + `BODYSTRUCTURE`-driven calendar part fetch** (findings 6, 7) | 1 day |
| 12 | **DB cleanups** — drop redundant `idx_contacts_email`, enable RLS on `contacts`, add `(organizer_id, start_time, end_time)` (finding §6) | quarter day |
| 13 | **TS literal unions for `status` and `rsvp`** (finding 8) | quarter day |
| 14 | **Modularise `handlers/calendar.rs`, `ContactsApp.tsx`, `CalendarManager.tsx`** (finding §9) | 1 day |
| 15 | **Delete `frontend/src/utils/calendar-helpers.ts`** (finding 21) | trivial |
| 16 | **`useMemo` on `findDuplicates`** (finding 17) | trivial |
| 17 | **Docs**: update CLAUDE.md to say `/api/dav/*` not `/api/caldav/*`; update Alt-UI section to admit calendar IS wired (finding 20, 22.i) | trivial |

**Out of scope for TMAIL-247:** the FreeBusy/SuggestSlots UI path (`SuggestSlotsPanel.tsx`) — used by the composer "Find time" feature, which sits under TMAIL-244 (compose-send).

---

## Severity legend

- **P0** — ship-blocker: a feature we sell that doesn't work. None at this level in calendar/contacts today (RRULE is close but the product can launch beta without recurring events; the runbooks don't promise them).
- **P1** — load-bearing pre-GA: things that would visibly hurt during closed-beta or that quietly lose customer data. **Finding 12 (vCard data loss on import)** is P1 — we explicitly sell "import your Gmail contacts". **Finding 1 (RRULE)** is borderline P1 if any beta customer asks "where's my weekly standup?".
- **P2** — cleanup: everything else, including all the perf/modularisation findings.

---

## Appendix A — files read

Backend:

- `backend/src/handlers/calendar.rs` (1502 lines)
- `backend/src/handlers/public_calendar.rs` (284 lines)
- `backend/src/handlers/contacts.rs` (170 lines)
- `backend/src/handlers/contact_groups.rs` (487 lines)
- `backend/src/handlers/groups.rs` (232 lines)
- `backend/src/handlers/dav_config.rs` (321 lines)
- `backend/src/services/ics_generator.rs` (452 lines)
- `backend/src/services/caldav_freebusy.rs` (577 lines)
- `backend/src/services/imip_parser.rs` (521 lines)
- `backend/src/services/slot_suggester.rs` (548 lines)
- `backend/src/services/vcard_service.rs` (288 lines)
- `backend/src/models/calendar_event.rs` (649 lines)
- `backend/src/models/contact.rs` (318 lines)
- `backend/src/models/contact_group.rs` (266 lines)
- `backend/src/models/dav_config.rs` (593 lines)
- `backend/src/models/distribution_group.rs` (skim only — covered separately)
- `backend/migrations/002_signatures_and_contacts.sql`
- `backend/migrations/031_calendar_events.sql`
- `backend/migrations/043_contact_groups.sql`
- `backend/migrations/048_caldav_config.sql`
- `backend/migrations/071_calendar_public_scheduling.sql`
- `backend/migrations/072_calendar_events_per_organizer_ics_uid.sql`

Frontend:

- `frontend/src/components/settings/CalendarManager.tsx` (544 lines)
- `frontend/src/components/settings/CalendarView.tsx` (157 lines)
- `frontend/src/components/settings/ContactsApp.tsx` (468 lines)
- `frontend/src/components/settings/ContactManager.tsx` (194 lines)
- `frontend/src/components/settings/GroupManager.tsx` (217 lines)
- `frontend/src/api/calendar.ts` (165 lines)
- `frontend/src/api/contacts.ts` (57 lines)
- `frontend/src/api/contact-groups.ts` (90 lines)
- `frontend/src/api/groups.ts` (25 lines)
- `frontend/src/utils/calendar-helpers.ts` (75 lines — dead code, finding 21)
- `themes/shadcn-prototype/src/features/calendar/CalendarView.tsx` (header skim — confirms backend wiring)

## Appendix B — what was NOT covered

- The `SuggestSlotsPanel.tsx` consumer of the slot-suggest endpoint is owned by the composer feature; assessed under TMAIL-244 (`compose-send-2026-05.md`).
- The Flutter mobile contacts/calendar surface is owned by TMAIL-253 (`mobile-sync-push-2026-05.md`).
- Distribution groups (`handlers/groups.rs` / `models/distribution_group.rs`) overlap with the admin domain-level groups assessed in TMAIL-251 — only the user-facing CRUD shape was reviewed here, not the LDAP-backed admin variant.
- Public-calendar scheduling (`public_calendar.rs`, `migrations/071`) was inspected for the unique-token + RLS posture (covered in #22 above) but the visitor-facing booking page (`frontend/src/components/booking/`) was not.
- Live performance numbers — no synthetic load test was run; latency estimates in the table are reasoned from round-trip count and known typical RTTs, not measured.
