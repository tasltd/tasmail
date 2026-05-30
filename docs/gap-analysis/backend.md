# TASMail Backend Gap Analysis (Rust/Axum)

**Date:** 2026-05-30
**Scope:** `backend/` crate — route surface, handler completeness, schema↔model drift, auth coverage, IMAP/SMTP completeness, frontend↔backend traceability
**Owner of this report:** TMAIL-297 (parent task; one child task per P0/P1 row below, all queued for auto-fix)
**Methodology:** Walked `backend/src/{router,handlers,services,models,middleware}.rs`, the 75 `backend/migrations/*.sql` files, and the SPA's `frontend/src/api/*` + `themes/shadcn-prototype/src/api/*` modules. Cross-referenced against the existing `docs/traceability/orphans-baseline.json` and `npm run trace-check` output.

---

## Executive Summary

The backend is **broad** (122 mounted routes across 65+ handler modules, 75 migrations, ~180 Rust source files) and the wiring is mostly clean: there are **zero `todo!()` / `unimplemented!()` / `panic!()` markers** in handlers and services, and only three benign `TODO/FIXME/XXX` comments. The trace-check gate currently passes (42 baselined orphans, no drift).

What it is **not** is feature-complete:

* **6 P0 gaps** are *silent functional defects* — endpoints return 2xx (or appear to) but produce nothing usable: scheduled-send tries to SMTP-AUTH with the literal string `"placeholder"` as password, the WebSocket pushes no mail events, SAML/OIDC callbacks never issue a JWT, `mailbox/provision` writes an IMAP row whose password is the string `REPLACE_ME_WITH_DOVEADM_GENERATED_PASSWORD`, and `/api/mobile/batch` returns `{pending: true}` for every sub-request instead of dispatching anything.
* **8 P1 gaps** are real-but-degradable: audit-log coverage is essentially admin-quote + auth-only (zero coverage for user/domain/payment-provider CRUD), CORS only accepts a single origin string (breaks dev + prod cohabitation), `auth_middleware`'s `set_rls_context` is a deliberate no-op (RLS is on in DB but unenforced by middleware — relies entirely on per-handler `WHERE` discipline), `/api/health` only pings the DB, and several BYOK migration follow-ups are still pending (`TMAIL-156`).
* **P2 / P3** are mostly hardening, observability, and ergonomics items.

The cumulative result: TASMail can run the read-mail / send-mail / login (password) golden path on the current `mail.techatscale.io` BYOK deployment, but **federated SSO, real-time mail push, scheduled send, managed-mailbox provisioning, and mobile batch sync are non-functional** until the P0 fixes land.

---

## Endpoint Inventory

