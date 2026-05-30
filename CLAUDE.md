# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

TASMail is a **webmail UI for any IMAP/SMTP server** (BYOK — bring your own key).
Users sign up for a TASMail account, then attach the credentials of their existing
mail server (Gmail, Outlook, Zoho, FastMail, ProtonMail Bridge, an existing Dovecot,
corporate Exchange, etc.) in the onboarding wizard. TASMail never stores email — it
proxies IMAP/SMTP for the browser using the user's encrypted credentials.

Stack: React 19 SPA frontend, Rust/Axum backend, Flutter mobile app, PostgreSQL.
GitHub: `tasltd/tasmail`. TASCIM PM project: `TMAIL`. Live at `https://mail.techatscale.io`.
External-facing overview lives in `README.md` at the repo root — keep it in sync with the positioning above when the pitch changes.

The repo also ships an **optional** Postfix/Dovecot installer (`deploy/scripts/setup-all.sh`)
for operators who want to run their own mail server alongside TASMail. That setup is
**not** wired into mail.techatscale.io — see `docs/SELF-HOST-MAIL-SERVERS.md` for the
deferred-install rationale and instructions.

## Build & Run Commands

### Backend (Rust/Axum) — from `backend/`
```bash
cargo build                    # Build
cargo run                      # Run (listens on :3000 by default)
cargo test                     # Run all tests
cargo test config::tests       # Run specific test module
cargo test test_config_from    # Run single test by name
cargo clippy                   # Lint
```

### Frontend (React/Vite) — from `frontend/`
```bash
npm install                    # Install dependencies
npm run dev                    # Dev server on :5173 (proxies /api to :3000)
npm run build                  # Type-check + production build (tsc -b && vite build)
npm run lint                   # ESLint
npm run test                   # Vitest (single run)
npm run test:watch             # Vitest (watch mode)
npx vitest run src/api/client.test.ts  # Run a single test file
npm run e2e                    # Playwright E2E tests (Firefox)
npm run e2e:headed             # Playwright E2E (headed Firefox)
npm run e2e:report             # Open last Playwright HTML report
npm run trace-check            # Run traceability drift gate locally (same as CI)
npm run trace-check:update     # Refresh docs/traceability/orphans-baseline.json
npm run build:alt-ui           # Build themes/shadcn-prototype into frontend/public/modern/
```

### Database
PostgreSQL 16+. Default dev connection: `postgres://tasmail:tasmail@localhost/tasmail`. Migrations run automatically on backend startup via `sqlx::migrate!("./migrations")` — see `backend/migrations/` for the latest files (numbered `NNN_description.sql`, sequential). Recent themes: feature flags (`057`), usage-based billing (`058`), enterprise quote requests (`059`), a series of FK-cascade + Postgres ENUM→TEXT-with-CHECK conversions (`060`–`065`) so sqlx can decode the columns as `String` in the Rust models, email-queue priority + bounced state (`066`), push notification quiet-hours + grouping (`067`), phishing dangerous-attachment tracking (`068`), eDiscovery compliance (`069`), email summary cache (`070`), CalDAV public-scheduling tokens (`071`), per-organizer ICS UID on calendar events (`072`), and per-account brute-force lockout (`073`). When adding a status/type column, prefer `TEXT + CHECK` over a Postgres ENUM — see migrations 061/063/065 for the pattern. SQLx is run in *online* mode (no `.sqlx/` offline cache committed) — the build relies on a reachable `DATABASE_URL` at `cargo check`/`cargo build` time.

## Architecture

### Backend (`backend/src/`)
Axum 0.8 web framework with layered architecture:

