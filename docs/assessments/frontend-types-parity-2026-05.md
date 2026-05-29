# Frontend Types & API Contract Parity (Rust ↔ TS ↔ Dart)

> **Tickets:** TMAIL-257 / TMAIL-262 (the two are duplicates per `INDEX.md`;
> this single report closes both)
> **Parent epic:** TMAIL-241 — modularisation, static-types, performance &
> scalability assessment (cross-feature)
> **Cycle:** 2026-05 — point-in-time audit at `main` HEAD around 2026-05-29
> **Scope:** `backend/src/models/`, `backend/src/handlers/`,
> `backend/src/services/auth_service.rs` (where wire types live),
> `frontend/src/types/`, `frontend/src/api/`,
> `mobile/lib/models/`

---

## TL;DR

The Rust ↔ TS contract is in **broadly good shape** — zero `Promise<any>` and
zero `Promise<unknown>` return types across all 65 `frontend/src/api/*.ts`
modules — but three real problems matter for shipping:

1. **P0 — Dart `LoginResponse` will crash at runtime against the live backend.**
   `mobile/lib/models/auth.dart::LoginResponse.fromJson` expects
   `json['user']` (non-nullable cast), but `backend/src/handlers/auth.rs::login`
   returns `Json<auth_service::TokenPair>` — the `{ access_token, refresh_token,
   expires_in }` triple, with no `user` field. The Dart factory throws
   `_CastError` before the user reaches the inbox. The TS `TokenPair` shape
   matches the Rust source of truth; Dart is the divergent one.
2. **P1 — Mobile model surface covers 4 of 60 Rust models (~6.7%).**
   Anything beyond login, inbox-list, message-detail, and sync-checkpoint
   silently flows as `Map<String, dynamic>` through the Dart `ApiClient`. As
   feature work lands on mobile (settings, contacts, calendar, signatures, 2FA)
   each new screen will roll its own ad-hoc model, drifting further from Rust.
   This is the largest structural gap in the contract and needs a strategy
   decision, not a per-screen fix.
3. **P1 — Inline-interface sprawl: 255 interfaces inlined in `api/*.ts` vs 27
   in the canonical `types/` directory** (a 9.4× ratio). Several entity types
   (`CalendarEvent`, `EmailDelegation`, `SieveRule`) exist in `api/*.ts` but
   not in `types/`, so non-API consumers (components, hooks) import them via
   `import type { X } from '../api/calendar'` — coupling presentation to the
   API client. Lifting these into `types/` is mechanical and unlocks a clearer
   future generator pipeline.

Beyond those: a handful of free-form `status: string` / `rsvp: string` fields
on the Rust side that the TS side has already correctly narrowed into
discriminated unions on outbound requests but not on inbound responses; a few
free-form `field: String` / `operator: String` strings in `models/sieve_rule.rs`
that the TS side has correctly narrowed (Rust is *wider* than TS here, which is
the wrong direction); and a small set of handler files (`mobile`, `sync`,
`websocket`, `metrics`, `health`, `mod`) with no TS pairing — all intentional.

The wire format is mostly OK. The **type discipline around the wire** is what
needs the work.

---

## 1. Surface-area metrics

Snapshot at HEAD ~2026-05-29.

| Surface | Files | Notes |
|---|---|---|
| Rust handler files (`backend/src/handlers/*.rs`) | **67** | Includes `mod.rs`; `admin/` subdir adds another 6 admin handlers separately |
| Rust model files (`backend/src/models/*.rs`) | **60** | Includes `mod.rs` |
| TS API modules (`frontend/src/api/*.ts`, excl. tests) | **65** | Plus 50+ colocated `*.test.ts` siblings |
| TS canonical types (`frontend/src/types/*.ts`) | **7** | `admin`, `auth`, `groups`, `mail`, `migration`, `pst-import`, `shared-mailboxes` |
| Dart model files (`mobile/lib/models/*.dart`) | **4** | `auth`, `email`, `attachment_draft`, `sync_checkpoint` |
| TS interface/type exports total (`api/`) | **255** | Inline — co-located with the call site |
| TS interface/type exports total (`types/`) | **27** | Canonical |
| `Promise<any>` returns | **0** | Clean |
| `Promise<unknown>` returns | **0** | Clean |

### Handler → TS pairing (54 / 67 paired by name)

Pairing rule: handler file `foo.rs` → either `api/foo.ts` or `api/foo-bar.ts`
(underscore → kebab). **MISS** rows below are unpaired by name; checked below
for whether the gap is intentional.

