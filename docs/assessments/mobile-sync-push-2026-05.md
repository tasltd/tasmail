# Mobile API, Offline Sync & Push Notifications Assessment — May 2026

**Ticket:** TMAIL-253 (axis of TMAIL-241 modularisation sweep)
**Scope:**
- Backend: `backend/src/handlers/{mobile,sync,push}.rs`,
  `backend/src/models/{mobile,sync,push_notification}.rs`,
  `backend/src/services/push_service.rs`, migration `067_push_quiet_hours_and_grouping.sql`.
- Mobile (Flutter): `mobile/lib/api/api_client.dart`,
  `mobile/lib/services/{sync_service,push_service}.dart`,
  `mobile/lib/models/{sync_checkpoint,email}.dart`.
**Method:** static read of every handler/model/service in scope, plus a
field-by-field diff of the Dart models against the Rust wire shapes. Tests in
each file were skimmed for what is and is not exercised.

---

## TL;DR

The push-notification layer is the cleanest of the three: quiet hours are fully
typed (`NaiveTime` + IANA timezone, validated on write, overnight windows handled
correctly), badge-count sync works end-to-end, and FCM/APNs grouping is
implemented via `collapse_key` / `thread-id` with a precedence rule that is
unit-tested. The mobile-optimised read endpoints deliver real payload wins on
inbox/message/folders/unread/usage.

The two real weak points are:

1. **Offline sync is a façade.** `mobile_sync` only ever emits `NewMessage`
   variants — the wire schema has `FlagChange` and `Deletion` variants that the
   handler never produces. `resolve_conflict` returns `applied: true` without
   ever calling IMAP `STORE`. The `sync_checkpoints` table records
   `last_modseq`/`uidvalidity` but no code path reads them when computing the
   delta. The protocol shape is good; the implementation is not.
2. **Wire drift between Rust handlers and Dart models is real and breaking.**
   At least five field-name or value mismatches will cause the Flutter app to
   misread real responses today (see _Dart/Rust drift_ section). Tests on each
   side pass in isolation because no integration test pairs them.

Two follow-ups have lower stakes:

3. The push **provider dispatch is a hardcoded match** (`fcm`/`apns`/`web`).
   Adding Huawei Push Kit (TMAIL-56) would require editing three places. A
   `dyn PushProvider` trait + registry would localise the change.
4. `/api/mobile/batch` is a **stub** — it validates and acknowledges
   sub-requests but never dispatches them through the router. Listed as
   pending in a `NOTE`; the endpoint is shipped but does nothing.

---

## What was checked

| Axis | Result |
|---|---|
| Mobile endpoints actually shrink payload vs SPA reuse | ✅ Yes for inbox/message/folders/unread/usage; ❌ batch is a stub |
| Sync delta covers new + flag + deletion changes | ❌ Only NewMessage is ever emitted |
| Conflict resolution actually applies (IMAP STORE etc.) | ❌ Stub — returns `applied: true` without side-effects |
| Conflict-resolution strategies registry-driven | ⚠️ Closed enum (3 variants) — fine today, blocks scaling |
| Push provider dispatch registry-driven | ❌ Hardcoded `match device.platform` |
| Quiet hours: typed schedule vs CSV string | ✅ Fully typed (`NaiveTime` + IANA tz, validated) |
| Overnight & DST handling in quiet hours | ✅ Unit-tested (overnight, equal-bounds, invalid tz, NY EDT) |
| Badge count sync is two-way | ✅ Device → server via PUT; server → APNs/FCM via send path |
| FCM/APNs grouping (`collapse_key` / `thread-id`) wired | ✅ Both surface from the same `PushNotificationPayload.collapse_key` |
| Web Push functional | ❌ VAPID auth "not yet implemented" — endpoint is a no-op |
| Dart models match Rust models 1:1 | ❌ ≥5 drifts (see table) |
| Tests cross the wire (integration coverage) | ❌ Each side tested in isolation |

---

## Backend findings

### 1. Mobile endpoints: payload shrink is real

`MobileMessageSummary` (8 fields, ~120–180 B JSON) vs the SPA's full envelope
from `/api/folders/{folder}/messages` (To/Cc lists, all flags, multipart info,
~600–900 B JSON). On a 20-row inbox the saving is ~10 KB. The biggest single
win is low-bandwidth message detail
(`handlers/mobile.rs:182–211`): in `low_bandwidth=true` mode it strips HTML,
caps the body at `LOW_BANDWIDTH_PREVIEW_CHARS` (280), empties To/Cc, and drops
HTML entirely — bounded cost regardless of sender body size.

UTF-8 handling is correct (`truncate_chars` uses `char_indices`, not
`&body[..n]`), which matters for Twi/Ewe diacritics — there is a test for it.

