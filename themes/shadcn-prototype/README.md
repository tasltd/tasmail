# TASMail — shadcn/ui prototype theme

Self-contained React + Vite prototype that reimagines the TASMail SPA on top
of [shadcn/ui](https://ui.shadcn.com), Radix primitives and Tailwind. Lives
here as an alternative theme/UI direction so the work isn't lost — the
production SPA continues to live in `frontend/`.

## What it is

- A standalone Vite app rooted at `themes/shadcn-prototype/` with its own
  `package.json`, `vite.config.ts` and `node_modules`.
- 50+ shadcn/ui primitives under `src/components/ui/` (accordion, dialog,
  command, sidebar, sheet, etc.) — drop-in replacements for the inline-styled
  components the production SPA currently uses.
- Three feature areas wired up against mock data:
  - `src/features/email/` — EmailClient, EmailList, EmailReader, ComposeModal
  - `src/features/calendar/` — CalendarView
  - `src/features/admin/` — AdminDashboard
- Routing via React Router 7 (`src/app/routes.ts`).
- Tailwind theme tokens + Inter font (`src/styles/`).

## What it is **not** (yet)

- It's **not wired to the TASMail backend.** Every screen reads from
  `src/data/mockData.ts`. To make this a real alternative theme:
  1. Swap the mock imports for the real clients in `frontend/src/api/`
     (which is the cleanest way to share the existing apiClient + auth
     plumbing — the alt-UI doesn't need to re-implement the network layer).
  2. Replace mock state hooks with TanStack Query calls.
  3. Wire login + onboarding + BYOK provider settings through the same
     endpoints documented in the root `CLAUDE.md`.
- It does **not yet** ship via the live deployment at `mail.techatscale.io`.
  The production tunnel serves `frontend/` only.

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