| Group | Routes | Notes |
|---|---|---|
| Auth + signup | 4 | `/api/auth/{login,signup,refresh,logout}` — login + signup + refresh rate-limited (10/60s/IP). |
| OIDC + SAML | 8 | List/authorize/callback for both. **Callback handlers do not finish login** (see P0-3, P0-4). |
| IMAP / messages | 16 | Folders list, message CRUD, send, drafts, search, move/flag, EML+MBOX, comments. |
| Drafts / scheduled / snooze | 7 | `/api/messages/{schedule,scheduled,snooze}`. Schedule POST persists but **scheduler can't send** (P0-1). |
| Calendar + iMIP + free-busy + CalDAV public tokens | 9 | `/api/calendar/*` — fully wired; public RSVP + iMIP accept exist. |
| Contacts + groups + vCard + CSV | 12 | Full CRUD + import/export/merge. |
| Templates + Sieve filters + tasks + auto-reply | 12 | CRUD + render + reorder + test sandbox. |
| Attachments + shared-files + EML/MBOX export | 7 | Public `/api/dl/{token}` is the only unauthenticated downloader. |
| AI: BYOK config + summarize + smart-reply + thread-summary + compose | 8 | All gated by AI config row. |
| Search: text + semantic (pgvector) + NLP + eDiscovery | 9 | NLP search + history wired; semantic search indexed via background ingest. |
| 2FA (TOTP) + SMS-OTP + WebAuthn | 14 | All three factors present. |
| Push (FCM/APNs) + quiet hours + badge sync | 5 | Token CRUD + quiet hours; **dispatch path is wired**, but ties into WebSocket gap. |
| Sync checkpoints (mobile) | 3 | `/api/sync/{checkpoint,resolve-conflict}` — mobile-only consumers. |
| Mobile API (offline-aware) | 7 | Inbox/message/folders/unread/usage + batch + sync. **Batch is a stub** (P0-6). |
| Migration: IMAP / MBOX / PST | 7 | IMAP + MBOX functional; PST has worker + 1 failing-fast path. |
| Admin: users / domains / branding / retention / legal-holds / hostnames / quote-requests / feature-flags / bulk-import / payment-providers / audit | 30 | Domain CRUD wired; **most admin actions skip audit_log** (P1-1). |
| Admin: LDAP/AD + SAML config + OIDC providers + DLP + DANE + Ollama + ActiveSync + archive + activesync policies | 30 | Each has list/CRUD + test/sync where relevant. |
| Billing: plans / subscribe / subscription / payments / webhooks (Paystack, Mastercard) / usage / invoices / quote-requests | 9 | Paystack + Mastercard webhooks publicly mounted; credentials read from `payment_provider_config`. |
| Spam (Rspamd) + queue (per-user) + quota + warmup + deliverability + cache | 14 | Spam + queue management + admin queue stats wired. |
| BYO-IMAP + BYO-SMTP configs + POP3 + CalDAV/CardDAV + ActiveSync devices + plugins + DAV + chat integrations + webhooks (outbound) | ~35 | Per-user secret CRUD; encryption via JWT-derived AES key. |
| Public infra: `/api/health`, `/metrics`, `/api/branding`, `/api/feature-flags`, `/api/enterprise/quote-request`, `/api/dl/{token}`, `/ws`, `/api/calendar/public/{token}/*` | 9 | Health is DB-only (P2). |

**Total mounted routes:** 122
**Total handler modules:** 65 (+ `admin/` submodule with 7)
**Total service modules:** 38
**Total migrations:** 75 (latest: `075_shared_mailbox_acl_rls_align.sql`)
**Background workers wired from `main.rs`:** `email_scheduler` (5s tick), `queue_processor` (5s, batch=50, workers=4), `billing_rollup` (daily by default).

---

## P0 — Critical (silent functional defects, must fix before next beta cycle)

### P0-1 — Scheduled-send SMTP-AUTHs with the literal string "placeholder"
* **Files:** `backend/src/services/email_scheduler.rs:67-92` (specifically L86-89)
* **Symptom:** Every email scheduled via `POST /api/messages/schedule` is enqueued into `scheduled_emails`. The 5s-tick scheduler picks it up and calls `smtp.send(&mailbox.username, "placeholder", &request)`. SMTP servers reject the login → `mark_failed`. The user sees their scheduled email never arrive and a `failed` row in `/api/messages/scheduled`.
* **Why it's P0:** Scheduled-send is a feature in the UI (`scheduled.ts` API module + classic SPA + alt-UI). It is fully broken in production.
* **Proposed fix:** Mirror `queue_processor`'s BYOK pattern — load the per-user `smtp_configurations` row, decrypt with `EncryptionService::from_jwt_secret`, pass the real password. Bonus: collapse `scheduled_emails` into `email_queue` so there's a single delivery pipeline (deferred to a follow-up).
* **PM child title:** `[Backend][P0] Scheduled-send uses literal "placeholder" password — broken delivery`

### P0-2 — WebSocket `/ws` pushes no real-time mail events
* **Files:** `backend/src/handlers/websocket.rs:74-119`
* **Symptom:** The `idle_poll` `tokio::interval` fires every 10s but its body is an empty comment block ("In production, this would listen to IMAP IDLE…"). The only thing the WS sends is a 30s heartbeat ping. SPA features that subscribe to `new_mail`, `unread_update`, `quota_update` events never receive them; the SPA falls back to TanStack-Query refetch on focus, which on slow networks looks like "no push at all".
* **Why it's P0:** The PRD lists WebSocket push as a core differentiator. Mobile/desktop UX is materially degraded.
* **Proposed fix:** Wire an IMAP IDLE bridge (one persistent `async-imap` session per connected client, with subscribe/unsubscribe per folder) and pump `WsEvent::NewMail` / `WsEvent::UnreadUpdate` into the WS sender. Reuse `ImapService::for_user` so it stays BYOK. Cap concurrent IDLE sessions per user (default 3).
* **PM child title:** `[Backend][P0] WebSocket /ws emits no new_mail/unread events — IMAP IDLE bridge missing`