- **`main.rs`** — Startup: loads config (TOML file or env vars), connects PgPool, runs migrations, starts email scheduler background task, binds Axum server
- **`config.rs`** — Deserialized from `config.toml` or env vars (`TASMAIL_HOST`, `DATABASE_URL`, `IMAP_HOST`, `SMTP_HOST`, `JWT_SECRET`, etc.)
- **`state.rs`** — `AppState { db: PgPool, config: Config }` shared across all handlers
- **`router.rs`** — All routes defined here (~1050 lines). Public routes (health, login, refresh, SAML/OIDC callbacks, branding, billing webhooks, metrics) vs protected routes (everything else behind `auth_middleware`)
- **`handlers/`** — 60+ route handler modules organized by domain (see API Route Structure below)
- **`services/`** — Business logic: `imap_service` (async-imap → Dovecot), `smtp_service` (lettre for sending), `auth_service` (JWT RS256 + Argon2id), `totp_service`, `sms_service`, `email_scheduler` (background polling), `push_service`, `webhook_dispatcher`, `payment_service`, `dlp_scanner`, `phishing_scanner`, `ollama_client`, `embedding_service`, `rspamd_client`, and more
- **`middleware/`** — `auth` (JWT validation + PostgreSQL RLS context), `rate_limit`, `metrics` (Prometheus instrumentation), `security_headers`
- **`models/`** — SQLx data structs (not ORM — raw SQL queries). One model file per domain entity
- **`error.rs`** — `AppError` enum with Axum `IntoResponse` impl

Key patterns:
- IMAP is the source of truth for mail data — the backend proxies to Dovecot via `async-imap`, not a local DB cache
- PostgreSQL stores metadata only (users, contacts, signatures, settings, audit logs, scheduled emails, billing, AI config, etc.)
- Row-Level Security (RLS) enforced at DB level — auth middleware sets `app.current_user_id` session var before each request
- JWT access tokens (15min) + refresh tokens (7 days), stored in localStorage on frontend
- CORS restricted to `CORS_ORIGIN` env var (defaults to `http://localhost:5173`)

### Frontend (`frontend/src/`)
React 19 SPA with Vite 8, TypeScript 6, React Router 7, TanStack Query 5, Zustand 5:

- **`api/client.ts`** — Singleton `ApiClient` class wrapping fetch with auto-refresh on 401. All API modules (`auth.ts`, `messages.ts`, `folders.ts`, etc.) use this client
- **`stores/`** — Zustand stores: `mailStore` (selected folder/uid/viewMode), `uiStore` (theme, sidebar state)
- **`hooks/`** — `useAuth`, `useMailbox`, `useLowBandwidth`, `useOnlineStatus`, `useWebSocket`, `useKeyboardShortcuts`, `useDragAndDrop`, `useMediaQuery`, `useResponsive`
- **`components/`** — Organized by function:
  - `layout/` — AppShell, Sidebar, TopBar, QuotaBar
  - `mail/` — MessageList, MessageView, Composer (TipTap), FolderTree, SearchResults
  - `settings/` — 30+ manager components (contacts, signatures, calendar, billing, AI config, DLP, LDAP, SAML, branding, retention, etc.)
  - `auth/` — LoginPage
  - `shared/` — ErrorBoundary, LoadingSkeleton
- **`types/`** — TypeScript interfaces matching backend API responses
- **`utils/`** — `offline-cache.ts` (IndexedDB), `background-sync.ts` (service worker sync — consumes `/api/messages/schedule`, `/api/folders/{folder}/messages/{uid}/{move,flag}`, `/api/drafts` via **dynamic imports**, so static traceability scans miss them), `sanitize.ts` (DOMPurify for HTML email), `date.ts`, `constants.ts`

PWA enabled via `vite-plugin-pwa` with Workbox runtime caching for API responses (`NetworkFirst`, 5s timeout, 100 entries max).

### Vite Dev Proxy
The dev server proxies `/api` → `http://127.0.0.1:3000` and `/ws` → `ws://127.0.0.1:3000`, so both backend and frontend can run on their own ports during development.

### Test Setup
- **Frontend unit tests**: Vitest 4 with jsdom, `@testing-library/react`, `fake-indexeddb`. Setup file: `src/test/setup.ts`. Tests colocated with source (`*.test.ts` next to `*.ts`). Vitest config in `vite.config.ts` under `test` key (globals enabled). E2E specs in `e2e/` are excluded from Vitest
- **Frontend E2E**: Playwright with Firefox as default project (`npm run e2e`)
- **Backend**: Standard `#[cfg(test)]` modules and `tokio-test`. Rust edition 2024

### Mobile App (`mobile/`)
Flutter app (Dart SDK ^3.11.3) targeting Android and iOS:

