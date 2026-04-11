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
npm run preview                # Preview production build
```

### Database
PostgreSQL 16+. Default dev connection: `postgres://tasmail:tasmail@localhost/tasmail`. Migrations run automatically on backend startup via `sqlx::migrate!("./migrations")`. 12 migration files (001–012) covering the full schema.

## Architecture

### Backend (`backend/src/`)
Axum 0.8 web framework with layered architecture:

- **`main.rs`** — Startup: loads config (TOML file or env vars), connects PgPool, runs migrations, starts email scheduler background task, binds Axum server
- **`config.rs`** — Deserialized from `config.toml` or env vars (`TASMAIL_HOST`, `DATABASE_URL`, `IMAP_HOST`, `SMTP_HOST`, `JWT_SECRET`, etc.)
- **`state.rs`** — `AppState { db: PgPool, config: Config }` shared across all handlers
- **`router.rs`** — All routes defined here. Public routes (health, login, refresh) vs protected routes (everything else behind `auth_middleware`)
- **`handlers/`** — Route handlers organized by domain: `auth`, `messages`, `folders`, `contacts`, `signatures`, `groups`, `migration`, `shared` (shared mailboxes), `scheduled`, `auto_reply`, `two_factor`, `sms_otp`, `quota`, `health`, plus `admin/` (domains, users, audit)
- **`services/`** — Business logic: `imap_service` (async-imap connections to Dovecot), `smtp_service` (lettre for sending), `auth_service` (JWT RS256 + Argon2id), `totp_service`, `sms_service`, `email_scheduler` (background polling for scheduled sends)
- **`middleware/`** — `auth` (JWT validation + PostgreSQL RLS context), `rate_limit`
- **`models/`** — SQLx data structs (not ORM — raw SQL queries)
- **`error.rs`** — `AppError` enum with Axum `IntoResponse` impl

Key patterns:
- IMAP is the source of truth for mail data — the backend proxies to Dovecot via `async-imap`, not a local DB cache
- PostgreSQL stores metadata only (users, contacts, signatures, settings, audit logs, scheduled emails)
- Row-Level Security (RLS) enforced at DB level — auth middleware sets `app.current_user_id` session var before each request
- JWT access tokens (15min) + refresh tokens (7 days), stored in localStorage on frontend

### Frontend (`frontend/src/`)
React 19 SPA with Vite 8, TypeScript 6, React Router 7, TanStack Query 5, Zustand 5:

- **`api/client.ts`** — Singleton `ApiClient` class wrapping fetch with auto-refresh on 401. All API modules (`auth.ts`, `messages.ts`, `folders.ts`, etc.) use this client
- **`stores/`** — Zustand stores: `mailStore` (selected folder/uid/viewMode), `uiStore` (theme, sidebar state)
- **`hooks/`** — `useAuth` (login/logout/session restore), `useMailbox` (TanStack Query wrappers), `useLowBandwidth`, `useOnlineStatus`
- **`components/`** — Organized by function:
  - `layout/` — AppShell, Sidebar, TopBar, QuotaBar
  - `mail/` — MessageList, MessageView, Composer (TipTap), FolderTree, SearchResults
  - `settings/` — ContactManager, SignatureManager, TwoFactorManager, GroupManager, MigrationManager, VacationResponder, LowBandwidthSettings
  - `auth/` — LoginPage
  - `shared/` — ErrorBoundary, LoadingSkeleton
- **`types/`** — TypeScript interfaces matching backend API responses
- **`utils/`** — `offline-cache.ts` (IndexedDB), `background-sync.ts` (service worker sync), `sanitize.ts` (DOMPurify for HTML email), `date.ts`, `constants.ts`

PWA enabled via `vite-plugin-pwa` with Workbox runtime caching for API responses. Service worker registered for offline support.

### Vite Dev Proxy
The dev server proxies `/api` → `http://127.0.0.1:3000` and `/ws` → `ws://127.0.0.1:3000`, so both backend and frontend can run on their own ports during development.

### Test Setup
- **Frontend**: Vitest 4 with jsdom environment, `@testing-library/react`, `fake-indexeddb`. Setup file: `src/test/setup.ts` (imports `@testing-library/jest-dom/vitest` for matcher extensions). 38 test files colocated with source (`*.test.ts` next to `*.ts`). Vitest config is in `vite.config.ts` under `test` key (globals enabled). Key test deps: `@testing-library/react` 16, `jsdom` 29
- **Backend**: Standard `#[cfg(test)]` modules and `tokio-test`. Rust edition 2024. No integration test infrastructure yet. No E2E tests configured

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

## API Route Structure

All routes in `router.rs`. Public: `/api/health`, `/api/auth/login`, `/api/auth/refresh`. Everything else requires `Authorization: Bearer <token>`. Key route groups:
- `/api/folders`, `/api/folders/{folder}/messages` — IMAP operations
- `/api/messages/send`, `/api/drafts`, `/api/search` — Compose/search
- `/api/signatures`, `/api/contacts`, `/api/groups` — User data
- `/api/2fa/*`, `/api/sms-otp/*` — Two-factor auth
- `/api/admin/*` — Domain/user management, audit log
- `/api/migration/*` — IMAP migration and MBOX import
- `/api/shared-mailboxes/*` — Dovecot ACL shared mailboxes

## Documentation

Detailed docs in `docs/`: PRD, Architecture, API Specification, Development Setup, Deployment Guide, Security, and Project Management Plan.
