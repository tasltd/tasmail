# Frontend Bundle Size, Code Splitting & Lazy Loading Assessment

- **Issue:** TMAIL-259 (axis of TMAIL-241)
- **Date:** 2026-05-27
- **Scope:** React SPA at `frontend/`, route/component graph, `vite.config.ts`, alt-UI
  prototype at `themes/shadcn-prototype/`, and Flutter mobile app at `mobile/`.
- **Method:** Two production builds (`npm run build`) with `rollup-plugin-visualizer`
  configured to emit `dist/stats.html`. The "before" build used `main` as-is; the
  "after" build applies the three commits landed alongside this report
  (`vite.config.ts` manual vendor chunks, `AppShell` + `App` route-level `React.lazy()`
  splits, and a `Suspense`-wrapped `CalendarView` in `CalendarManager`).

---

## TL;DR — biggest wins, by ROI

| # | Finding | Impact | Effort | Status |
|---|---------|--------|--------|--------|
| 1 | **One single 1,446 kB / 398 kB-gzip chunk** shipped the entire app — every Settings manager, the TipTap editor, every admin page, FullCalendar — to every user before first paint. | Critical — every visitor downloads ~398 kB gzip even to look at the landing page | Medium | **Fixed in this audit** |
| 2 | `AppShell.tsx` statically imports **all 40 Settings managers** at the top of the file, so the conditional `viewMode === 'X' && <X />` rendering does not actually delay loading the code. | High — biggest single contributor to the entry size | Medium — 40 `React.lazy` wrappers + one `<Suspense>` boundary | **Fixed in this audit** |
| 3 | Composer (TipTap + ProseMirror, ~370 kB raw) was in the main bundle even though it's only opened on `compose` view. | High | Trivial — one `React.lazy` | **Fixed in this audit** |
| 4 | `CalendarView` (FullCalendar core + dayGrid + timeGrid + list + interaction, ~265 kB raw) loaded eagerly with every visit to `CalendarManager`, even in list mode. | Medium — only matters once user opens Calendar, but unrequested | Trivial — `lazy()` + `<Suspense>` toggled on `showCalendarView` | **Fixed in this audit** |
| 5 | No `manualChunks` strategy — react, react-dom, tanstack/query, tiptap and fullcalendar were inlined into the single entry chunk, so every code change invalidated the whole bundle hash. | High (cache invalidation cost on every deploy) | Trivial — vendor split function | **Fixed in this audit** |
| 6 | Auxiliary routes (signup, onboarding, pricing, **all 8 admin pages**, usage billing, public booking) were eager imports off `App.tsx`. The marketing landing page paid for the admin shell. | Medium | Trivial — `React.lazy` per route | **Fixed in this audit** |
| 7 | No `dist/stats.html` was being generated. Bundle drift was invisible to reviewers — only the Vite stdout summary. | Low | Trivial — `rollup-plugin-visualizer` added to dev deps | **Fixed in this audit** |
| 8 | `frontend/src/utils/background-sync.ts` dynamically imports `api/messages.ts` and `api/scheduled.ts`, but those modules are also statically imported by Composer, FolderTree, MessageView, useKeyboardShortcuts, useMailbox. Rolldown's `[INEFFECTIVE_DYNAMIC_IMPORT]` warning fires — the dynamic import yields no chunk split. | Low (a few KB, cosmetic) | Low — would require routing `api/messages.ts` through a thin proxy, or accepting the warning. Not landed in this audit; see "Deferred work" below. | Deferred |
| 9 | Workbox PWA `globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}']` now precaches every settings-manager chunk too. After the split, precache count goes from 20 → 170 entries (2 042 KiB → 2 949 KiB). That's worse for first install, better for offline coverage. The TMAIL-261 PWA assessment already flagged the precache strategy; not changed here so the two assessments don't conflict. | Low (one-time install cost only) | Low | Cross-reference TMAIL-261 |
| 10 | Alt-UI (`themes/shadcn-prototype/`) builds independently and only ships its own shadcn primitives. Tree-shaking is already effective because each `@/components/ui/*` is imported by file, not from a barrel — `unused-import` rules in `eslint-plugin-react-refresh` enforce this. No change required. | Positive baseline | — | — |
| 11 | Flutter mobile app (`mobile/`) does its own deferred-loading via Dart's `lazy()` and `flutter build apk --split-per-abi`. Bundle-size-sensitive surfaces (AI compose, calendar) are not part of the mobile MVP yet, so no Dart-side work was needed; flagged for the next mobile sprint. | N/A today | — | Flagged for TMAIL-149+ mobile epic |