| Status | Count | Handler files |
|---|---|---|
| OK (direct name match) | 54 | `activesync, ai_config, archive, attachments, auth, auto_reply, billing, branding, bulk_import, cache, calendar, chat_integrations, comments, contact_groups, contacts, custom_hostnames, dane, dav_config, delegation, deliverability, dlp, ediscovery, eml, folders, groups, ldap, messages, migration, nlp_search, oidc, ollama, phishing, plugins, pop3_config, pst_import, public_calendar, push, queue, quota, retention, saml, scheduled, semantic_search, shared_files, signatures, sms_otp, smtp_config, snooze, spam, tasks, templates, two_factor, webauthn, webhooks` |
| MISS — intentional (not SPA-consumed) | 8 | `health` (public probe), `metrics` (Prometheus), `mobile` (mobile-only — see `CLAUDE.md` API Route Structure), `sync` (mobile-only), `websocket` (`/ws` — auth via query-param token, no REST shape), `mod` (Rust module manifest, not a handler), `mailbox_provision` (admin-internal), `shared` (small helper shared between handlers, not a route group) |
| MISS — renamed pair | 5 | `enterprise_quote` → `quoteRequests.ts`, `imap_config` → `byok.ts` (bundles IMAP+SMTP wizard wiring), `sieve` → `filters.ts` (TS uses user-facing name), `usage_billing` → folded into `billing.ts`, `warmup` → `admin-warmup.ts` |

No genuine orphan handlers. The renamed pairs are documented but not
greppable — the CI `trace-check` script handles this via its alias map, which
is the right home for the knowledge.

### Inline vs canonical TS interface distribution

`grep -rE '^export (interface|type) '` results:

```
types/  → 27 interfaces in 7 files  (avg 3.9 per file)
api/    → 255 interfaces in 58 files (avg 4.4 per file)
```

The 27 canonical interfaces in `types/` cover: auth (3), mail (7 — `Folder`,
`MessageEnvelope`, `FullMessage`, `Attachment`, `MessageListResponse`,
`SearchResponse`, `SendEmailRequest`), admin (4), groups (5),
shared-mailboxes (4), migration (3), pst-import (1).

The other ~28 entity types that *should* be canonical are inlined in `api/`:
`CalendarEvent`, `EventAttendee`, `CalendarEventWithAttendees`, `RsvpRequest`,
`FreeBusyRequest`, `FreeBusyResponse`, `AttendeeBusy`, `BusySpan`,
`SuggestSlotsRequest`, `SuggestSlotsResponse`, `SuggestedSlot`,
`EmailDelegation`, `DelegationType`, `CreateDelegationRequest`, `SieveRule`,
`RuleCondition`, `RuleAction`, `CreateFilterRequest`, `UpdateFilterRequest`,
`Contact`, `EmailTemplate`, `Signature`, `Task`, `EmailQueueEntry`, `Webhook`,
`PhishingReport`, `BulkImportJob`, `AiConfig` — and ~225 request/response
shapes.

The inline-by-default convention is fine for request/response shapes that only
the API module knows about. It's the *entity* types (`SieveRule`, `Contact`,
`CalendarEvent`) that need to live in `types/` so React components can consume
them without coupling to the API client.

---

## 2. P0 — Dart `LoginResponse` will crash at runtime

This is the most surprising finding and almost certainly explains why nobody
has loaded the mobile app against the live backend recently.

### The contract

`backend/src/handlers/auth.rs:30`:

```rust
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<(StatusCode, Json<auth_service::TokenPair>), AppError> {
    ...
    let tokens = auth_service::authenticate(...)?;
    Ok((StatusCode::OK, Json(tokens)))
}
```

`backend/src/services/auth_service.rs:61`:

```rust
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}
```

So the wire shape is exactly:

```json
{ "access_token": "…", "refresh_token": "…", "expires_in": 900 }
```

### TS consumer (correct)

`frontend/src/types/auth.ts:8`:

```ts
export interface TokenPair {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}
```

Matches 1:1. ✅

### Dart consumer (broken)

`mobile/lib/models/auth.dart:16`:

```dart
class LoginResponse {
  final String accessToken;
  final String refreshToken;
  final UserInfo user;        // ← non-nullable, but the field doesn't exist in the response
  ...
  factory LoginResponse.fromJson(Map<String, dynamic> json) {
    return LoginResponse(
      accessToken: json['access_token'] as String,
      refreshToken: json['refresh_token'] as String,
      user: UserInfo.fromJson(json['user'] as Map<String, dynamic>),  // ← _CastError
    );
  }
}
```

`json['user']` is `null` → `null as Map<String, dynamic>` throws
`_TypeError: type 'Null' is not a subtype of type 'Map<String, dynamic>' in
type cast`. Login fails before the mobile app can even store the tokens.

### What Dart is also missing

| Field | Rust | TS | Dart |
|---|---|---|---|
| `access_token` | `String` | `string` | `String` ✅ |
| `refresh_token` | `String` | `string` | `String` ✅ |
| `expires_in` | `u64` | `number` | **missing** ❌ |
| `user` | *(not returned)* | *(not in TokenPair)* | **expected non-nullable** 💥 |

