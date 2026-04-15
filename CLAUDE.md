# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

TASMail — a self-hosted email service with a React 19 SPA frontend, Rust/Axum backend, and Postfix/Dovecot mail infrastructure. GitHub: tasltd/tasmail. TASCIM PM project: TMAIL.

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
```

### Database
PostgreSQL 16+. Default dev connection: `postgres://tasmail:tasmail@localhost/tasmail`. Migrations run automatically on backend startup via `sqlx::migrate!("./migrations")`. 53 migration files (001–053) covering the full schema.

## Architecture

### Backend (`backend/src/`)
Axum 0.8 web framework with layered architecture:

- **`main.rs`** — Startup: loads config (TOML file or env vars), connects PgPool, runs migrations, starts email scheduler background task, binds Axum server
- **`config.rs`** — Deserialized from `config.toml` or env vars (`TASMAIL_HOST`, `DATABASE_URL`, `IMAP_HOST`, `SMTP_HOST`, `JWT_SECRET`, etc.)
- **`state.rs`** — `AppState { db: PgPool, config: Config }` shared across all handlers
- **`router.rs`** — All routes defined here (~875 lines). Public routes (health, login, refresh, SAML/OIDC callbacks, branding, billing webhooks, metrics) vs protected routes (everything else behind `auth_middleware`)
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
- **`utils/`** — `offline-cache.ts` (IndexedDB), `background-sync.ts` (service worker sync), `sanitize.ts` (DOMPurify for HTML email), `date.ts`, `constants.ts`

PWA enabled via `vite-plugin-pwa` with Workbox runtime caching for API responses (`NetworkFirst`, 5s timeout, 100 entries max).

### Vite Dev Proxy
The dev server proxies `/api` → `http://127.0.0.1:3000` and `/ws` → `ws://127.0.0.1:3000`, so both backend and frontend can run on their own ports during development.

### Test Setup
- **Frontend unit tests**: Vitest 4 with jsdom, `@testing-library/react`, `fake-indexeddb`. Setup file: `src/test/setup.ts`. Tests colocated with source (`*.test.ts` next to `*.ts`). Vitest config in `vite.config.ts` under `test` key (globals enabled). E2E specs in `e2e/` are excluded from Vitest
- **Frontend E2E**: Playwright with Firefox as default project (`npm run e2e`)
- **Backend**: Standard `#[cfg(test)]` modules and `tokio-test`. Rust edition 2024

### Deployment (`deploy/`)
Production infrastructure configs: `docker-compose.yml`, plus config directories for `nginx`, `postfix`, `dovecot`, `dns`, `tls`, `systemd`, and helper `scripts/`. Environment template: `tasmail.env.example`.

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
| `CORS_ORIGIN` | http://localhost:5173 | Allowed CORS origin |

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
- `/api/admin/*` — Domains, users, audit, branding, retention/legal-holds, custom hostnames, LDAP/AD, SAML, bulk user import
- `/api/billing/*` — Subscription plans, Paystack/MoMo webhooks
- `/ws` — WebSocket (auth via token query param)

## Documentation

Detailed docs in `docs/`: PRD, Architecture (SRS), API Specification, Development Setup, Deployment Guide, Security, Project Management Plan, and Ghana Business Validation.
