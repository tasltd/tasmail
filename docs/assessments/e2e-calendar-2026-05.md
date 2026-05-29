# TMAIL-287 — E2E sweep: calendar (CRUD, ICS, RSVP, free-busy, iMIP, public booking)

- **Issue:** TMAIL-287 (continues the TMAIL-281 / 282 / 283 / 284 / 285 / 286 sweep series)
- **Date:** 2026-05-29
- **Spec:** [`frontend/e2e/calendar.spec.ts`](../../frontend/e2e/calendar.spec.ts)
- **Screenshots:** [`frontend/e2e/screenshots/calendar/`](../../frontend/e2e/screenshots/calendar/) — 13 PNGs covering manager / form / grid / detail / share / booking / cancel.
- **Target:** Live `https://mail.techatscale.io` (workstation backend on `127.0.0.1:3300` reverse-tunnelled through `140.82.32.141:9601`).
- **Browser:** Firefox (per the E2E HARD RULE).
- **Workers:** 1 (`mode: 'serial'` — every test reuses the BYOK organizer + attendee signups from `beforeAll`).

---

## TL;DR

All 10 tests pass on a clean run after the **one production-blocking bug**
this commit ships. The sweep proves the calendar surface — every public
endpoint plus the SPA `CalendarManager`, `CalendarView` (FullCalendar),
event-detail view, and the public `BookingPage` — works end-to-end against
the live tunnel.

| # | Test | Outcome |
| - | - | - |
| 1 | Navigate to Calendar via sidebar (no direct URL) | ✅ pass |
| 2 | Create event with attendee through the UI, API round-trip | ✅ pass — depends on the migration-074 fix |
| 3 | Grid view loads FullCalendar lazy chunk + renders event chip | ✅ pass |
| 4 | Detail view: ICS download + RSVP controls + public-share section | ✅ pass |
| 5 | Cross-user RSVP: attendee accepts, organizer sees status update | ✅ pass — depends on the migration-074 fix |
| 6 | Free-busy: resolved organizer + `not_resolved` external | ✅ pass |
| 7 | Suggest-slots returns candidates in a free working-hours window | ✅ pass |
| 8 | iMIP accept endpoint rejects bogus folder/uid with actionable error | ✅ pass |
| 9 | Public booking: enable share, external RSVP, organizer sees attendee | ✅ pass — depends on the migration-074 fix |
| 10 | Cancel event: row dims, status flips to `cancelled`, API confirms | ✅ pass |

---

## Bug found and fixed: `event_attendees.rsvp` Postgres ENUM blocks every attendee path

Severity: **CRITICAL — every attendee-touching endpoint 500'd in production**.

| Aspect | Detail |
| - | - |
| **Symptom** | `POST /api/calendar/events` with `attendees` returned `{"error":"Internal server error"}` (HTTP 500). Even though the row was persisted, the response body never landed. Same family of bug broke `POST /events/{id}/rsvp`, `POST /imip/accept`, and `POST /public/{token}/rsvp`. |
| **Root cause** | Migration `031_calendar_events.sql` created `event_attendees.rsvp` as a Postgres `ENUM` (`rsvp_status`). The Rust model (`backend/src/models/calendar_event.rs::EventAttendee`) decodes `rsvp` as `String`. sqlx refuses the mismatch with `mismatched types; Rust type 'alloc::string::String' (as SQL type 'TEXT') is not compatible with SQL type 'rsvp_status'`. |
| **Why it survived this long** | The columnar mismatch only surfaces when the row goes through sqlx's `query_as` decode path — i.e. when the response is serialized. The DB write itself succeeded, so the calendar grid (`/api/calendar/events` without joining attendees) was unaffected and TMAIL-127's release validation missed it. Free-busy also avoided it because it queries `calendar_events` aggregates, not `event_attendees`. |
| **Fix** | Added [`backend/migrations/074_event_attendees_rsvp_to_text.sql`](../../backend/migrations/074_event_attendees_rsvp_to_text.sql) — widens the column to `TEXT + CHECK (rsvp IN ('pending','accepted','declined','maybe'))`, mirroring the pattern of migrations `061` / `063` / `065`. The Rust struct stays unchanged. |
| **Applied to live DB** | Yes — the SQL was executed against the workstation Postgres (alleina@127.0.0.1) and the SQLx migration table was updated with the real SHA-384 checksum so `sqlx::migrate!` skips it on the next backend boot. |

### Reproducer (before fix)

```bash
$ curl -sX POST $BASE/api/calendar/events -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"title":"X","start_time":"2026-05-29T14:00:00Z","end_time":"2026-05-29T15:00:00Z","attendees":[{"email":"a@b"}]}'
{"error":"Internal server error"}
# Server log:
# Database error: ColumnDecode { index: "\"rsvp\"", source: "mismatched types; Rust type
#   `alloc::string::String` (as SQL type `TEXT`) is not compatible with SQL type `rsvp_status`" }
```

### Reproducer (after fix)

```json
{
  "id": "f42747f2-7dfb-4dc0-90fc-4a75252d17e5",
  "title": "Probe Meeting",
  "status": "tentative",
  "ics_uid": "073160e8-a6b6-4e9f-aad9-45565645867f@tasmail.io",
  "public_token": "aa57f5e8-c352-4cc3-80a3-7324dd6936a2",
  "public_enabled": false,
  "attendees": [
    { "email": "alice@e2e.tasmail", "display_name": "Alice", "rsvp": "pending" }
  ]
}
```

---

## Surface coverage

### Routes exercised end-to-end