### Likely cause

The Dart model was clearly written from a *plan* of what the login response
*should* look like (probably mirroring an OAuth2-style nested user blob from
some other system) rather than from the actual Rust handler. It would have
slipped past CI because:

- No mobile integration test runs against a real backend (the Flutter `test`
  suite is unit-only — see `mobile/test/`).
- The `trace-check.py` script audits Rust ↔ TS only, not Rust ↔ Dart.

### Fix shape

Two options:

- **(A) Match the Rust contract** — drop `user` from `LoginResponse`, fetch
  the user separately via `GET /api/me` (or whatever the canonical "current
  user" endpoint becomes). Cleanest. Matches TS.
- **(B) Widen the Rust contract** — make `login` return a wrapping struct
  `{ tokens: TokenPair, user: UserSummary }`. Coordinated rollout because the
  TS client also needs updating.

Recommend (A) — it's a one-file Dart change, no backend wire-breaking, no TS
churn. The mobile login flow then needs a follow-up `GET /api/me` call to hydrate
the user before the inbox renders. Filed as a follow-up task (§ Follow-ups
below).

---

## 3. P1 — Mobile coverage gap (4 of 60 Rust models)

```
mobile/lib/models/
├── attachment_draft.dart      → no direct Rust model (compose-side staging)
├── auth.dart                  → maps to TokenPair (broken — see § 2)
├── email.dart                 → maps to mobile.rs (MobileMessageSummary etc.)
└── sync_checkpoint.dart       → maps to sync.rs::SyncCheckpoint
```

The four models cover the **mobile-only** Rust surface (`backend/src/models/mobile.rs`
and `backend/src/models/sync.rs`) plus auth. They do **not** cover the 56
remaining Rust models that the mobile app will need as the surface grows
(active PM backlog items TMAIL-152 "Mobile settings screens (signatures,
contacts, 2FA)" and TMAIL-153 "Mobile attachment viewer" both touch areas
without Dart models today).

### What that means in practice

When a mobile screen calls a non-mobile endpoint (e.g. `GET /api/contacts`
once the contacts screen lands), the Dart client today receives
`Map<String, dynamic>` and the screen reads `map['phone']` against a moving
Rust contract. Any rename or removal on the Rust side fails silently in
mobile — the field just becomes `null` at the property-access site.

This is **the** scaling problem for the mobile surface and the cost rises as
more screens land. The longer the gap exists, the more ad-hoc per-screen
parsing accumulates.

### Field-by-field drift in the four existing models

#### `MobileMessageSummary` (email.dart:5) vs `models/mobile.rs:16`

| Field | Rust | Dart | Drift |
|---|---|---|---|
| `uid` | `u32` | `int` (uid) | OK |
| `from` | `Option<String>` | `String?` | OK |
| `subject` | `Option<String>` | `String?` | OK |
| `date` | `Option<String>` | `String?` | OK |
| `is_read` | `bool` | `bool` (isRead) | OK |
| `is_flagged` | `bool` | `bool` (isFlagged) | OK |
| `has_attachment` | `bool` | `bool` (hasAttachment) | OK |
| `preview` | `Option<String>` | **missing** | ❌ low-bandwidth preview ignored by mobile |
| **`folder`** | *(not in Rust)* | `String` (required) | ❌ Dart invents a field |

The `folder` field is a Dart-side hallucination. The Rust handler returns
messages from a single folder per request — the folder is the URL path, not a
response field. Right now Dart parses incoming JSON with a missing key — but
`folder: json['folder'] as String` casts `null` to `String`, which throws.
Either (a) the mobile inbox isn't actually working today on a fresh install,
or (b) the wrapper somewhere upstream injects the folder name into each
summary before `fromJson` runs. Either way it's a contract bug.

#### `MobileMessageDetail` (email.dart:40) vs full message response

The Dart model bundles 13 fields (`uid, folder, from, to[], cc[], subject,
date, body_html, body_text, is_read, is_flagged, has_attachment,
attachments[]`). The Rust mobile handler `handlers/mobile.rs` returns a
similar but not identical shape — needs a focused check (filed as follow-up).

#### `SyncCheckpoint` (sync_checkpoint.dart:6) vs `models/sync.rs::SyncCheckpoint`

This one is healthy:

| Field | Rust | Dart | Drift |
|---|---|---|---|
| `folder_name` | `String` | `String` (folderName) | OK |
| `device_id` | `Option<String>` | `String?` | OK |
| `last_uid` | i64 (approx — used as IMAP UID) | `int` | OK |
| `last_modseq` | i64 | `int` | OK |
| `uidvalidity` | i64 | `int` | OK |
| `last_synced_at` | `Option<DateTime<Utc>>` | `DateTime?` | OK |

The Dart model also adds two helpers (`needsFullSync`, `requiresResyncAfter`)
that aren't on the Rust side — pure client-side derived state, no drift risk.
This is the model to copy when filling out the other 56.

