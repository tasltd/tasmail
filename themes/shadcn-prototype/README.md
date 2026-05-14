# TASMail — shadcn/ui prototype theme

Self-contained React + Vite prototype that reimagines the TASMail SPA on top
of [shadcn/ui](https://ui.shadcn.com), Radix primitives and Tailwind. Lives
here as an alternative theme/UI direction so the work isn't lost — the
production SPA continues to live in `frontend/`.

## Status: live alt-theme at /modern/

The prototype is now wired to the production backend and shipped as an
alternative theme. From the classic SPA, click the **Wand** icon in the
TopBar to hop over; or type `/modern/index.html` in the URL bar. The
auth gate reads the JWT this SPA already wrote to `localStorage`, so
no second login. Header shows a `← Classic` link to come back.

What works end-to-end (TMAIL-211 epic):
- **EmailClient** — folder list + message list from `/api/folders` and
  `/api/folders/{folder}/messages` via TanStack Query.
- **EmailReader** — full message body from `/api/folders/{folder}/messages/{uid}`,
  HTML sanitised through DOMPurify before render.
- **ComposeModal** — sends through `scheduledApi.scheduleSend`
  (`/api/messages/schedule` with `delay_seconds: 0`). Same code path the
  production composer uses.
- **AdminDashboard** + **CalendarView** still read from `src/data/mockData.ts`
  — the next iteration can wire them to `/api/admin/*` and `/api/calendar/*`.

## What's in the source tree

- A standalone Vite app rooted at `themes/shadcn-prototype/` with its own
  `package.json`, `vite.config.ts` and `node_modules`.
- 50+ shadcn/ui primitives under `src/components/ui/` (accordion, dialog,
  command, sidebar, sheet, etc.).
- Three feature areas:
  - `src/features/email/` — EmailClient, EmailList, EmailReader, ComposeModal (LIVE)
  - `src/features/calendar/` — CalendarView (mock data)
  - `src/features/admin/` — AdminDashboard (mock data)
- Routing via React Router 7's `createHashRouter` (`src/app/routes.ts`) so
  internal nav stays inside `/modern/index.html#/...` and doesn't fall
  through Vite's SPA fallback to the classic frontend.
- Auth + apiClient layer copied from `frontend/src/api/` into
  `themes/shadcn-prototype/src/api/`. Acceptable duplication for now;
  promote to a shared package if a third UI ever needs it.
- Tailwind theme tokens + Inter font (`src/styles/`).

## Building + deploying

`scripts/build-alt-ui.sh` (also wired as `npm run build:alt-ui` from
`frontend/`) installs deps, runs `vite build`, wipes
`frontend/public/modern/`, and copies the fresh bundle in. Vite's static
file handler then serves it at `/modern/`.

Re-run after any change to `themes/shadcn-prototype/src/`.

## Running locally

```bash
cd themes/shadcn-prototype
npm install      # ~250 deps including MUI + Radix + Tailwind
npm run dev      # Vite dev server (auto-picks a port; usually :5173)
npm run build    # production bundle into dist/
```

## Origin

Originally built on the `feature/20260322-195439-project-email-servic-mazrui-3bb3`
branch as a parallel React experiment with the working title "RustMail". When
the main codebase reorganized everything under `frontend/`, the branch went
stale and never got merged. The salvageable artefacts (everything under `src/`,
the build chain, and the Tailwind config) were extracted here so the work has
a permanent home in the repo. The original branch is now safe to delete.

## Status

**Prototype only** — not part of the test or release pipeline. CI does not
build this directory. The production SPA is `frontend/`; everything in
`themes/shadcn-prototype/` is opt-in for a future theming or rewrite project.