### P0-3 — SAML callback never issues a JWT
* **Files:** `backend/src/handlers/saml.rs:130-177`
* **Symptom:** The callback parses `SAMLResponse`, creates a `SamlSession` row, and returns `{message: "...would occur here.", name_id, session_index}` — explicitly tagged "placeholder response". No user lookup, no JWT issuance, no Set-Cookie. The SAML round-trip completes in the IdP but the SPA receives nothing it can use to authenticate the user.
* **Why it's P0:** SAML SSO is sold as part of the Enterprise tier (single-tenant + SAML/OIDC).
* **Proposed fix:** After the SAML XML is parsed (and signature verified), find-or-create the user in `mailboxes` (auto-create flag from `SamlConfiguration`), then call `auth_service::issue_token_pair` and return the same `{access_token, refresh_token, mailbox}` payload as `/api/auth/login` does. Add SLO support via the existing `SamlSession` row.
* **PM child title:** `[Backend][P0] SAML callback returns placeholder — does not issue JWT or create user`

### P0-4 — OIDC callback returns `400 "not yet implemented"`
* **Files:** `backend/src/handlers/oidc.rs:162-193`
* **Symptom:** After validating that `code` and `state` are non-empty, the handler returns `Err(AppError::BadRequest("OIDC callback token exchange not yet implemented..."))`. Sign in with Google / Microsoft is non-functional even though `/api/admin/oidc` + `/api/auth/oidc/{id}/authorize` + `/api/auth/oidc/providers` are all wired.
* **Why it's P0:** OIDC is the primary SSO path for non-enterprise customers (free-tier users with Google Workspace). It's also the only Sign-in-with-Google integration.
* **Proposed fix:** Implement steps 1-6 from the in-code comment: validate `state`, exchange `code` at the provider's `token_endpoint`, validate `id_token` via JWKS, extract `sub`+`email`, find-or-create `OidcUserLink` + user, issue JWT pair. Use `reqwest` (already a dep) for the token + JWKS calls.
* **PM child title:** `[Backend][P0] OIDC callback returns 400 — token exchange not implemented`

### P0-5 — `mailbox/provision` writes IMAP row with literal "REPLACE_ME…" password
* **Files:** `backend/src/handlers/mailbox_provision.rs:84-129`
* **Symptom:** When the feature flag `dns_mx_onboarding_enabled` is on and the two `TASMAIL_MANAGED_*` env vars are set, the endpoint logs a warning ("would `doveadm user add` … but SSH integration is not wired yet") and inserts an `imap_configurations` row whose stored (encrypted) password is the string `REPLACE_ME_WITH_DOVEADM_GENERATED_PASSWORD`. The user is told their mailbox is provisioned but cannot log in.
* **Why it's P0:** The DNS-MX onboarding tile in the wizard drives users straight into this code path. Better to 503 with "managed onboarding not wired yet" than to write a bogus row.
* **Proposed fix:** (a) Short-term: keep the env-var + feature-flag gating but **don't write the IMAP row** until the doveadm bridge exists — return `503 Service Unavailable`. (b) Long-term: implement the `ssh user@managed_dovecot doveadm user add …` + `doveadm pw -p <random32>` call, then write the row with the freshly generated password. Track the SSH/doveadm side under a follow-up child task.
* **PM child title:** `[Backend][P0] mailbox/provision writes IMAP row with REPLACE_ME password — fail-closed needed`

