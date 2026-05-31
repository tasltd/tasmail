# Firefox E2E suite (Modern + Classic UI)

This directory hosts the Firefox-based Playwright E2E tests for the **Modern
UI** (`modern/`) and **Classic UI** (`classic/`) surfaces, scaffolded under
TMAIL-388 (parent: TMAIL-300). Spec files land in the surface-specific
subdirectory; shared utilities live under `helpers/` and `fixtures/`.

```
e2e/firefox/
├── modern/      # Modern UI specs (siblings B–G plug into this dir)
├── classic/     # Classic UI specs (sibling H — deferred)
├── helpers/     # signup, login, screenshot, api
├── fixtures/    # attachment.txt + optional inbox seeder
└── README.md    # this file
```

Screenshots land under `frontend/e2e/screenshots/{modern|classic}/{feature}/`
where `{feature}` is the spec filename minus `.spec.ts`. Naming convention is
`NN-action.png` (zero-padded so files sort in flow order). Toggle with
`E2E_SCREENSHOTS=false`.

## One-time setup

The suite expects a dedicated test database `tasmail_e2e` so it never touches
the live `tasmail` DB the workstation backend talks to:

```bash
createdb tasmail_e2e
```

Migrations run automatically on backend startup, so nothing else is required.

## Running the suite

Three processes need to be up — open three terminals (or use tmux):

```bash
# 1. Test backend (Rust/Axum on :3399 against tasmail_e2e)
npm run start-test-backend          # from frontend/

# 2. Vite dev server (proxies /api → :3399 if you set the env, defaults to :3000)
npm run dev                          # from frontend/

# 3. Playwright
npm run e2e:firefox                  # from frontend/
```

> Note: the Vite proxy is hard-wired to `http://127.0.0.1:3000` by default. To
> point it at the test backend instead, run Vite with
> `VITE_API_PROXY_TARGET=http://127.0.0.1:3399 npm run dev` once that env var
> is wired into `vite.config.ts` (TMAIL-388 sibling task — not done here).
> Until then, run `npm run start-test-backend` on port 3000 instead by
> overriding `TASMAIL_PORT=3000` for ad-hoc local runs.

Override the base URL for non-default setups:

```bash
PLAYWRIGHT_TEST_BASE_URL=http://localhost:5273 npm run e2e:firefox
```

## Helper API

| Helper | Purpose |
|---|---|
| `helpers/signup.ts` | `signupFreshUser(request)` — programmatically signs up a fresh BYOK tenant via `POST /api/auth/signup`. Returns `{ email, password, accessToken, refreshToken }`. Emails follow `e2e-modern-{ts}-{rand}@byok.tasmail` so each spec gets a clean tenant. |
| `helpers/login.ts` | `login(page, email, password)` — the ONLY place `page.goto('/')` is allowed; everything after lands via menu clicks. `dismissOverlays(page)` clears post-login modals. |
| `helpers/screenshot.ts` | `snap(page, '01-login-form')` — full-page PNG into `screenshots/{surface}/{feature}/`. Surface + feature inferred from the spec's path. |
| `helpers/api.ts` | `new TestApiClient({ request, token })` with `.get / .post / .patch / .delete / .count` for SPA before/after assertions. |
| `fixtures/seed.ts` | `seedInbox({ request, token, email })` drops a 3-message starter set via `/api/messages/send`. Use `maybeSeedInbox` for opt-out via `TASMAIL_SKIP_SEED=1`. |
| `fixtures/attachment.txt` | ~1 KB text file used by compose-attachment specs. |

## Rules anchors (do not bypass)

- **Node.js only** — use `npx` / `npm`, never `bunx` / `bun`.
- **Firefox only** — the project config is `firefox-test`; do not add Chromium/WebKit.
- **No `page.goto()` for internal routes** — navigation must be menu clicks. The
  only allowed `page.goto` is `/` inside `login()`.
- **Screenshot every key step** — page load, after form interaction, before
  assertion. Full-page PNGs.
- **Real backend** — no mocking. Use the running test backend on :3399 against
  `tasmail_e2e`.