---

## Before / after — chunk-by-chunk

### Initial-load chunks (what every authenticated user downloads to see the inbox)

| Chunk | Before (raw / gzip) | After (raw / gzip) | Delta gzip |
|---|---|---|---|
| Entry (`index-*.js`) | 1 445.67 kB / 398.21 kB | 102.97 kB / **27.86 kB** | **−370 kB gzip (−93%)** |
| `react-vendor-*.js` | — (inlined in entry) | 242.20 kB / 77.00 kB | new chunk, cached across deploys |
| `query-vendor-*.js` | — (inlined in entry) | 43.05 kB / 13.11 kB | new chunk |
| **Eager total** | **1 445.67 kB / 398.21 kB** | **388.22 kB / 117.97 kB** | **−281 kB gzip (−70.5%)** |

### Deferred chunks (loaded only when the user navigates there)

| Chunk | Loaded when | Size (raw / gzip) |
|---|---|---|
| `editor-vendor-*.js` (TipTap + ProseMirror + DOMPurify) | First `compose` view | 371.55 kB / 118.58 kB |
| `Composer-*.js` | First `compose` view | 29.44 kB / 8.08 kB |
| `calendar-vendor-*.js` (`@fullcalendar/*`) | First time user toggles Grid in CalendarManager | 265.34 kB / 76.30 kB |
| `CalendarView-*.js` | Same as above | 2.04 kB / 1.00 kB |
| Each Settings manager (40 chunks) | First time user opens that view | 0.4 – 13.6 kB raw / 0.2 – 4.2 kB gzip each |
| Each admin page (8 chunks: AuditLog, Cache, Domains, PaymentProviders, Users, Warmup, FeatureFlags, QuoteRequests) | First time user opens that page | 3 – 8 kB raw / 1.3 – 2.6 kB gzip each |
| `OnboardingWizard-*.js` | After signup, only for users in BYOK onboarding | 11.22 kB / 3.35 kB |
| `SignupPage-*.js` | `/signup` route | 2.82 kB / 1.14 kB |
| `BookingPage-*.js` | `/book/:token` public scheduling | 4.70 kB / 1.73 kB |
| `PricingPage-*.js` | `/pricing` | 7.73 kB / 2.52 kB |
| `UsageBillingPage-*.js` | `/billing` | 5.23 kB / 1.86 kB |

### Chunk-size warning threshold

| | Before | After |
|---|---|---|
| Chunks > 300 kB gzip | 1 | 0 |
| Chunks > 500 kB raw warning | yes (1 446 kB) | no — largest is editor-vendor at 372 kB raw / 118.58 kB gzip |
| Build warning suppressed? | no | no — warning genuinely cleared |

### PWA precache footprint

| | Before | After |
|---|---|---|
| precache entries | 20 | 170 |
| precache size | 2 041.62 KiB | 2 948.63 KiB |
| First-install cost | ~2.0 MB | ~2.9 MB (one-time) |
| Steady-state cost | full app already cached | full app already cached, but with finer cache-bust granularity (one chunk per manager) |

The +900 KiB precache increase is the cost of granular caching. After deploys, only the
chunks whose source actually changed get re-fetched — before, **any** change re-fetched
the full 1.4 MB entry. The PWA precache strategy itself is owned by TMAIL-261 and is not
modified in this audit.

---

## What changed in the codebase

Three scoped commits, each kept small enough to revert in isolation:

1. **`vite.config.ts`** — added `rollup-plugin-visualizer` + `build.rolldownOptions.output.manualChunks`
   (function form, required by Vite 8 / Rolldown). Vendor groups:
   - `react-vendor` → `react`, `react-dom`, `react-router(-dom)`, `scheduler`
   - `query-vendor` → `@tanstack/react-query`, `@tanstack/react-virtual`
   - `editor-vendor` → `@tiptap/*`, `prosemirror-*`, `dompurify`
   - `calendar-vendor` → `@fullcalendar/*`