| Route | Verb | Test # | Note |
| - | - | - | - |
| `/api/calendar/events` | GET / POST | 2, 5, 6, 9, 10 | Listing + creation. Auto-sends iMIP REQUEST when attendees provided. |
| `/api/calendar/events/{id}` | GET / PUT / DELETE | 4, 5, 9, 10 | Detail, public-toggle PUT, soft-delete via DELETE. |
| `/api/calendar/events/{id}/rsvp` | POST | 5 | Cross-user RSVP from attendee mailbox. |
| `/api/calendar/events/{id}/ics` | GET | 4 | RFC 5545 VCALENDAR with `METHOD:REQUEST` + organizer ICS UID (migration 072). |
| `/api/calendar/free-busy` | POST | 6 | Resolved vs `not_resolved` attendee paths. |
| `/api/calendar/suggest-slots` | POST | 7 | Honours weekday-only default + duration. |
| `/api/calendar/imip/accept` | POST | 8 | Negative path — no BYO IMAP → 503 with actionable message. |
| `/api/calendar/public/{token}` | GET | 9 | Anonymous projection (no `organizer_id` / `ics_uid` / `public_token` leaked). |
| `/api/calendar/public/{token}/rsvp` | POST | 9 | External RSVP creates an `event_attendees` row visible to the organizer. |

### SPA surfaces exercised

| Surface | File | Test # |
| - | - | - |
| Sidebar Calendar nav entry | `frontend/src/components/layout/Sidebar.tsx` | 1 |
| `CalendarManager` list view + new-event form | `frontend/src/components/settings/CalendarManager.tsx` | 1, 2, 4, 5, 9, 10 |
| `CalendarView` (FullCalendar lazy chunk) | `frontend/src/components/settings/CalendarView.tsx` | 3 |
| Event detail (ICS / RSVP / public-share) | `CalendarManager.tsx::EventDetail` | 4, 5, 9 |
| Public BookingPage | `frontend/src/components/booking/BookingPage.tsx` | 9 |

### What the spec deliberately does NOT cover

- **Inbound iMIP REQUEST → Accept happy path** — requires a real `text/calendar; method=REQUEST` message landing in the attendee's INBOX, which means routing through BYO SMTP → BYO IMAP. That round-trip is covered by `backend/src/handlers/calendar.rs::accept_imip` unit / integration tests; the SPA hits the negative path only.
- **Recurring events (`recurrence_rule`)** — TMAIL-118 deferred RRULE expansion to a follow-up because of an ESM-interop bug between `@fullcalendar/rrule` and `rrule@^2.8`. The grid shows the master event only.
- **CalDAV server config (`/api/dav/*`)** — that's the *outbound* CalDAV client (Radicale / Nextcloud / iCloud), not the public-scheduling surface this issue scopes. Covered separately by `DavConfigManager.test.tsx`.

---

## Selector + reliability changes shipped alongside

The first run surfaced two SPA gotchas that would have re-flaked any future
calendar sweep:

1. **`event-row` data-testid + `data-event-id` / `data-event-title` attrs** on
   `CalendarManager.tsx`'s list row. The previous `div:has-text(title)` pattern
   matched both the title `<div>` (no click handler) and the row container,
   and `.first()` landed on the wrong one — clicks didn't navigate to detail.
   Future calendar specs should follow this anchor pattern.
2. **Auth fast-path** (`loginAsAPI`) that reuses the JWT pair from the
   `beforeAll` signup and seeds `localStorage` via `context.addInitScript`.
   Going through the form-fill login flow for every single test would exhaust
   the auth router's rate limit (`AUTH_RATE_LIMIT_MAX=10/IP/60s`) on the
   second retry pass and randomly time out over the SSH tunnel. Login UI is
   still validated by test 1, which uses the original `loginViaUI`.

---

## How to re-run locally

```bash
# from /home/ddr/Documents/code/project-email-service/frontend
npx playwright test e2e/calendar.spec.ts --project=firefox --retries=2

# Inspect the screenshots
ls e2e/screenshots/calendar/

# Inspect any failing trace
npx playwright show-trace test-results/<failing-spec-path>/trace.zip
```

Run-time on a warm Vite dev server is ~90s. On a cold server the first test
can take ~30s longer while @fullcalendar's chunk transpiles.

---

## Open follow-ups

- **TMAIL-NNN (new):** auto-include the organizer as an `event_attendees` row
  on event creation so `POST /events/{id}/rsvp` works for the organizer
  themselves (currently 404s with "You are not an attendee of this event").
  Useful for the "organizer marks themselves tentative" pattern.
- **TMAIL-NNN (new):** add an iMIP-banner + Accept button to `MessageView`
  so the SPA can drive the `accept_imip` happy path end-to-end. The endpoint
  already exists and is auth-gated; it just has no UI consumer yet.

---

## References

- Bug fix migration: [`backend/migrations/074_event_attendees_rsvp_to_text.sql`](../../backend/migrations/074_event_attendees_rsvp_to_text.sql)
- Spec: [`frontend/e2e/calendar.spec.ts`](../../frontend/e2e/calendar.spec.ts)
- Component change: [`frontend/src/components/settings/CalendarManager.tsx`](../../frontend/src/components/settings/CalendarManager.tsx) (data-testid on event row)
- Sister assessments: [`e2e-auth-2026-05.md`](e2e-auth-2026-05.md), [`e2e-mfa-2026-05.md`](e2e-mfa-2026-05.md), [`e2e-folder-2026-05.md`](e2e-folder-2026-05.md), [`e2e-contacts-templates-2026-05.md`](e2e-contacts-templates-2026-05.md)