`/api/mobile/usage` is a ~120 B trim of `/api/quota` (~250 B) and skips the
Redis-backed cache. Net win for push-notification refresh cycles.

### 2. `mobile_batch` is a stub

`handlers/mobile.rs:289–341` validates the request (method allowlist, path
prefix, size cap) but the response is built from a placeholder loop:

```rust
.map(|req| BatchResponseItem {
    status: 200,
    body: json!({
        "message": format!("Batch sub-request {} {} acknowledged", req.method, req.path),
        "pending": true,
    }),
})
```

The `NOTE` says a full implementation would use `axum::Router::oneshot()`. As
it stands the endpoint is shipped, route-tested, accepting load, and returning
fake 200s. Any mobile client calling it gets a no-op. File this as a real
ticket or pull the route until it works.

### 3. Sync delta is incomplete

`SyncChange` is a tagged enum with three variants:

```rust
pub enum SyncChange {
    NewMessage  { folder, uid, from, subject, date },
    FlagChange  { folder, uid, flags },
    Deletion    { folder, uid },
}
```

`mobile_sync` and `mobile_sync_post` only ever push `NewMessage`. Flag changes
(read/unread, star/unstar) and deletions are invisible — the mobile client
will not learn about them via this endpoint. There is no IMAP CONDSTORE or
QRESYNC call, despite `sync_checkpoints` recording `last_modseq` for exactly
this purpose. A `NOTE` acknowledges this: _"A production implementation would
maintain a server-side change log or use IMAP CONDSTORE/QRESYNC extensions for
true delta sync."_

The cursor on POST is opaque on the wire (good) but is just an RFC3339
timestamp underneath (acceptable for now since `imap_service` does not yet
expose `MODSEQ`).

### 4. Conflict resolution is a protocol stub

`resolve_conflict` (`handlers/sync.rs:111–163`) returns `applied: true`
unconditionally for any valid `ConflictResolution`. The `NOTE` lists the three
things a full implementation would do (discard / `STORE` / merge) — none of
them happen today. A client calling this endpoint cannot trust that the
returned state matches the server state.

### 5. Conflict-resolution strategy is a closed enum

`ConflictResolution` is a 3-variant Rust enum with hand-rolled `from_str` /
`as_str` (`models/sync.rs:11–40`). Adding a fourth strategy (e.g.
`last_writer_wins`, `field_level_merge`) means editing the enum, two match
arms, and potentially a DB `CHECK` constraint. At 3 variants this is fine —
the "modularize" rule cuts in when we add the 4th. Keep this on the radar
rather than refactor now.

### 6. Push provider dispatch is hardcoded

`services/push_service.rs:266–283`:

```rust
match device.platform.as_str() {
    "fcm"  => send_fcm(...).await,
    "apns" => send_apns(...).await,
    "web"  => send_web_push(...).await,
    other  => { tracing::warn!(...); Err(...) }
}
```

This is the textbook example our `Modularize` rule names as an anti-pattern.
Adding Huawei Push Kit (TMAIL-56) or Amazon SNS or a mock-for-tests provider
means editing **three** call sites (the `PushPlatform` enum, the `match`, plus
a new `send_*` function). A `trait PushProvider { fn id() -> &'static str; fn
build(...) -> Bytes; fn endpoint(...) -> String; async fn send(...) ->
Result<()>; }` plus a
`HashMap<&'static str, Box<dyn PushProvider>>` keyed by platform localises the
change to one new file. Worth doing _before_ TMAIL-56 lands, not after.

### 7. Web Push is non-functional

`send_web_push` posts to the endpoint URL with no VAPID `Authorization`
header. The standard-compliant browser push services (Mozilla, Google, Apple)
all reject unsigned requests with 401/403. Until VAPID is implemented this
provider is dead code. Either implement VAPID, mark the `web` platform as
disabled, or block registration of `web` devices in `register_device`. The
current behaviour — silent registration, silent failure on send, "delivered:
false" in the log — is the worst of the three options.

### 8. Quiet hours: clean and well-tested

This is the best-built piece of the assessment surface.

- Typed: `quiet_hours_start: Option<NaiveTime>`, `quiet_hours_end:
  Option<NaiveTime>`, `quiet_hours_timezone: Option<String>` (validated as
  IANA via `chrono_tz::Tz::parse` on write).
- DB-enforced: migration `067` adds the columns + `badge_count >= 0` CHECK.
- Overnight windows handled: `is_in_quiet_hours` inverts the comparison when
  `start > end` (e.g. 22:00 → 07:00), with unit tests for late-night,
  early-morning, midday, evening, equal-bounds (= locked DND), and a
  cross-timezone test (NY EDT vs UTC).