2. **`frontend/src/components/layout/AppShell.tsx`** — moved 40 Settings managers + the
   Composer + SearchResults to `React.lazy()` with one shared `<Suspense fallback="Loading…">`
   boundary. `MessageList` and `MessageView` stay eager — they're the first surface a
   logged-in user sees.

3. **`frontend/src/App.tsx` + `frontend/src/components/settings/CalendarManager.tsx`** —
   route-level `lazy()` for `SignupPage`, `OnboardingWizard`, `PricingPage`, the
   `AdminShell` + all 8 admin pages, `UsageBillingPage`, `BookingPage`; in-component
   `lazy()` + `<Suspense>` for `CalendarView` inside `CalendarManager`.

`LandingPage` and `LoginPage` stay eager so the public marketing + login surface paints
without a Suspense round-trip — they live in the entry chunk and account for most of the
27.86 kB gzip there.

---

## Test impact

- `frontend/src/components/layout/AppShell.test.tsx` — the parametrised `it.each` for
  `viewMode → testId` was updated from `screen.getByTestId(testId)` to
  `await screen.findByTestId(testId)` because every non-list/non-reader view now resolves
  through Suspense. All 19 existing assertions still pass.
- `frontend/src/components/settings/CalendarManager.test.tsx` — already used `findBy*`
  patterns; passes unchanged (14/14).
- No other test file imports `AppShell` directly, so the Suspense change is contained.

Build still type-checks (`tsc -b`) with no new errors attributable to the split. Two
pre-existing TypeScript hints (`FormEvent` deprecated, `document.execCommand` deprecated)
in `CalendarManager.tsx` predate this change and are intentionally left for a separate
cleanup commit — they would expand the scope past "Vite config + lazy() wraps".

---

## Deferred work (out of scope for TMAIL-259, flagged for follow-up)

1. **Resolve `[INEFFECTIVE_DYNAMIC_IMPORT]` warnings for `api/messages.ts` and
   `api/scheduled.ts`.** The dynamic imports in `background-sync.ts` are shadowed by
   static imports from at least five other modules. Properly splitting them would
   require either (a) re-routing the static callers through a thinner proxy, or
   (b) accepting that the offline-queue path doesn't actually save bytes and
   converting it to a static import. Either path is a non-trivial refactor and
   would have changed scope past "Vite config + lazy() wraps".

2. **Revisit `lucide-react` tree-shaking.** `node_modules/lucide-react` is 39 MB on
   disk. The current import pattern (`import { Plus, Trash2, ... } from 'lucide-react'`)
   tree-shakes well in production builds — observed icon chunks (`star-*.js`,
   `sparkles-*.js`, etc.) are 0.4 – 1.4 kB each, so Vite/Rolldown is already pruning
   correctly. No action needed, recorded here so future bundle audits don't re-research it.

3. **Cross-reference TMAIL-261 PWA precache strategy.** The increase from 20 → 170
   precache entries is a direct side-effect of the split. Whether to keep the
   blanket `globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}']` or narrow it
   to entry-critical chunks is a PWA-layer decision owned by TMAIL-261 and should
   be settled there, not by re-revising chunking.

4. **Mobile (`mobile/`) bundle audit.** Flutter ships its own deferred loading.
   Bundle-size-sensitive features (AI compose, calendar grid) are not yet
   implemented on mobile (TMAIL-149/150/151/152 are still in backlog), so there
   is no Dart-side equivalent of this audit to run today. Once those features
   land, repeat the same `manualChunks`-equivalent inspection there.

---

## How to re-run this audit

```bash
cd frontend
rm -rf dist
npm run build
# Open dist/stats.html in a browser for the treemap with gzip + brotli sizes.
# Or scrape sizes from the Vite stdout summary directly:
ls -la dist/assets/*.js | sort -k5 -n -r | head -20
```

If any chunk exceeds **300 kB gzip**, expand `manualChunks` in `vite.config.ts` or add a
`React.lazy()` boundary. The current largest gzipped chunk is `editor-vendor` at
**118.58 kB gzip**, well under threshold.