#### `attachment_draft.dart`

Compose-side staging, no Rust counterpart by design — it represents an
attachment selected by the user but not yet uploaded. Not a drift candidate.

### Strategy options for the 56 missing models

| Option | Pros | Cons |
|---|---|---|
| Hand-write Dart models as screens land | Selective, low investment | Repeats the auth.dart mistake on every model — no enforcement |
| Use `freezed` + `json_serializable` with `build_runner` | Idiomatic Dart, codegen-checked | Maintenance overhead per model, still hand-aligned to Rust |
| Generate Dart models from a shared OpenAPI / `serde` JSON Schema | Single source of truth, drift impossible | Backend has no OpenAPI today — significant up-front investment |
| Generate from an `openapi.json` fixture that the trace-check produces | Reuses existing CI infra | Trace-check today is presence-only, not shape-aware |

The right answer depends on how serious mobile is as a near-term product. The
TASCIM PM backlog currently has TMAIL-152 and TMAIL-153 marked priority=urgent
but both `Backlog` status — until those are picked up the mobile surface will
continue to grow ad-hoc. Recommend filing the strategy decision as a follow-up
task explicitly.

---

## 4. P1 — Inline-interface sprawl

255 interfaces in `api/`, 27 in `types/`. The breakdown by `api/*.ts` file
(top of distribution):

| File | Inline exports | Should move to `types/`? |
|---|---|---|
| `calendar.ts` | 13 | Yes — `CalendarEvent`, `EventAttendee`, `CalendarEventWithAttendees` are entities |
| `ai-config.ts` | 12 | Partial — `AiConfig` is an entity |
| `dlp.ts` | 9 | Partial — `DlpRule` is an entity |
| `billing.ts` | 8 | Yes — `Plan`, `Subscription`, `Invoice` are entities |
| `archive.ts` | 8 | Partial — `ArchivePolicy` is an entity |
| `plugins.ts` | 7 | Partial |
| `deliverability.ts` | 7 | Partial |
| `webhooks.ts` | 5 | Partial — `Webhook` is an entity |
| `dane.ts` | 6 | Mostly request/response |
| `byok.ts` | 6 | Yes — `ImapPreset`, `SmtpConfig` are entities |
| `phishing.ts` | 6 | Partial |
| `quoteRequests.ts` | 6 | Yes — entity-heavy |
| `ollama.ts` | 6 | Mostly request/response |
| `templates.ts` | 5 | Yes — `EmailTemplate` is an entity |
| `filters.ts` | 5 | Yes — `SieveRule` is an entity |
| `contact-groups.ts` | 5 | Partial |
| `groups.ts` (in api/) | 5 | Already partially in `types/groups.ts` — **dup risk** |

A focused refactor would lift ~28 entity types from `api/` into `types/`,
leaving the request/response shapes inline where they're tightly coupled to a
single call. The mechanical lift is straightforward — the value is opening
the door to a future generator that walks Rust models and emits `types/*.ts`
under codegen rather than hand-typing.

### Worst dup risk: `groups.ts`

There's a `types/groups.ts` with `DistributionGroup`, `GroupMember`,
`CreateGroupRequest`, `UpdateGroupRequest`, `AddMemberRequest` (5 exports)
AND an `api/groups.ts` with another 5 exports. These need to be checked for
drift in a follow-up — the names overlap.

---

## 5. P2 — Discriminated unions: TS is sometimes tighter than Rust

The codebase has good instincts about discriminated unions in TS:

```ts
// frontend/src/api/calendar.ts:67
export interface RsvpRequest {
  status: 'accepted' | 'declined' | 'maybe';
}

// frontend/src/api/filters.ts:3
export interface RuleCondition {
  field: 'from' | 'to' | 'cc' | 'subject' | 'body' | 'header' | 'size';
  operator: 'contains' | 'not_contains' | 'equals' | 'starts_with'
          | 'ends_with' | 'matches_regex' | 'greater_than' | 'less_than';
  value: string;
}

// frontend/src/api/delegation.ts:5
export type DelegationType = 'send_as' | 'send_on_behalf';
```

But the Rust side is often `String`:

```rust
// backend/src/models/calendar_event.rs:86
pub struct RsvpRequest {
    pub status: String,
}

// backend/src/models/sieve_rule.rs:8
pub struct RuleCondition {
    pub field: String,      // /// "from", "to", "cc", "subject", "header", "size", "body"
    pub operator: String,   // /// "contains", "not_contains", "equals", ...
    pub value: String,
}
```

Direction of drift matters:

- **TS narrower than Rust** = if Rust ever sends `"tentative"` for RSVP, TS
  rejects valid data. This is the *worse* direction because the source of
  truth ships something the consumer can't represent.