### P0-6 — `/api/mobile/batch` returns `{pending: true}` for every sub-request
* **Files:** `backend/src/handlers/mobile.rs:280-341`
* **Symptom:** After validating the batch envelope (max 50 requests, allowed methods, path prefix), the handler maps each sub-request to a fake `BatchResponseItem { status: 200, body: { message: "...acknowledged", pending: true } }` — no internal dispatch occurs. The Flutter mobile app's offline-flush flow uses this endpoint to flush queued mutations; nothing actually persists.
* **Why it's P0:** The mobile app is part of the v1.0 ship (Flutter, see `MOBILE-PLATFORM-DECISION.md`). This endpoint sits in the offline-write path.
* **Proposed fix:** Use `axum::Router::oneshot()` (or an in-process `tower::Service` call) to dispatch each sub-request through the same router instance — preserving the user's auth context. Reject batches that mix methods on the same resource without ordering.
* **PM child title:** `[Backend][P0] /api/mobile/batch returns placeholder responses — internal dispatch missing`

---

## P1 — High (degrades safety, compliance, or DX; should land in current sprint)

### P1-1 — Admin actions do not write audit_log entries
* **Files:** `backend/src/handlers/admin/users.rs`, `admin/domains.rs`, `admin/payment_providers.rs`, `admin/feature_flags.rs`, plus retention/legal-holds, custom hostnames, branding, LDAP, SAML, OIDC, DLP, DANE, archive, ActiveSync policies. Only `admin/quote_requests.rs` (1 call) and `ediscovery.rs` (1 call) currently call `AuditLog::record` outside of `auth.rs` + `auth_service.rs`.
* **Symptom:** `audit_log` table has only login/logout/refresh, eDiscovery executions, and quote-request state changes. Admin actions like "delete user", "rotate payment provider key", "release legal hold", "update branding", and "toggle feature flag" leave no audit trail. Compliance / forensics gap.
* **Why P1:** The eDiscovery + DPC registration runbooks both assume admin actions are auditable. Without this, "who deleted user X on Y date" is unanswerable.
* **Proposed fix:** Add `AuditLog::record(&state.db, &claims, action, resource_type, resource_id, details, req_ip, req_ua)` at the end of every state-changing admin handler. Centralise via a small `audit_admin_action!(claims, "domain.delete", id, json!({}))` macro to keep call sites uniform.
* **PM child title:** `[Backend][P1] Admin actions skip audit_log — compliance trail incomplete`

### P1-2 — CORS only accepts a single origin string
* **Files:** `backend/src/router.rs:25-35`
* **Symptom:** `AllowOrigin::exact(allowed_origin.parse().unwrap_or_else(...))` only takes one origin. In production we already need both the classic SPA origin (`https://mail.techatscale.io`) and, in dev, both `http://localhost:5173` (Vite) + sometimes the alt-UI when served from a different port. Operators have been working around by setting one and rebuilding.
* **Why P1:** Hampers dev experience and any multi-domain rollout (e.g., when custom-hostname tenants come online via `/api/admin/hostnames`).
* **Proposed fix:** Parse `CORS_ORIGIN` as comma-separated, use `AllowOrigin::list(...)`. Add a regex/wildcard mode for `*.tenants.tasmail.io` if/when needed for custom-hostname customers.
* **PM child title:** `[Backend][P1] CORS_ORIGIN only accepts single origin — needs comma-separated list`

### P1-3 — `auth_middleware` no longer sets RLS session vars
* **Files:** `backend/src/middleware/auth.rs:47-70`
* **Symptom:** `set_rls_context` is intentionally a no-op (TMAIL-161 comment). All RLS migrations are still in place (`061`, `064`, `075`, etc.) but the middleware no longer SETs `app.mailbox_id` / `app.is_admin`. Tenant isolation relies *entirely* on per-handler `WHERE user_id = $N` discipline — one missing WHERE = tenant leak.
* **Why P1:** Defense-in-depth is gone. If any future handler forgets the WHERE filter, the RLS net no longer catches it. Compliance posture is weakened relative to what the migrations advertise.
* **Proposed fix:** Use `acquire_with_rls` (already exists in `services::db_session`) as the *only* way handlers get a DB connection, and add a `Drop`-based or middleware-based guard that asserts `app.mailbox_id` is set when the handler completes. Alternative: a `tower` layer that acquires + holds a transaction-scoped connection per request, sets the session vars, runs the handler, commits.
* **PM child title:** `[Backend][P1] RLS context no longer set by middleware — defense-in-depth gone`