```bash
cd mobile
flutter pub get                # Install dependencies
flutter run -d chrome          # Run on Chrome (web debug)
flutter run                    # Run on connected device/emulator
flutter test                   # Run unit tests
flutter build apk              # Build Android APK
```

- **`lib/api/`** — API client matching backend endpoints
- **`lib/models/`** — Dart data classes for API responses
- **`lib/providers/`** — State management
- **`lib/screens/`** — App screens
- **`lib/services/`** — Business logic (auth, sync, notifications)
- **`lib/l10n/`** — Localization (includes Twi, Ewe, Ga, Hausa for Ghana market)

### Deployment (`deploy/`)
Production infrastructure configs: `docker-compose.yml`, plus config directories for `nginx`, `postfix`, `dovecot`, `dns`, `tls`, `systemd`, and helper `scripts/`. Environment template: `tasmail.env.example`.

### Brand kit (`branding/`)
Source-of-truth for the TASMail mark + palette. `BRAND.md` documents the `t@s` envelope mark, palette tokens (`--tm-blue-600`, `--tm-teal-400`, etc.), typography, and contrast rules. `src/build_logo.py` and `src/build_assets.py` regenerate every derivative in `build/` (`app-icons/`, `ico/`, `png/`, `social/`, `svg/`, `wordmark/`) from those primitives, and the published downloadable archive ships from the same pipeline. Run the scripts from `branding/` after any palette or glyph change rather than editing the rendered assets by hand.

### Repo-root `scripts/`
Small grab-bag of repo-level tooling. Operational scripts live under `deploy/scripts/` and `backend/`, not here.

| Script | Purpose |
|--------|---------|
| `trace-check.py` | Backend↔SPA route traceability gate. Used by the `trace-check` CI workflow and the `npm run trace-check` script. Reads `docs/traceability/orphans-baseline.json` to allow legacy orphans while blocking new drift on critical route categories (auth, billing, folders, messages, signatures, contacts). |
| `build-alt-ui.sh` | Builds `themes/shadcn-prototype/` and copies the bundle into `frontend/public/modern/`. Called by `npm run build:alt-ui`. |
| `notebooklm-login-firefox.mjs` | One-off NotebookLM auth helper. |

### CI (`.github/workflows/`)
- **`trace-check.yml`** — runs `scripts/trace-check.py` on PRs touching `backend/src/router.rs`, `backend/src/handlers/**`, `frontend/src/api/**`, `frontend/src/components/**`, `frontend/src/hooks/**`, `frontend/src/utils/**`, the script itself, or the baseline JSON. When adding/removing a route in a critical category, expect to either wire up a SPA consumer or refresh `docs/traceability/orphans-baseline.json` via `npm run trace-check:update`. Note that `frontend/src/utils/background-sync.ts` uses **dynamic imports** for some API modules, so the static scan can miss them — keep that in mind when investigating false positives.

### Alt-UI theme (`themes/shadcn-prototype/`)
Standalone Vite + React app on top of shadcn/ui + Radix + Tailwind, **wired to the production backend** as an alternative theme served at `/modern/`. Logged-in users hop over via the wand-icon button in the classic SPA's TopBar (or by typing `/modern/index.html`); the alt-UI's AuthGate reads the same JWT from localStorage so no second login. The header shows a `← Classic` link to come back.

Build pipeline: `npm run build:alt-ui` (in `frontend/`, calls `scripts/build-alt-ui.sh`) installs deps, runs `vite build`, and copies the bundle into `frontend/public/modern/`. Vite serves it as static files; in production the same path goes through Apache → SSH tunnel → Vite. Re-run after any change to `themes/shadcn-prototype/src/`.