- Partial-window guard: PUT rejects `(Some, None)` or `(None, Some)` —
  you must set both or clear both.
- One-window-per-device limit: cannot represent "9pm–7am AND 12pm–1pm".
  Fine for the Ghana market today; revisit if multiple-window DND becomes a
  paid-tier feature.

### 9. Badge count: round-trip works

Device PUTs its current unread count → row updated → next send to that device
falls back to `device.badge_count` when the payload has no explicit
`payload.badge`. Precedence is tested:
`payload.badge > device.badge_count > default(1)` for APNs,
`payload.badge.or(badge_override)` for FCM `data.badge`. Good.

---

## Mobile (Flutter) findings

### 10. `ApiClient` is a singleton with auto-refresh — fine

`api/api_client.dart` is a Dio-based singleton with secure-storage token
persistence and a 401 → `/auth/refresh` → retry-original interceptor.
Standard pattern; nothing to flag.

### 11. Dart/Rust drift — the biggest live bug surface

Each side has its own tests passing in isolation. There is no contract test
that compares actual JSON payloads against the Dart model — so all of these
drifts will only surface at runtime.

| Dart expects | Rust emits | Direction | Severity |
|---|---|---|---|
| `SyncChange.fromJson` reads `'change_type'` | Rust uses `#[serde(tag = "type")]` → emits `'type'` | Server→Client | 🔴 Breaks — Dart throws or reads null |
| `SyncChange.data` field nested object | Rust flattens `from`/`subject`/`date` at the top level | Server→Client | 🔴 Breaks — Dart `data` always `null` |
| `SyncDelta.checkpoint` | Rust emits `sync_token` | Server→Client | 🔴 Breaks — Dart `checkpoint` always `null` |
| `PushService.registerToken` sends `'token'` + `platform: 'android'\|'ios'` | Rust expects `'device_token'` + `platform: 'fcm'\|'apns'\|'web'` | Client→Server | 🔴 Breaks — 400 BadRequest |
| `MobileMessageSummary.fromJson` reads `'folder'` and `'total_count'` | `mobile_inbox` returns `total` not `total_count`; inbox rows have no `folder` field | Server→Client | 🟠 Field missing → defaults |
| `MobileFolderSummary.totalCount` | Rust mobile folder summary has no `total_count` | Server→Client | 🟢 Defaults to 0 — visible-but-not-fatal |
| `mobile_usage` response | Dart has no model for `MobileUsage` yet | n/a | 🟢 Endpoint unused on mobile so far |

The five 🔴 rows are runtime failures the next time anyone wires these
services to a real screen. Need a follow-up ticket to:
- Pick which name wins (`type` is the Rust convention, change Dart) for sync.
- Rename Dart `checkpoint` → `sync_token` (or the reverse).
- Normalise platform strings on the Rust side OR translate on the Dart side,
  but pick one place.
- Add a `'folder'` field to the inbox row response (cheap, useful) OR drop
  it from Dart.

### 12. `SyncCheckpoint` Dart model: matches

This one model is clean — fields line up 1:1 with the Rust struct, names
match (snake_case in JSON), `needsFullSync` is a derived property on the Dart
side rather than a wire field. ✅

### 13. Push service: hardcoded platform string + missing FCM SDK

`push_service.dart::registerToken` accepts `platform: 'android' | 'ios'`. Both
values are wrong (server expects `fcm`/`apns`). On top of that the file's own
header comment notes _"FCM dependency (`firebase_messaging`) should be added
when Firebase is configured"_ — there is no actual FCM token acquisition
code, so even fixing the platform string would only get the server-side
registration right, not the client-side push reception.