### P1-4 — `/api/health` only pings DB; no IMAP / SMTP / Redis / queue probes
* **Files:** `backend/src/handlers/health.rs`
* **Symptom:** Returns `{status: healthy, database: connected}` even when Redis is down (graceful-degrade), the queue processor is stuck, or the configured Dovecot is unreachable. The proxy / uptime monitor sees green when the user-visible system is red.
* **Why P1:** Operations playbook (`BETA-LAUNCH-RUNBOOK.md`) calls for a richer health check for the morning weather report.
* **Proposed fix:** Add liveness vs readiness split. Liveness = `SELECT 1`. Readiness = DB + Redis (if configured) + queue-processor heartbeat (last-tick timestamp) + a `Mailbox::count_active` query. Return structured per-component status. Keep the existing endpoint shape for back-compat under a `?detail=full` flag.
* **PM child title:** `[Backend][P1] /api/health is DB-only — add Redis + queue + IMAP probes`

### P1-5 — Remaining IMAP handlers still pending BYOK migration (TMAIL-156 follow-up)
* **Files:** `backend/src/services/imap_service.rs` legacy paths + any handler still calling `ImapService::new(state.config.imap.clone())` (none active outside tests, but the constructor still exists and is publicly callable from new code).
* **Symptom:** TMAIL-156 is marked In Review. All known callers in `handlers/` are already on `ImapService::for_user`, but the **legacy global path is still public API** of the service, and future contributors will accidentally use it.
* **Why P1:** Without removal, the BYOK guarantee is structural, not enforced.
* **Proposed fix:** Make `ImapService::new` `pub(crate)` and only callable from tests, OR remove it entirely (the tests in `imap_service.rs` exist *specifically* to assert the legacy path errors out on `connect_user`, so they can be reworked once the function is gone). Update `main.rs` (which still keeps `config.imap` for back-compat) to remove the field unless something else needs it.
* **PM child title:** `[Backend][P1] Remove legacy ImapService::new global path — finish TMAIL-156`

### P1-6 — `email_scheduler` and `queue_processor` are two parallel delivery pipelines
* **Files:** `backend/src/services/email_scheduler.rs` + `backend/src/services/queue_processor.rs` + `backend/src/main.rs:71-97`
* **Symptom:** Two background tasks poll two different tables (`scheduled_emails` vs `email_queue`) and dispatch via different code paths. `queue_processor` is BYOK-aware and instrumented. `email_scheduler` is the broken-password one from P0-1. The scheduled-send handler writes to `scheduled_emails`, every other send path writes to `email_queue`. There is no reason to maintain two pipelines.
* **Why P1:** Even after P0-1 is patched, having two pipelines duplicates retry logic, metrics, and operational surface.
* **Proposed fix:** Migrate `POST /api/messages/schedule` to enqueue into `email_queue` with `next_retry_at = scheduled_at`, retire `scheduled_emails` table (or keep it as a view over `email_queue` filtered by `scheduled_at IS NOT NULL`), and remove the `email_scheduler` background task. Add a migration that backfills any unsent `scheduled_emails` rows into `email_queue`.
* **PM child title:** `[Backend][P1] Collapse email_scheduler into queue_processor — single delivery pipeline`

### P1-7 — `webhook_dispatcher` retry strategy not visible from API; no rotate-secret endpoint
* **Files:** `backend/src/services/webhook_dispatcher.rs` + `backend/src/handlers/webhooks.rs`
* **Symptom:** `/api/webhooks/{id}/deliveries` lists deliveries but there is no `POST .../redeliver` to manually replay a failed delivery, and no `POST .../rotate-secret` to rotate the HMAC signing secret. Customers integrating with their CRM ask for both.
* **Why P1:** Webhook ops without redelivery means every failure becomes a support ticket.
* **Proposed fix:** Add `POST /api/webhooks/{id}/deliveries/{delivery_id}/redeliver` (re-runs the same payload, new delivery row) and `POST /api/webhooks/{id}/rotate-secret` (writes a new HMAC secret, returns it once). Audit-log both.
* **PM child title:** `[Backend][P1] Webhook redelivery + secret rotation endpoints missing`