All four surfaces are live-API-backed: EmailClient + EmailList + EmailReader + ComposeModal (folders, messages, send via scheduledApi, save-draft), CalendarView (`/api/calendar/events` per TMAIL-235/236/237), and AdminDashboard (`/api/admin/users`, `/api/admin/domains`, `/api/quota` per TMAIL-232/233). The legacy `themes/shadcn-prototype/src/data/mockData.ts` was removed in TMAIL-239 — only the typed view-model in `types/ui.ts` remains. Routing uses `createHashRouter` so internal nav stays inside `/modern/index.html#/...` and doesn't get caught by Vite's SPA fallback. End-to-end coverage lives in `frontend/e2e/alt-ui-modern.spec.ts` (TMAIL-292) and `frontend/e2e/specs/modern-ui-noreply-walkthrough.spec.ts`. See `themes/shadcn-prototype/README.md` for the full status.

## Configuration

Backend reads `config.toml` if present, otherwise falls back to environment variables:

| Env Var | Default | Purpose |
|---------|---------|---------|
| `TASMAIL_HOST` | 127.0.0.1 | Server bind host |
| `TASMAIL_PORT` | 3000 | Server bind port |
| `DATABASE_URL` | postgres://tasmail:tasmail@localhost/tasmail | PostgreSQL connection |
| `IMAP_HOST` | 127.0.0.1 | Dovecot IMAP server |
| `IMAP_PORT` | 993 | IMAP port (IMAPS) |
| `SMTP_HOST` | 127.0.0.1 | Postfix SMTP server |
| `SMTP_PORT` | 587 | SMTP submission port |
| `JWT_SECRET` | dev-secret-change-in-production | JWT signing key |
| `LOG_FORMAT` | (text) | Set to "json" for JSON logging |
| `CORS_ORIGIN` | http://localhost:5173 | Allowed CORS origin(s). Comma-separated list supported, and entries may use `*.subdomain` wildcards (e.g. `https://mail.techatscale.io,https://*.tenants.tasmail.io`). See `backend/src/cors.rs`. |

## API Route Structure

All routes in `router.rs`. Public: `/api/health`, `/api/auth/login`, `/api/auth/refresh`, `/api/auth/saml/*/login`, `/api/auth/oidc/*`, `/api/branding`, `/api/billing/plans`, `/api/billing/webhook/*`, `/metrics`, `/api/dl/{token}`. Everything else requires `Authorization: Bearer <token>`.

Key route groups:
- `/api/folders`, `/api/folders/{folder}/messages` — IMAP operations (list, get, delete, move, flag)
- `/api/messages/send`, `/api/drafts`, `/api/search` — Compose/search
- `/api/folders/{folder}/messages/{uid}/comments` — Email comments
- `/api/folders/{folder}/messages/{uid}/eml` — EML export/import
- `/api/folders/{folder}/messages/{uid}/phishing` — Phishing scan/report
- `/api/signatures`, `/api/contacts`, `/api/groups` — User data
- `/api/templates` — Email templates with variable rendering
- `/api/filters` — Sieve mail filter rules
- `/api/tasks` — Email-linked to-do items
- `/api/messages/snooze`, `/api/messages/schedule` — Snooze and scheduled send
- `/api/calendar/events` — Calendar events with ICS export and RSVP
- `/api/calendar/free-busy` — Attendee free/busy lookup (GET single, POST batch) + meeting-slot suggestion
- `/api/calendar/imip/accept` — Inbound iMIP REQUEST handler (auto-sent on event create per TMAIL-127)
- `/api/caldav/*` — CalDAV/public-scheduling tokens (migration `071`)
- `/api/attachments` — Attachment storage and download
- `/api/shared-files` — Large file sharing with token-based download
- `/api/queue` — Email queue management (list, retry, cancel)
- `/api/delegation` — Email delegation (send-as)
- `/api/2fa/*`, `/api/sms-otp/*`, `/api/webauthn/*` — Multi-factor auth (TOTP, SMS, FIDO2)
- `/api/webhooks` — Outbound webhook management
- `/api/chat-integrations` — Slack/Teams/Discord notifications
- `/api/ai/config` — AI provider configuration (BYOK)
- `/api/shared-mailboxes/*` — Dovecot ACL shared mailboxes
- `/api/auto-reply` — Vacation responder
- `/api/quota` — Storage quota
- `/api/migration/*` — IMAP, MBOX, and PST import
- `/api/mobile/*` — Flutter-app surface (mobile/), not consumed by the web SPA. Six routes: inbox, message, folders, unread-count, batch, sync.
- `/api/sync/checkpoint/{folder}`, `/api/sync/resolve-conflict` — used by the mobile app's offline sync; not consumed by the web SPA. Distinct from `frontend/src/utils/background-sync.ts` which is the SPA's own offline queue.
- `/api/admin/*` — Domains, users, audit, branding, retention/legal-holds, custom hostnames, LDAP/AD, SAML, bulk user import
- `/api/billing/*` — Subscription plans, Paystack/MoMo webhooks
- `/ws` — WebSocket (auth via token query param)