- **TS wider than Rust** = if TS sends `"foo"` and Rust rejects, the user
  gets a 400. Recoverable.

The current pattern in this codebase is the first one — TS is the source of
narrowness, and there's no static check that the Rust enum-strings stay in
sync with TS literals.

### Where Rust already has proper enums

The good examples to copy:

| Rust file | Type | Pattern |
|---|---|---|
| `models/ai_config.rs:18` | `AiProvider` | `#[serde(rename = "openai|anthropic|google|ollama|custom")]` — full enum, 5 variants |
| `models/dav_config.rs:15` | `DavType` | `#[serde(rename_all = "lowercase")] enum { Caldav, Carddav }` |
| `models/dlp_rule.rs:13` | `DlpAction` | `#[serde(rename = "block|quarantine|warn|log")]` |
| `models/spam.rs:14` | `SpamAction` | `#[serde(rename = "reject|greylist|add_header|no_action")]` |
| `models/webhook.rs:12` | `WebhookEventType` | `#[serde(rename = "email.received|email.sent|...")]` |
| `models/chat_integration.rs:14` | `ChatProvider` | `slack|teams|google_chat|discord|custom` |
| `models/mobile.rs:65` | `SyncChange` | `#[serde(tag = "type")] enum { NewMessage{...}, FlagChange{...}, Deletion{...} }` — full discriminated-union! |

### Where Rust *should* have enums but doesn't

| Rust file | Field | TS narrowing | Suggested Rust enum |
|---|---|---|---|
| `calendar_event.rs:20` | `status: String` | _(none in TS — TS also `string`)_ | `EventStatus { Confirmed, Tentative, Cancelled }` (per RFC 5545) |
| `calendar_event.rs:42` | `rsvp: String` | `'accepted'|'declined'|'maybe'` in `RsvpRequest` only | `Rsvp { Accepted, Declined, Tentative, NeedsAction }` |
| `calendar_event.rs:86` | `RsvpRequest.status: String` | tightened TS-side | tighten Rust-side to match |
| `sieve_rule.rs:8` | `field: String` | `'from'|'to'|...` | `ConditionField { From, To, Cc, Subject, Body, Header, Size }` |
| `sieve_rule.rs:11` | `operator: String` | full union in TS | `ConditionOperator { Contains, NotContains, ... }` |
| `sieve_rule.rs:21` | `action_type: String` | full union in TS | `RuleActionType { Move, Copy, ... }` |
| `email_delegation` (in handler) | `delegation_type: String` | `'send_as'|'send_on_behalf'` | `DelegationType { SendAs, SendOnBehalf }` |
| `mailbox.rs` | `role: String` (likely) | _(not narrowed)_ | check |

This is the highest-leverage Rust-side cleanup the report identifies. Each
fix is local to one file and gains static guarantees that ripple through
every serde call site.

### `#[serde(flatten)]` pattern — used correctly

`models/calendar_event.rs:91` and `models/ediscovery.rs:134` use `flatten`
for response composition:

```rust
#[derive(Debug, Serialize)]
pub struct CalendarEventWithAttendees {
    #[serde(flatten)]
    pub event: CalendarEvent,
    pub attendees: Vec<EventAttendee>,
}
```

The TS side mirrors this correctly with interface extension:

```ts
export interface CalendarEventWithAttendees extends CalendarEvent {
  attendees: EventAttendee[];
}
```

This is the right pattern and should be the template for any future response
composition. Document it in CLAUDE.md under "Adding a new route" so the next
session reaches for it.

---

## 6. P2 — Untyped `apiClient.*()` call sites

Detail: 16 call sites use `apiClient.get/post/put/patch/delete(...)` without
an explicit `<T>` parameter. In every case checked, TypeScript infers `T`
correctly from the wrapping function's return-type annotation — so these are
*not* `Promise<unknown>` leaks at the call-site boundary. But they are
implicit, and removing the explicit `<T>` makes the trace easier to break
silently (rename the function return type → call site silently widens).

### Sample (frontend/src/api/delegation.ts)

```ts
export async function grantDelegation(data: CreateDelegationRequest): Promise<EmailDelegation> {
  return apiClient.post('/api/delegation', data);       // ← inferred <EmailDelegation>
}
export async function listDelegations(): Promise<EmailDelegation[]> {
  return apiClient.get('/api/delegation');              // ← inferred <EmailDelegation[]>
}
```

### Files with the pattern

| File | Untyped call sites |
|---|---|
| `filters.ts` | 5 (`apiClient.get/post/put/delete/post`) |
| `delegation.ts` | 4 |
| `snooze.ts` | 3 |
| `migration.ts` | 1 |
| `messages.ts` | 4 (but all are `Promise<void>` returns, safe) |

A 30-minute follow-up could add explicit `<T>` to all 16 — strictly cosmetic
*today*, but enforced via an ESLint rule (`@typescript-eslint/no-unsafe-return`
combined with `noImplicitAny: true` — already set in `tsconfig.json`) would
prevent regressions. No urgent risk; **P2**.