### P1-8 — `/metrics` Prometheus endpoint is public (no auth, no IP allowlist)
* **Files:** `backend/src/router.rs:98` (sits in `public_routes`)
* **Symptom:** Anyone on the public internet can scrape `https://mail.techatscale.io/metrics` and see per-handler latency histograms, queue depths, error rates, internal label cardinality. Information leak.
* **Why P1:** Both an info-leak and a DoS amplifier (the response is large).
* **Proposed fix:** Either (a) require Basic auth with a token from env `METRICS_TOKEN`, (b) gate to an env-defined IP allowlist (`METRICS_ALLOWED_IPS`), or (c) move behind `auth_middleware` and require `is_admin = true`. Prefer (b) since Prom scrapers don't always carry headers.
* **PM child title:** `[Backend][P1] /metrics is publicly scrapeable — add auth/IP allowlist`

---

## P2 — Medium (improves robustness, observability, ergonomics)

| ID | Title | File(s) | Proposed fix (1 line) |
|---|---|---|---|
| P2-1 | WebSocket has no per-user connection cap or active-connection gauge | `backend/src/handlers/websocket.rs` | Wrap `handle_socket` in a `Semaphore` per `user_id`; emit a `tasmail_ws_active_connections` gauge. |
| P2-2 | `mobile_batch` validates allowed methods but not allowed path patterns | `backend/src/handlers/mobile.rs:316-322` | Replace prefix-startswith check with a per-method route allowlist (e.g. POST only to `/api/folders/{folder}/messages/{uid}/flag\|move`). |
| P2-3 | No `POST /api/messages/scheduled/{id}` for editing a scheduled email | — | Add route; `scheduled.ts` SPA module currently lacks an edit affordance. |
| P2-4 | DANE `list_verifications` orphan (no SPA `useQuery` ever fires it) | `backend/src/handlers/dane.rs` + `frontend/src/components/settings/DaneManager.tsx` | DaneManager imports types but never reads verifications; wire a `useQuery(['dane','verifs'])`. |
| P2-5 | `audit_log.list_audit_logs` has no CSV export | `backend/src/handlers/admin/audit.rs` | Add `GET /api/admin/audit-log/export?format=csv` (auditor friendly). |
| P2-6 | Push devices: no batch send-test API for ops | `backend/src/handlers/push.rs` | Add `POST /api/admin/push/broadcast` (admin only) with rate cap. |
| P2-7 | `/api/quota/sync` is a one-shot — no schedule | `backend/src/handlers/quota.rs` | Add a periodic background sync per-user; emit Prom gauge. |
| P2-8 | Pre-existing scheduled-email rows: no UI to surface "failed" status reason | — | Surface `error_message` field on `/api/messages/scheduled` list. |
| P2-9 | No `GET /api/auth/me` (current-user introspection) | — | Add lightweight endpoint that returns the validated claims + mailbox row. |
| P2-10 | DLP scanner uses regex-only patterns; no contextual ML scoring | `backend/src/services/dlp_scanner.rs` | Add optional Ollama-backed scoring tier. |

---

## P3 — Low (nice-to-have, cosmetic, documentation)