This is consistent with the TMAIL-150 backlog item ("Implement push
notifications via FCM"); the assessment is just documenting what's there.

### 14. `SyncService` has no scheduler hook

`shouldSyncNow({required bool onWifi})` is a single decision point ready for
a background worker (WorkManager on Android, BGTaskScheduler on iOS) to call.
No background worker actually calls it today — TMAIL-51 landed the protocol
plumbing but TMAIL-151 (offline sync with local database) is still on the
backlog.

---

## Recommendations

Grouped by effort. Each maps to a ticket that should exist.

### Quick wins (≤1 day)

1. **Fix the five wire drifts.** Add a contract test (snapshot the Rust JSON,
   feed to Dart `fromJson`) so the next drift surfaces in CI. New ticket.
2. **Remove or block `/api/mobile/batch`.** Either implement
   `Router::oneshot()` dispatch or return `501 Not Implemented` until it
   works. Today's "200 OK + pending: true" is a foot-gun. New ticket.
3. **Block `web` platform registration** (or implement VAPID). Same
   foot-gun pattern. New ticket.
4. **Add a `folder` field to `MobileMessageSummary`** server-side so the Dart
   model stops defaulting it. ~20 lines. Roll into the drift-fix PR.

### Medium (1–3 days)

5. **Emit `FlagChange` and `Deletion` from `mobile_sync`.** Use the
   server-side change log if/when one exists; in the interim, compare the
   IMAP `FLAGS` of the recent UIDs against a per-device cache. Tied to
   TMAIL-51.
6. **Make `resolve_conflict` actually apply the resolution.** Three real
   IMAP code paths (no-op / `STORE` / merge-and-`STORE`). Tied to TMAIL-51.

### Larger (≥3 days)

7. **Provider registry for push.** `trait PushProvider` + registry keyed by
   platform string. Block on TMAIL-56 (Huawei Push Kit) — no point doing this
   abstractly; do it as the migration that lands Huawei. Wire `apns`/`fcm`
   as the first two concrete impls.
8. **Real CONDSTORE/QRESYNC delta.** Requires extending `ImapService` to
   expose `MODSEQ` and `VANISHED`. Stops being a stub. Probably a TMAIL-51
   follow-up after IMAP-level support lands.

### Non-actions (intentional non-recommendations)

- **Do not** convert `ConflictResolution` to a registry today. 3 variants is
  too small to pay for the abstraction. Revisit when the 4th lands.
- **Do not** widen quiet hours to multiple windows. One window covers ~95%
  of users; revisit when a paying customer asks.

---

## Appendix

### Source files (line counts current as of this assessment)

| File | Lines | Role |
|---|---|---|
| `backend/src/handlers/mobile.rs` | 891 (incl. 220+ test lines) | 7 mobile route handlers + helpers |
| `backend/src/handlers/sync.rs` | 315 | 4 sync endpoints (checkpoint CRUD + conflict resolve) |
| `backend/src/handlers/push.rs` | 205 | 6 push endpoints (register / list / unregister / test / quiet-hours / badge) |
| `backend/src/services/push_service.rs` | 583 | FCM/APNs/Web Push senders + payload builders |
| `backend/src/models/mobile.rs` | 398 | `MobileMessageSummary`, `BatchRequest`, `SyncChange`, `MobileUsage` |
| `backend/src/models/sync.rs` | 422 | `SyncCheckpoint`, `ConflictResolution`, request/response DTOs |
| `backend/src/models/push_notification.rs` | 691 | `PushDevice`, `PushNotificationLog`, `is_in_quiet_hours` |
| `backend/migrations/067_push_quiet_hours_and_grouping.sql` | 21 | Quiet hours columns + `badge_count` |
| `mobile/lib/api/api_client.dart` | 136 | Dio client + JWT auto-refresh |
| `mobile/lib/services/sync_service.dart` | 195 | Delta sync + checkpoint + conflict + draft queue |
| `mobile/lib/services/push_service.dart` | 96 | Push registration + quiet hours + badge sync |
| `mobile/lib/models/sync_checkpoint.dart` | 53 | Per-folder checkpoint (lines up with Rust) |
| `mobile/lib/models/email.dart` | 160 | `MobileMessageSummary`, `MobileMessageDetail`, `InboxResponse` |

### Routes audited (router.rs:941–986)

```
GET    /api/mobile/inbox
GET    /api/mobile/message/{folder}/{uid}
GET    /api/mobile/folders
GET    /api/mobile/unread-count
POST   /api/mobile/batch
GET    /api/mobile/sync
POST   /api/mobile/sync
GET    /api/mobile/usage
POST   /api/push/register
GET    /api/push/devices
DELETE /api/push/devices/{id}
POST   /api/push/test
PUT    /api/push/devices/{id}/quiet-hours
PUT    /api/push/devices/{id}/badge
GET    /api/sync/checkpoints
GET    /api/sync/checkpoint/{folder}
POST   /api/sync/checkpoint/{folder}
POST   /api/sync/resolve-conflict
```

### Tickets referenced

- **TMAIL-50** Push notifications (FCM/APNs + quiet hours + badge) — partial.
- **TMAIL-51** Offline sync — protocol shape landed, delta + conflict apply
  still stubbed.
- **TMAIL-52** Mobile-optimised payloads — landed.
- **TMAIL-56** Huawei Push Kit — not started. Should land alongside provider
  registry refactor.
- **TMAIL-150** FCM client integration on Flutter — backlog.
- **TMAIL-151** Offline sync with local database — backlog.

### Out of scope

The Mobile-app onboarding flow (`/api/auth/signup` + onboarding wizard),
biometric auth (TMAIL-142), and ActiveSync (`handlers/activesync.rs`) are
deliberately not in this assessment — they are flagged for separate
TMAIL-241 axes.