### `unknown` usage in interfaces (legitimate, leave alone)

`spam.ts`, `webauthn.ts`, `featureFlags.ts` use `unknown` for fields that are
genuinely opaque to TS:

- `webauthn.ts:39-40` — `attestation_object: unknown`, `client_data_json: unknown`
  — these are raw `ArrayBuffer`s round-tripped between the browser WebAuthn
  API and the backend; they cannot be statically typed at the API boundary.
- `spam.ts:16` — `custom_rules: unknown[]` — pluggable rspamd rule blobs.
- `featureFlags.ts:11` — `value: unknown | null` — feature-flag values can be
  bool/number/string/object.

These are correctly typed. The narrowing happens at the consumer end.

---

## 7. Per-area parity tables

The four areas below are the highest-traffic surfaces. Each table walks one
Rust handler → response shape → TS consumer → Dart consumer (if any) and flags
drift inline.

### 7.1 Auth (`POST /api/auth/login`, `POST /api/auth/refresh`, `POST /api/auth/signup`)

| Endpoint | Rust response | TS shape | Dart shape | Drift |
|---|---|---|---|---|
| `POST /api/auth/login` | `auth_service::TokenPair { access_token, refresh_token, expires_in }` | `TokenPair` (auth.ts:8) — matches 1:1 | `LoginResponse { accessToken, refreshToken, user }` (auth.dart:16) | **P0 — Dart expects `user`, backend doesn't return it; Dart missing `expires_in`** |
| `POST /api/auth/refresh` | `TokenPair` | same `TokenPair` | _(not modelled)_ | OK on TS; Dart unimplemented |
| `POST /api/auth/signup` | `TokenPair` (auth.rs:81) | request shape inline in `byok.ts` | _(not modelled)_ | OK on TS; Dart unimplemented |
| `GET /api/me` _(if it exists)_ | _(unverified)_ | `User { id, username, display_name?, is_admin }` (auth.ts:1) | `UserInfo { id, email, displayName, avatarUrl }` (auth.dart:36) | **DRIFT — TS has `username`+`is_admin`, Dart has `email`+`avatar_url`; neither verified against a Rust shape** |

The TS `User` and Dart `UserInfo` shapes look like two independent guesses
about what the backend returns. There is no `pub struct User` in
`backend/src/models/` (verified with grep — no matches). The actual current-user
shape needs to be located (likely `Mailbox` in `models/mailbox.rs`) and both
clients aligned. Filed as follow-up.

### 7.2 Folders & Messages (web SPA)

| Endpoint | Rust response | TS shape | Drift |
|---|---|---|---|
| `GET /api/folders` | `Vec<Folder>` (handlers/folders.rs) | `Folder { name, delimiter, messages, unseen }` (mail.ts:1) | OK |
| `GET /api/folders/{folder}/messages?page&page_size` | `MessageListResponse { messages, total, page, page_size }` | `MessageListResponse` (mail.ts:40) | OK |
| `GET /api/folders/{folder}/messages/{uid}` | `FullMessage` | `FullMessage` (mail.ts:17) | OK |
| `POST /api/folders/{folder}/messages/{uid}/move` | `()` | `Promise<void>` | OK |
| `POST /api/folders/{folder}/messages/{uid}/flag` | `()` | `Promise<void>` | OK |
| `GET /api/search` | `SearchResponse { messages, total, query, folder }` | `SearchResponse` (mail.ts:47) | OK |

The mail TS surface is one of the strongest — entity types in `types/mail.ts`,
request shapes in `api/messages.ts`. Use this as the template when lifting
other entities out of `api/` files. ✅

### 7.3 Mobile messages (mobile-only Rust → Dart)

Already covered in § 3. Key drifts:

- `MobileMessageSummary.preview` exists in Rust, missing in Dart
- `MobileMessageSummary.folder` exists in Dart, missing in Rust
- `MobileMessageDetail` body shape: needs full re-check against
  `handlers/mobile.rs::get_message` (follow-up)

### 7.4 Calendar (`/api/calendar/events`, `/api/calendar/free-busy`,
`/api/calendar/suggest-slots`)

| Endpoint | Rust response | TS shape | Drift |
|---|---|---|---|
| `GET /api/calendar/events` | `Vec<CalendarEvent>` | `CalendarEvent[]` (calendar.ts:5) | OK |
| `GET /api/calendar/events/{id}` | `CalendarEventWithAttendees` (flatten) | `CalendarEventWithAttendees extends CalendarEvent` | OK |
| `POST /api/calendar/events` | `CalendarEventWithAttendees` | same | OK |
| `PATCH /api/calendar/events/{id}` | `CalendarEvent` | same | OK |
| `DELETE /api/calendar/events/{id}` | `()` | `Promise<void>` | OK |
| `POST /api/calendar/events/{id}/rsvp` | `EventAttendee` | `EventAttendee` | OK |
| `POST /api/calendar/free-busy` | `FreeBusyResponse { attendees: AttendeeBusy[] }` | matches | OK on shape; `AttendeeBusy.status: 'resolved' | 'not_resolved'` narrowed in TS but Rust returns `String` |
| `POST /api/calendar/suggest-slots` | `SuggestSlotsResponse { slots, unresolved_attendees }` | matches | OK |