| ID | Title | File(s) | Proposed fix (1 line) |
|---|---|---|---|
| P3-1 | Three benign `TODO/XXX` comments in `imap_service.rs` + `dlp_scanner.rs` | as named | Resolve or convert to issue references. |
| P3-2 | `imap_service.rs` legacy `fake_global_config()` helper for tests | `backend/src/services/imap_service.rs:1052` | Replace with `#[cfg(test)] mod fixtures` to make intent obvious. |
| P3-3 | `set_rls_context` keeps unused locals to silence dead-code warning | `backend/src/middleware/auth.rs:68` | Either delete or properly inject into request extensions for handler-side use. |
| P3-4 | `/api/feature-flags` (public subset) has no caching headers | `backend/src/handlers/admin/feature_flags.rs::list_public_flags` | Add `Cache-Control: max-age=30, public`. |
| P3-5 | `enterprise_quote::submit_quote_request` has no per-IP rate limit (relies on global) | `backend/src/handlers/enterprise_quote.rs` | Apply the existing `RateLimiter` layer to this route specifically. |
| P3-6 | `health_check` does not return commit SHA, only `CARGO_PKG_VERSION` | `backend/src/handlers/health.rs` | Inject via `option_env!("GIT_SHA")`. |
| P3-7 | `branding.update_branding` accepts any image URL without size/MIME validation | `backend/src/handlers/branding.rs` | Validate via `image::guess_format` + size limit. |
| P3-8 | Trace-check baseline names `auth: false` for routes that actually require auth (script bug, not backend) | `scripts/trace-check.py` | Rename the field to `requires_auth_header` in the baseline so it's not misleading. |

---

## Schema ↔ Model drift

Spot-checked the latest 15 migrations against the corresponding `models/`:

* `073_account_lockout.sql` → `models/mailbox.rs` has `failed_login_attempts` + `last_failed_login_at` + lockout fields — wired into `auth_service.rs::evaluate_lockout` ✅
* `074_event_attendees_rsvp_to_text.sql` → `models/calendar_event.rs` reads `rsvp_status` as `String` ✅
* `075_shared_mailbox_acl_rls_align.sql` → `models/shared_mailbox.rs` is RLS-aligned ✅
* `069_ediscovery_compliance.sql` → `models/ediscovery.rs` covers all columns ✅
* `070_email_summary_cache.rs` → `models/email_summary_cache.rs` matches ✅

No drift found in the sample. The ENUM→TEXT+CHECK conversions (`061`, `063`, `065`, `074`) are all consistently reflected in the corresponding model `String` fields.

---

## Frontend ↔ Backend traceability summary

`npm run trace-check` reports **42 orphans, baseline matches** — no drift. The 42 baselined orphans break down as:

* **Intentional / mobile-only:** 8 `/api/mobile/*` routes + 4 `/api/sync/*` routes (consumed by the Flutter app, not the SPA) — correct to be orphans.
* **Webhook callbacks:** Paystack + Mastercard webhooks + SAML callback + OIDC callback + `/api/dl/{token}` — intentionally no SPA consumer.
* **Background-only:** `/metrics`, `/api/health`, `/api/admin/queue-stats` — intentional.
* **Public read-only:** `/api/calendar/public/{token}*` (pre-auth scheduling) — intentional.
* **False positives (dynamic import):** `/api/messages/scheduled`, `/api/folders/{folder}/{import-eml,export-mbox}`, `/api/folders/{folder}/messages/{uid}/eml`, `/api/contacts`, `/api/calendar/events`, `/api/dane/verifications` — all consumed by the SPA, the static scan misses them (mostly via `background-sync.ts` dynamic imports or via `useQuery` hooks the scanner doesn't recognise).
* **Real orphans:** `/api/mailbox/provision` (only the wizard tile would consume it, see P0-5; UI tile present, integration pending), `/api/folders` (GET — this one's odd, see TMAIL-292 trace-check fixes).

**Conclusion:** the trace-check signal is clean for *gap* purposes; orphans don't translate to missing backend coverage.

---

## Methodology footnotes

* All `grep -rn "todo!()|unimplemented!()|panic!("` across `backend/src/handlers/` + `backend/src/services/` returned **zero matches**. The placeholder-density argument above comes from explicit `// NOTE: ... For now, return a placeholder ...` comments, of which there are 7 in handlers.
* `cargo build` was **not** run as part of this audit (per the rule: don't deploy/build unless asked). The findings above are derived from source inspection.
* Cross-checked against `docs/GAP-ANALYSIS.md` (dated 2026-03-22, mobile-focused). That report covers product/strategy gaps; this report covers backend implementation gaps. They are complementary.
* PM mirror: child tasks for every P0 + P1 row are created under TMAIL-297 and queued for auto-fix in the same session that produces this report (see commit history + auto-fix queue).

---

**End of report. Next action:** child PM tasks (one per P0/P1) are created against this report and immediately queued for auto-fix.