## BYOK signup + onboarding

The webmail-for-any-IMAP positioning is wired end-to-end:

| Piece | Location |
|---|---|
| Public signup page | `frontend/src/components/auth/SignupPage.tsx` → `/signup` |
| Public landing page | `frontend/src/components/landing/LandingPage.tsx` → `/` |
| Onboarding wizard | `frontend/src/components/onboarding/OnboardingWizard.tsx` → `/onboarding` |
| Per-user IMAP servers | migration `055_imap_configurations.sql` + `models/imap_config.rs` + `handlers/imap_config.rs` |
| Per-user SMTP servers | migration `042_byo_smtp.sql` + `models/smtp_config.rs` + `handlers/smtp_config.rs` (already existed) |
| Synthetic byok.tasmail domain | migration `056_byok_signup.sql` (so signups don't require an admin to pre-create a domain) |
| `POST /api/auth/signup` | `handlers/auth.rs::signup` — public, returns JWT pair |
| `GET /api/imap-configs/presets` | 11 popular providers auto-fill the wizard (Gmail/Outlook/Yahoo/Zoho/FastMail/iCloud/ProtonMail Bridge/etc.) |
| `POST /api/imap-configs/test` | TCP+LOGIN test before save |
| `ImapService::for_user(state, user_id)` | Loads the user's default IMAP config + decrypts password — used by `handlers/folders.rs` (other handlers still pending migration; see `docs/SELF-HOST-MAIL-SERVERS.md`) |

## Payment provider config

Credentials live in the **DB**, not env vars (mirrors PayPro's `payment_provider_config`):

* Migration `054_payment_provider_config.sql` — table with AES-256-GCM-encrypted columns
* `models/payment_provider_config.rs` — `PaymentProviderConfig::resolve(provider, tenant_id)` returns the effective row (tenant-scoped row beats global)
* `services/encryption.rs::EncryptionService` — derives 32-byte key from `JWT_SECRET`, used for all DB-stored secrets
* `handlers/admin/payment_providers.rs` — `GET/POST /api/admin/payment-providers` and `DELETE /api/admin/payment-providers/{id}` for credential CRUD
* `handlers/billing.rs` — calls `load_provider(&state, "PAYSTACK"|"MASTERCARD"|"CYBERSOURCE"|"BANK_TRANSFER")` per request; returns 503 with actionable message if the row is missing

The four providers mirror PayPro: **Paystack, Mastercard MPGS, Cybersource invoicing,
manual Bank Transfer**. MTN MoMo was removed during the pivot.

## Live deployment (mail.techatscale.io)

Backend runs on the workstation (`tas-src-1`), not the proxy server:

* Backend: Rust/Axum on `127.0.0.1:3300` (managed by systemd user unit `tasmail-backend.service`)
* Frontend: Vite dev server on `127.0.0.1:5273` (`tasmail-vite.service`)
* SSH reverse tunnel: `9601→3300`, `9602→5273` to `140.82.32.141` (`tasmail-tunnel.service`, script in `~/Documents/code/tas-src-rtunnel/tasmail-tunnel.sh`)
* Apache vhost on the proxy: `/api`, `/metrics`, `/ws` → backend; `/` → Vite SPA. Let's Encrypt cert.
* Postfix/Dovecot are intentionally NOT installed — TASMail is BYOK, see `docs/SELF-HOST-MAIL-SERVERS.md`

Manage the live site:

```bash
systemctl --user status tasmail-{backend,vite,tunnel}.service
systemctl --user restart tasmail-backend.service   # picks up new release binary
~/Documents/code/tas-src-rtunnel/tasmail-tunnel.sh status
journalctl --user -u tasmail-backend.service -f
```

## Pricing & business model (BYOK positioning)

The product is sold as **TASMail BYOK at GHS 1.00 / GB · month** (≈ $0.07 USD, GHS 5
monthly minimum) with a custom-quoted **Enterprise** tier (single-tenant, SAML/OIDC,
on-prem option). Settlement is in Ghana cedis via Paystack, Mastercard MPGS, Cybersource
invoicing or bank transfer — the same four providers PayPro uses. The live calculator
is at `/pricing`. Keep this in mind when touching billing / quote-request code paths
(`migrations/058_usage_billing.sql`, `migrations/059_enterprise_quote_requests.sql`,
`handlers/billing.rs`).

## Branch & deploy policy (TMAIL convention)

While TMAIL is in its early/beta stage, the workflow is **direct-to-`main`**:

* Routine dev work and the auto-fix queue commit straight to `origin/main` — no PR or worktree gating required.
* `git push --no-verify` is the default (matches the global rule + bypasses husky).
* The systemd unit `tasmail-backend.service` picks up the new release binary on restart, and `tasmail-vite.service` is hot-reload. So a green `cargo build --release` + `systemctl --user restart tasmail-backend.service` is the deploy.
* The main session should *orchestrate* (queue / re-queue auto-fix work, summarise, escalate) — the queue workers do the coding. Reference `TMAIL-NNN` in commit subjects; TodoWrite progress auto-syncs to the linked PM item.

This convention will tighten (PR-gated, worktree-isolated) once the closed beta graduates — see `docs/BETA-LAUNCH-RUNBOOK.md`.

## Common gotchas

A small set of things that have bitten new sessions and are not obvious from reading code:

* **`JWT_SECRET` is load-bearing** — it signs JWTs **and** derives the AES-256-GCM key used to encrypt IMAP/SMTP passwords, payment provider credentials, and other DB-stored secrets (`backend/src/services/encryption.rs`). Rotating it invalidates every refresh token *and* every encrypted column. Keep the production value stable.
* **Row-Level Security is on by default** — raw `psql` queries against tenant-scoped tables (`emails`, `contacts`, `signatures`, etc.) will return empty unless you first `SET app.current_user_id = '<uuid>';` in the session. The auth middleware sets this per-request; ad-hoc DB inspection needs it too.
* **Build needs a live database** — `cargo build` / `cargo check` invoke the SQLx macros against `DATABASE_URL`. CI and local builds will fail with a SQLx error if the DB is unreachable. There's no `.sqlx/` offline cache committed.
* **`npm run trace-check` is a CI gate** — run it locally before pushing changes to `backend/src/router.rs`, `backend/src/handlers/**`, or `frontend/src/api/**`. If you intentionally removed a SPA consumer of a critical route (auth/billing/folders/messages/signatures/contacts), refresh the baseline with `npm run trace-check:update` in the same commit.
* **`background-sync.ts` uses dynamic imports** — the static traceability scan misses these. False positives on `/api/messages/schedule`, `/api/drafts`, `/api/folders/.../move|flag` usually trace back here, not to a genuine orphan.
* **CORS_ORIGIN must include both the SPA and the alt-UI origin** in dev if you want the wand-icon hop to work — `/modern/` is served by Vite at the same origin in prod, but in dev the alt-UI build is copied into `frontend/public/modern/` and read via the same Vite server.
* **Live deployment is workstation-hosted** — the backend runs on `tas-src-1` (this machine) and is reverse-tunnelled to the public proxy. `systemctl --user restart tasmail-backend.service` is what "deploys" — there is no container registry, no remote build, no CI/CD pipeline.

## Adding a new route (cookbook)

The codebase has 60+ handlers; the pattern for a new endpoint is:

1. **Migration** — add `backend/migrations/NNN_feature.sql` (next sequential number). Prefer `TEXT + CHECK` over Postgres `ENUM`; add the appropriate `tenant_id` FK + RLS policy if the table is tenant-scoped.
2. **Model** — add `backend/src/models/feature.rs` with SQLx-decodable structs (use `String` for status/type columns thanks to the ENUM→TEXT migrations).
3. **Handler** — add `backend/src/handlers/feature.rs` with `async fn` signatures of shape `(State<AppState>, AuthUser, Json<Req>) -> Result<Json<Resp>, AppError>`.
4. **Route** — wire it in `backend/src/router.rs` (decide public vs protected — protected sits below `auth_middleware`).
5. **API client** — add the call in `frontend/src/api/feature.ts` going through the singleton `ApiClient` (gives auto-refresh on 401 for free).
6. **Consumer** — wire a component / hook / TanStack Query usage so the route isn't orphaned.
7. **Traceability** — run `npm run trace-check` locally. If the route is in a *critical category* (auth, billing, folders, messages, signatures, contacts) and intentionally has no SPA consumer (e.g. webhook, mobile-only), refresh `docs/traceability/orphans-baseline.json` with `npm run trace-check:update` in the same commit.
8. **Tests** — backend: `#[cfg(test)]` module on the handler; frontend: colocated `*.test.ts` next to the API module. Add an E2E spec under `frontend/e2e/` if it's a user-visible flow.
9. **Commit** — reference the PM item (e.g. `TMAIL-123`) in the commit subject; the PM module auto-syncs TodoWrite progress.

## Documentation

Detailed docs in `docs/`: `PRD.md`, `ARCHITECTURE.md`, `SSR.md` (SRS), `API-SPECIFICATION.md`,
`DEVELOPMENT-SETUP.md`, `DEPLOYMENT-GUIDE.md`, `SECURITY.md`, `PROJECT-MANAGEMENT-PLAN.md`,
`PROJECT-MEMBERS.md`, `BUSINESS-VALIDATION-GHANA.md`, `DNS-MX-ONBOARDING.md`,
`GAP-ANALYSIS.md`, `SELF-HOST-MAIL-SERVERS.md` (optional Postfix/Dovecot install path),
`MOBILE-PLATFORM-DECISION.md` (ADR: Flutter for the Ghana market — TMAIL-49),
`BACKUP-RESTORE.md` (daily pg_dump + incremental Maildir rsync + off-site + verify — TMAIL-42),
`IP-WARMUP-RUNBOOK.md` (8-week ramp + Google Postmaster Tools enrollment — TMAIL-17),
`PAYMENT-PROVIDER-MIGRATION.md` (PayPro → TASMail credential migration runbook — TMAIL-163),
`HOSTING-PROCUREMENT.md` (Aveshost beta → Smart Infraco colocation procurement runbook — TMAIL-18),
`COMPANY-REGISTRATION-RUNBOOK.md` (Ghana Office of the Registrar of Companies filing runbook — TMAIL-43),
`DPC-REGISTRATION-RUNBOOK.md` (Ghana Data Protection Commission Data Controller registration runbook — TMAIL-44),
`BETA-CUSTOMER-RECRUITMENT-RUNBOOK.md` (closed-beta recruitment of 10 customers — 3 BYO-SMTP + 7 Full Hosted — from personal network — TMAIL-45),
`BETA-LAUNCH-RUNBOOK.md` (4-week closed-beta launch + operations playbook — WhatsApp group, daily morning weather report, deliverability/perf/UX monitoring, weekly digests, exit interviews, graduation decision — TMAIL-47),
`MOBILE-DISTRIBUTION-RUNBOOK.md` (Google Play Store + Huawei AppGallery distribution runbook — account setup, keystore ceremony, fastlane lanes, release procedure, store assets, submission checklists — TMAIL-56),
`PRIVACY-POLICY-MOBILE.md` (mobile-app privacy policy text rendered at `/privacy-policy/mobile` and linked from Play + AppGallery listings — TMAIL-56),
`MOBILE-FCM-SETUP.md` (Firebase Cloud Messaging operator setup runbook — Firebase project creation, FlutterFire CLI config, APNs key upload, the 3-edit code swap in `main.dart` that flips on real FCM — TMAIL-150),
plus `docs/research/` (raw research notes), `docs/traceability/` (generated
DOCX/PDF traceability reports — regenerated, don't hand-edit), and
`docs/assessments/` (point-in-time audits, e.g. `frontend-render-perf-2026-05.md`).