Status drifts (P2):
- `CalendarEvent.status: string` (TS) ← `String` (Rust) — should be enum on both
- `EventAttendee.rsvp: string` (TS) ← `String` (Rust) — should be enum on both
- `AttendeeBusy.status: 'resolved' | 'not_resolved'` (TS) ← `String` (Rust) — TS is tighter than Rust

---

## 8. Cross-cutting patterns

### 8.1 `serde` rename conventions in use

Catalogued via grep. Patterns in `backend/src/models/`:

| Pattern | Count | Examples |
|---|---|---|
| `#[serde(rename = "snake_string")]` on enum variants | ~40 | `webhook.rs:12-27`, `dlp_rule.rs:13-35`, `chat_integration.rs:14-22`, `spam.rs:14-21` |
| `#[serde(rename_all = "lowercase")]` on enum | ~8 | `dav_config.rs:15`, `imap_config.rs:13`, `smtp_config.rs:15`, `plugin.rs:10`, `activesync.rs:11`, `push_notification.rs:11`, `deliverability.rs:7` |
| `#[serde(rename_all = "snake_case")]` on enum | ~3 | `ai_config.rs:110`, `plugin.rs:42`, `sync.rs:11` |
| `#[serde(skip_serializing)]` (secret field) | ~3 | `oidc_provider.rs:15`, `ldap_config.rs:15`, `migration_job.rs:15` |
| `#[serde(skip_serializing_if = "Option::is_none")]` | ~5 | `mobile.rs:25`, `push_notification.rs:112,115` |
| `#[serde(flatten)]` for response composition | 3 | `calendar_event.rs:92`, `ediscovery.rs:134`, `distribution_group.rs:55` |
| `#[serde(tag = "type")]` for discriminated unions | 1 | `mobile.rs:65` (`SyncChange`) |
| `#[serde(default)]` for backward-compat new fields | ~15 | `imap_config.rs:70-75`, `mailbox.rs:19-33`, `email_queue.rs:29-42`, etc. |
| `#[serde(alias = "low_bw")]` for legacy short field name | 1 | `mobile.rs:103,114` |

### 8.2 TS conventions in use

- `snake_case` field names throughout (matches Rust default) — ✅ no `camelCase` ↔ `snake_case` translator middleware needed
- Date fields are typed as `string` (ISO 8601) on the TS side; Rust uses
  `DateTime<Utc>` which serializes to ISO 8601 by default — ✅
- UUIDs typed as `string` on TS side, `Uuid` on Rust — ✅
- Numeric fields typed as `number` regardless of Rust type
  (`i32`/`u32`/`i64`/`f64`) — note that `i64` values > 2^53 will silently
  lose precision on the TS side. Not a current concern because no money-cents
  field crosses that threshold, but worth documenting if billing ever moves
  away from `String` cents → `i64` cents.

### 8.3 Dart conventions in use

- `camelCase` Dart field names with explicit `snake_case` keys in
  `fromJson`/`toJson` — ✅ idiomatic
- Numeric coercion via `(json['x'] as num?)?.toInt() ?? 0` — defensive,
  handles JSON number-as-double edge cases — ✅
- Date fields parsed via `DateTime.tryParse(... as String)` — ✅ matches Rust
  ISO 8601 emit

The Dart conventions are healthy. The problem is the **count** of Dart
models, not the quality.

---

## 9. Recommended follow-ups (file as child tasks)

Ordered by ROI.

| # | Task | Severity | Effort | Outcome |
|---|---|---|---|---|
| F1 | **Fix `mobile/lib/models/auth.dart::LoginResponse`** — drop `user` field, add `expiresIn`. Add a follow-up call to `GET /api/me` after login | **P0** | 1 hour | Mobile login stops crashing |
| F2 | **Identify the current-user response shape on the backend** — locate or create `GET /api/me`, define a single `UserSummary` struct in Rust, mirror in TS `types/auth.ts` and Dart `UserInfo` | **P0** | 2 hours | TS `User`/Dart `UserInfo` realign on a Rust source of truth |
| F3 | **Mobile model strategy decision** — pick between hand-rolled, `freezed`+codegen, or generated-from-OpenAPI. Document in `docs/MOBILE-MODEL-STRATEGY.md` | **P1** | 4-hour spike | Unblock mobile screens (TMAIL-152, TMAIL-153) without further drift |
| F4 | **Lift 28 entity types from `api/*.ts` into `types/`** — `CalendarEvent`, `EmailDelegation`, `SieveRule`, etc. Mechanical move + import-path update | **P1** | 1 day | Components stop importing from `api/` |
| F5 | **Audit `types/groups.ts` vs `api/groups.ts` for dup/drift** | **P1** | 30 min | One concept, one file |
| F6 | **Convert `String` fields to typed enums** in Rust models — `CalendarEvent.status`, `EventAttendee.rsvp`, `SieveRule.field/operator/action_type`, `EmailDelegation.delegation_type`. Match the TS literal unions already in place | **P2** | 1 day | Source of truth becomes the narrow one |
| F7 | **Extend `scripts/trace-check.py` from route-presence to response-shape** — emit an OpenAPI fixture from the Rust handlers, diff against TS/Dart models in CI | **P1 strategy, P2 build** | 1 week | Drift becomes statically impossible |
| F8 | **Add explicit `<T>` to the 16 untyped `apiClient.*()` call sites** + enable `@typescript-eslint/no-unsafe-return` | **P2** | 1 hour | Future renames can't silently widen types |
| F9 | **Verify `MobileMessageDetail` field-by-field against `handlers/mobile.rs::get_message`** | **P2** | 30 min | Confirm `body_html`/`body_text` ordering and attachment handling |
| F10 | **Remove `MobileMessageSummary.folder` from Dart** (or verify it's injected client-side from the request path) | **P2** | 15 min | Mobile inbox stops casting null → String |

F1, F2, F3 are the only ones that materially affect users. The rest are
hygiene that improves the **next** report's signal-to-noise ratio.

---

## 10. Source references

Every claim in this report is grounded in a file:line. Key anchors:

**Rust source of truth:**
- `backend/src/services/auth_service.rs:61-65` — `TokenPair`
- `backend/src/handlers/auth.rs:27-62` — `login` returns `Json<TokenPair>`
- `backend/src/models/mobile.rs:16-27` — `MobileMessageSummary` (no `folder` field)
- `backend/src/models/calendar_event.rs:10-95` — Calendar event shape + `#[serde(flatten)]`
- `backend/src/models/sieve_rule.rs:8-24` — `RuleCondition`/`RuleAction` with `String` fields
- `backend/src/models/ai_config.rs:18-30` — exemplary enum with `#[serde(rename)]`
- `backend/src/models/webhook.rs:12-27` — exemplary enum
- `backend/src/models/mobile.rs:65-86` — exemplary `#[serde(tag = "type")]` discriminated union

**TS consumer:**
- `frontend/src/api/client.ts:100-121` — `post/put/patch/delete<T>` generic API
- `frontend/src/types/auth.ts:1-18` — `User`, `TokenPair`, `LoginRequest`
- `frontend/src/types/mail.ts:1-62` — exemplary canonical type module
- `frontend/src/api/calendar.ts:5-165` — inline-rich, narrowed unions
- `frontend/src/api/filters.ts:3-66` — narrowed unions vs Rust `String`
- `frontend/src/api/delegation.ts:5-39` — untyped call sites

**Dart consumer:**
- `mobile/lib/models/auth.dart:16-34` — broken `LoginResponse`
- `mobile/lib/models/email.dart:5-160` — message + folder models
- `mobile/lib/models/sync_checkpoint.dart:6-53` — healthy model template

**CI / process:**
- `scripts/trace-check.py` — current presence-only drift gate
- `docs/traceability/orphans-baseline.json` — allowed orphans baseline
- `docs/assessments/INDEX.md` — parent index, including the TMAIL-257/262 duplicate marker

---

## 11. Method

1. Pulled every `*.rs` file under `backend/src/models/` and
   `backend/src/handlers/` and every `*.ts` under `frontend/src/api/` and
   `frontend/src/types/` and every `*.dart` under `mobile/lib/models/`.
2. Cross-tabulated handlers ↔ API modules by name (handler `foo.rs` →
   `api/foo.ts` or `api/foo-bar.ts`); manually triaged 13 unpaired files (8
   intentional, 5 renamed).
3. Counted `Promise<any>` and `Promise<unknown>` returns in `api/*.ts` (zero
   of each).
4. Cross-referenced each Dart model against its Rust counterpart field-by-field.
5. Grepped `#[serde(...)]` attributes across `models/` to catalogue rename
   conventions.
6. Read four representative areas in full (auth, calendar, mobile messages,
   filters/sieve) to confirm the inline-vs-canonical pattern claims.
7. Filed findings as P0 (ship-blocker), P1 (load-bearing pre-GA), P2 (cleanup).

No code was changed by this report. The accompanying commit lands only this
document and an updated `docs/assessments/INDEX.md` row. Follow-up tasks F1–F10
are filed against the PM separately.
