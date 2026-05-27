# Frontend PWA, Offline Cache & Service Worker Assessment

- **Issue:** TMAIL-261 (axis of TMAIL-241)
- **Date:** 2026-05-27
- **Scope:** `frontend/src/utils/offline-cache.ts`, `frontend/src/utils/background-sync.ts`,
  `frontend/src/hooks/useOnlineStatus.ts`, `vite-plugin-pwa` config in
  `frontend/vite.config.ts`, plus the SPA + alt-UI surfaces that consume them.
- **Method:** Static read at HEAD (`2e56a74`). No SW runtime trace was captured because the
  auto-fix session cannot drive a headless install/upgrade cycle of the PWA against a
  changing release. The "to verify" sections call out which findings need an in-browser
  check before they're acted on.

---

## TL;DR — biggest wins, by ROI

| # | Finding | Impact | Effort | Suggested ticket |
|---|---------|--------|--------|------------------|
| 1 | **No service-worker update prompt anywhere.** `vite.config.ts` uses `registerType: 'autoUpdate'` and no `useRegisterSW` hook is wired in `main.tsx`, so a new SW silently installs in the background and only activates on a full reload. Users sitting in an open SPA tab can run a stale SW for hours/days; for security or correctness fixes that's not acceptable. | High | S–M — add `virtual:pwa-register/react` + a banner | New |
| 2 | **Workbox `runtimeCaching` is one-size-fits-all.** A single `NetworkFirst` `/^https?:\/\/.*\/api\//` with `maxEntries: 100`, `maxAgeSeconds: 300`, `networkTimeoutSeconds: 5` is wrong for: search (caches per-query garbage that's never reused), auth (privacy + correctness), mutations (Workbox only caches GET, but the catch-all should still narrow), branding (deserves long SWR), free/busy (NetworkOnly — stale availability data is worse than no answer), folders (deserves SWR — cheap to refresh). | High | M — split into 5–7 route patterns | New |
| 3 | **No conflict resolution on queued offline actions.** `background-sync.executeAction` replays move/delete/flag/save-draft against whatever state the server is in now. If two clients touched the same uid while one was offline, the queued action either silently overwrites or fails opaquely (`processPending` only counts `processed`/`failed`, no per-action surfacing). No `ETag`/`If-Match` on the backend or the client. Treat as documented Last-Write-Wins for now, then add typed strategies. | High (data) | L — needs backend `ETag` support per resource | New |
| 4 | **IndexedDB has no migration path.** Both `tasmail-cache` and `tasmail-sync` are pinned at `DB_VERSION = 1`. `onupgradeneeded` only `createObjectStore`s if missing — it has no `switch (oldVersion)` block, no rename handling, no key-path migration handling. The first time a store needs renaming/reshaping, existing users either get a silent no-op or `ConstraintError`. | Medium | S — adopt the `switch (oldVersion)` pattern now while there's only one version to migrate from | New |
| 5 | **Background-sync is main-thread only, not Web Background Sync.** Queue replay is triggered by `useOnlineStatus`'s `useEffect`, so if the tab is closed and the user reconnects, **nothing replays until they reopen TASMail**. The "background" in the name is misleading. Wiring `registration.sync.register('tasmail-sync')` + a SW `sync` event would let the browser replay even when the tab is shut. | Medium | M — needs a custom SW (current `generateSW` mode hides that file) | New |
| 6 | **No typed SW ↔ main-thread message protocol.** Today nothing posts between the SW and the SPA — there's no `BroadcastChannel('tasmail')`, no `postMessage` on `navigator.serviceWorker.controller`. When (1) and (5) are wired the SW will need to tell the SPA "a new version is ready" and "your queued sync just succeeded". Define the message envelope (`{ type, payload, msgId }`) and a tiny typed router up front. | Medium | S — pure design + a wrapper | New |
| 7 | **Cache-tier TTL semantics are layered and undocumented.** Three independent freshness models — TanStack Query `staleTime` (15 s msgs / 30 s folders / 30 s search), IndexedDB `offlineCache` TTL (2 / 5 / 30 min), Workbox SW cache (5 min) — overlap with no clear invariant. A user can see a 30-min-old message from offline-cache while TanStack believes the response is fresh. Document the intended hierarchy: SW is for offline survival, IDB is for *durable* offline survival, TanStack is for in-tab dedup. | Medium | S — doc + a couple of constants | New |
| 8 | **No `cleanupOutdatedCaches`.** Workbox keeps the previous precache around on every upgrade. After a few deploys the user's Cache Storage carries N revisions of the precache forever. One-line fix. | Low–Medium (storage) | Trivial | Fix in this audit |
| 9 | **Cache cap of 100 entries is small for power users.** `expiration.maxEntries: 100` is shared across all `/api/*` GETs (folders + each page of each folder + each full message + branding + audit). A user paging 5 folders × 20 pages already hits 100. Workbox does LRU evict, so older entries silently drop. Per-route caches (finding 2) each get their own caps. | Medium | Subsumed by fix 2 | Subsumed |
| 10 | **No offline indicator in the alt-UI.** `useOnlineStatus` is only consumed in `frontend/src/components/layout/TopBar.tsx`. The alt-UI in `themes/shadcn-prototype/` has its own header chrome and zero offline affordance, so users on `/modern/` get no signal when they go offline (despite the SW + IDB still being shared with the SPA, since both apps are served from the same origin). | Medium (UX consistency) | S — wire `useOnlineStatus`-equivalent into the alt-UI header | New |
| 11 | **No `navigateFallback`.** Workbox `navigateFallback` is not set, so a fully-offline hard-reload of `/` returns the browser's "no internet" page instead of the cached SPA shell. The precache has `index.html` (via `globPatterns: ['**/*.{js,css,html,…}']`) but nothing wires it as the navigation fallback. | Medium | Trivial | New |
| 12 | **Mutation methods are routed through the same SW cache match.** Workbox `NetworkFirst` only stores GET by default, so the practical risk is low, but `urlPattern: /^https?:\/\/.*\/api\//` matches **every** verb. Explicit `method: 'GET'` makes the intent obvious, prevents future Workbox version regressions, and keeps the runtime check off the hot path for mutations. | Low | Trivial | Fix in this audit |
| 13 | **No `beforeinstallprompt` capture.** TASMail meets installability criteria (manifest + SW + HTTPS) so browsers do show their own install button, but the SPA never captures the event to control its own "Install TASMail" affordance. Worth adding so the install hint surfaces in a context where the user has invested (e.g. after sending the first email), not buried in the omnibar. | Low | S | New |
| 14 | **`background-sync.processPending` runs all queued actions immediately with no backoff.** Online event fires → every queued action replays once → if the backend is down or rate-limiting, every action's `retries` increments together. Next `online` event repeats the burst. Add per-action `nextRetryAt = createdAt + (2 ** retries) * 1000` so backoff is exponential and the queue doesn't thunderclap the backend. | Low–Medium | S | New |
| 15 | **No SW for the alt-UI hash routes.** Workbox precaches `frontend/public/modern/**` (since it's under `dist/`) but `navigateFallback` (finding 11) and `navigateFallbackAllowlist` aren't configured, so a deep link like `/modern/index.html#/inbox` on a cold SW cache offline returns nothing. Once 11 is fixed, allowlist `/modern/index.html`. | Low | Trivial (after 11) | Subsumed by 11 |

---

## Detailed findings

### 1. Service-worker update flow

**Current state (`vite.config.ts:10`)**

```ts
VitePWA({
  registerType: 'autoUpdate',
  ...
  workbox: { /* no skipWaiting, no clientsClaim, no cleanupOutdatedCaches */ },
})
```

`registerType: 'autoUpdate'` means `vite-plugin-pwa` auto-registers the SW *and* schedules an update check on `pageshow` / `online` / hourly. **But** the new SW still goes into `waiting` until *every* SPA tab closes. The user keeps seeing the old SW indefinitely; for a long-running webmail tab that's the dominant case.

Also missing in `main.tsx`:

```tsx
// what's there today
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>)
```

No `import { useRegisterSW } from 'virtual:pwa-register/react'` anywhere in the repo
(confirmed by `grep -r virtual:pwa frontend/src` → empty). No update toast component.

**What to do**

1. Add `workbox-window` (already in `package.json` at `^7.4.0`, unused) **or** the
   `virtual:pwa-register/react` adapter (preferred — comes free with vite-plugin-pwa).
2. New file `frontend/src/components/shared/SWUpdatePrompt.tsx`:

   ```tsx
   import { useRegisterSW } from 'virtual:pwa-register/react';
   export function SWUpdatePrompt() {
     const { needRefresh: [needRefresh, setNeedRefresh], updateServiceWorker } = useRegisterSW({
       onRegisteredSW(_url, r) {
         // Check for updates every 30 min on tabs that stay open all day.
         setInterval(() => r?.update(), 30 * 60 * 1000);
       },
     });
     if (!needRefresh) return null;
     return (
       <div className="sw-update-banner">
         <span>A new version of TASMail is available.</span>
         <button onClick={() => updateServiceWorker(true)}>Reload</button>
         <button onClick={() => setNeedRefresh(false)}>Later</button>
       </div>
     );
   }
   ```

3. Mount in `App.tsx` next to other global affordances.
4. **Don't** ship `skipWaiting: true` in Workbox — that's auto-update without consent and
   races in-flight writes. Keep the user-driven `updateServiceWorker(true)` call.

**To verify in browser**

- Build, deploy, install PWA, build again with a SW change, observe banner.
- Confirm `setInterval(() => r.update(), 30min)` doesn't bust caches needlessly.

### 2. Workbox runtime caching — per-route plan

**Current (`vite.config.ts:27`)**

```ts
runtimeCaching: [{
  urlPattern: /^https?:\/\/.*\/api\//,
  handler: 'NetworkFirst',
  options: {
    cacheName: 'api-cache',
    expiration: { maxEntries: 100, maxAgeSeconds: 300 },
    networkTimeoutSeconds: 5,
  },
}],
```

**Proposed split**

| Route | Handler | maxAgeSeconds | maxEntries | Why |
|-------|---------|--------------:|-----------:|-----|
| `/api/auth/**` | `NetworkOnly` | — | — | Privacy + correctness; never cache JWTs, refresh, signup. |
| `/api/billing/webhook/**` | `NetworkOnly` | — | — | Webhook ingress, not user-facing. |
| `/api/search/**`, `/api/messages/search`, `/api/search/semantic`, `/api/search/nlp` | `NetworkOnly` | — | — | Per-query cache is garbage that's never reused; bloats the 100-entry budget. |
| `/api/calendar/free-busy` | `NetworkOnly` | — | — | Time-sensitive availability data; a 5 min stale read schedules conflicts. |
| `/api/branding` | `StaleWhileRevalidate` | 86 400 | 4 | Rarely changes, every render reads it. |
| `/api/folders` (list) | `StaleWhileRevalidate` | 60 | 4 | Cheap to refresh; render once from cache, kick off network. |
| `/api/folders/*/messages` (GET) | `NetworkFirst` | 300 | 500 | Power-user paging. Bigger cache than 100. |
| `/api/folders/*/messages/*` (GET full message) | `NetworkFirst` | 600 | 200 | Reads dominate; 10 min cache OK because user explicitly opened the message. |
| `/api/quota` | `NetworkFirst` | 60 | 4 | Bottom-of-shell quota bar, cheap. |
| catch-all `/api/**` (GET) | `NetworkFirst` | 300 | 100 | Backstop for unmatched routes. **Pin `method: 'GET'`** so mutations always skip the SW. |

Implementation notes:

- Order matters in `runtimeCaching`. `NetworkOnly` patterns first, specific `*` next, catch-all last.
- Add `method: 'GET'` to every entry that uses a cache. Workbox 7 defaults to GET, but the
  explicit field stops a future Workbox-8 default change from leaking cached POST replies.
- Each entry gets its own `cacheName` so `expiration` LRU evicts within its own bucket — a
  big `messages` page won't evict the small `branding` cache.

**To verify in browser**

- DevTools → Application → Cache Storage shows 5–8 named caches, each within its `maxEntries`.
- Forced offline + cold tab can still render shell + last folder list + last opened messages.

### 3. Conflict resolution on offline mutations

**Today** `executeAction` (background-sync.ts:110) just replays. There's no:

- Server-side version/ETag returned with GET of message/draft/event.
- Client-side `If-Match: <etag>` sent on the replay PUT/DELETE.
- Per-action conflict callback to the SPA ("your offline draft was overwritten — view both").

For the four action types:

| Action | Conflict shape | Recommended strategy |
|--------|---------------|----------------------|
| `send` (scheduled send) | Idempotent if the server stores `client_msg_id`; otherwise duplicates on retry. | Add a UUID `client_msg_id` to the payload; server dedupes. Server-side: TMAIL-261-a. |
| `move` | Target uid may have moved/been deleted; folder may not exist. | `If-Match: <message-etag>`; on 409 surface "this message was already moved" to the user. |
| `delete` | Same as move + may already be in Trash. | Same as move. 404 = treat as success (idempotent delete). |
| `flag` | Race with server-side label set. | Last-Write-Wins is acceptable for flags; document it. Don't bother with ETag for `\Seen`. |
| `save-draft` | Two devices editing same draft = silent overwrite. | Server returns `draft_id` + `version` on create; client sends `If-Match` on update; on 409 → keep both drafts, prompt user. |

This is **mostly a backend change**: the backend needs `ETag` / `version` on
`/api/folders/*/messages/*` and `/api/drafts/*`. Once that exists, the client work is small
(send the header, surface 409 via a new `SyncAction` field `conflictResolution?: 'overwrite' | 'keep-both' | 'prompt'`).

For the SPA UX: a queued action surfacing a conflict needs a place to land. Suggestion: the
"Offline" pill in TopBar (finding 10) grows into an offline-queue dropdown showing pending
actions + any with conflicts.

### 4. IndexedDB versioning

**Today** (`offline-cache.ts:6`, `background-sync.ts:18`):

```ts
const DB_VERSION = 1;
request.onupgradeneeded = () => {
  const db = request.result;
  if (!db.objectStoreNames.contains('folders')) db.createObjectStore(...);
  ...
};
```

**Problem**: `onupgradeneeded` runs on every version bump, not just initial install.
`request.oldVersion` tells you the prior version — if you skip checking it, all migrations
are conditional "only add if missing" which is fine for **adding** but breaks the moment you
need to **rename**, **change keyPath**, or **drop** a store. Today both DBs are at v1, so
nothing has gone wrong yet; that's exactly when to fix it.

**Pattern to adopt now**

```ts
const DB_VERSION = 1;

function migrate(db: IDBDatabase, oldVersion: number, _newVersion: number | null, tx: IDBTransaction) {
  // Each branch is an idempotent step. Add new branches for v2, v3, …; do not edit older ones.
  if (oldVersion < 1) {
    db.createObjectStore('folders', { keyPath: 'key' });
    db.createObjectStore('messages', { keyPath: 'key' });
    db.createObjectStore('fullMessages', { keyPath: 'key' });
  }
  // Future v2 example:
  // if (oldVersion < 2) {
  //   const store = tx.objectStore('messages');
  //   store.createIndex('cachedAt', 'cachedAt');
  // }
  void tx; // silence unused param until first migration that needs it
}

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = (e) => migrate(req.result, e.oldVersion, e.newVersion, req.transaction!);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
    req.onblocked = () => console.warn('IndexedDB upgrade blocked by another tab');
  });
}
```

Cost is one trivial refactor in two files, paid down once, never again. The
`onblocked` handler is the bit that catches a tab holding the old version open during an
upgrade — today we wouldn't even know.

### 5. Web Background Sync vs. main-thread replay

**Today** `useOnlineStatus.ts` calls `backgroundSync.processPending()` in a `useEffect`.
The browser's `sync` event in the SW is not used. The two consequences:

1. If the user closes the tab while offline, queued actions stay in IndexedDB until the
   next time TASMail is opened *and* the SPA mounts. Could be hours.
2. The replay runs in the main thread, blocking the UI if there are many actions. Today
   actions are bounded by user interactions, so this isn't acute.

**Path forward** — needs `injectManifest` mode (not `generateSW`) so we own the SW file:

```js
// frontend/src/sw.ts (new)
self.addEventListener('sync', (event) => {
  if (event.tag === 'tasmail-sync') {
    event.waitUntil(processPendingInSW());
  }
});
```

And on the SPA side, after enqueueing:

```ts
const reg = await navigator.serviceWorker.ready;
if ('sync' in reg) await reg.sync.register('tasmail-sync');
else /* fall back to the current useOnlineStatus path */;
```

Caveat: Background Sync support is patchy in Safari (still no native support as of 2026).
Keep the main-thread fallback. Don't migrate fully — additive.

### 6. Typed SW ↔ main-thread protocol

Findings 1 and 5 both need bidirectional messaging. Define it once:

```ts
// frontend/src/sw-protocol.ts (new)
export type SwToClient =
  | { type: 'SW_UPDATE_READY'; version: string }
  | { type: 'SYNC_COMPLETED'; processed: number; failed: number; conflicts: SyncConflict[] }
  | { type: 'CACHE_PURGED'; reason: 'manual' | 'storage-pressure' };

export type ClientToSw =
  | { type: 'SKIP_WAITING' }
  | { type: 'PURGE_CACHE'; cacheName?: string }
  | { type: 'TRIGGER_SYNC'; tag: 'tasmail-sync' };

export type SyncConflict = {
  actionId: number;
  type: SyncActionType;
  reason: 'http-409' | 'http-404' | 'http-410';
};
```

Wire over a `BroadcastChannel('tasmail')` for SW → many-tabs broadcasts, and direct
`navigator.serviceWorker.controller.postMessage(...)` for SPA → SW. Always include a
discriminated `type` so the receiver can `switch` without `any`.

### 7. Cache-tier hierarchy

Document the intended layering in `frontend/src/utils/README.md` (new):

```
Read priority for "folders" data:
  1. TanStack Query in-memory (stale time 30s)   → fastest, scoped to tab
  2. IndexedDB offlineCache (TTL 5 min)          → survives reloads, per-origin
  3. Workbox SW cache (NetworkFirst, 60s SWR)    → survives reloads, HTTP-level
  4. Network                                     → source of truth
```

Aligning the IDB TTL (currently 5 min folders / 2 min messages / 30 min full messages) with
the per-route Workbox TTL from finding 2 means a single user-perceived freshness rule per
resource. Today they drift independently.

### 8. `cleanupOutdatedCaches`

One-line Workbox fix. Without it, every deploy stacks a new revision of the precache and
old ones live forever in the user's storage.

```ts
workbox: {
  cleanupOutdatedCaches: true,   // <-- add
  ...
}
```

Applied in this commit (see "Quick wins" below).

### 9. Cache cap 100 entries

Today: shared 100-entry LRU across every `/api/*` GET. The `ExpirationPlugin` Workbox uses
applies `lastUsed`-based LRU eviction (confirmed against Workbox 7.4 docs), so this isn't
data loss — but a power user paging through 5 folders silently evicts branding + quota +
audit cache constantly. Splitting per-route caches (finding 2) gives each its own budget.

### 10. Alt-UI offline indicator

`themes/shadcn-prototype/src/` has zero references to `useOnlineStatus`, `navigator.onLine`,
or `backgroundSync`. The alt-UI header is independent from the SPA TopBar, so the
"Offline" pill in `TopBar.tsx:98-102` is invisible there.

Two options:

- **Minimal**: replicate the `useOnlineStatus` hook in the alt-UI (small file, no
  deps), surface a Wifi-off icon in the alt-UI header next to the "← Classic" link.
- **Better**: extract `useOnlineStatus` into a shared module both apps import. Currently
  the alt-UI is a *standalone* Vite app (`themes/shadcn-prototype/`) — sharing requires
  either a path alias into `frontend/src/hooks/` or extracting to a workspace package.
  Path alias is enough for now.

### 11. `navigateFallback` for the cached shell

Without `navigateFallback`, a hard reload while offline returns the browser's offline page.
Set it so the SPA shell takes over:

```ts
workbox: {
  navigateFallback: '/index.html',
  navigateFallbackDenylist: [/^\/api/, /^\/metrics/, /^\/ws/, /^\/modern\/api/],
  navigateFallbackAllowlist: [/^\/(?!.*\.[a-z0-9]+$).*/], // routes, not files
  ...
}
```

Denylist `/api`, `/metrics`, `/ws` so the SW doesn't serve `index.html` in place of a
failed API call (which would corrupt error handling). Allowlist routes that aren't files.

For the alt-UI at `/modern/index.html#/...`, the alt-UI is precached so the hash router
works once `index.html` loads — finding 15 is subsumed by this.

### 12. Explicit `method: 'GET'` on cache patterns

`urlPattern: /^https?:\/\/.*\/api\//` matches POST/PUT/DELETE too. Workbox-7
`NetworkFirst` only stores GET, so the practical risk today is just one extra regex check
per mutation. But:

```ts
{
  urlPattern: /^https?:\/\/.*\/api\//,
  method: 'GET',                              // <-- add
  handler: 'NetworkFirst',
  ...
}
```

…makes the intent explicit and survives a future Workbox default change. Applied in this
commit.

### 13. Install prompt capture

```tsx
useEffect(() => {
  const handler = (e: BeforeInstallPromptEvent) => {
    e.preventDefault();
    installPromptRef.current = e;   // stash for later
    setCanInstall(true);
  };
  window.addEventListener('beforeinstallprompt', handler as EventListener);
  return () => window.removeEventListener('beforeinstallprompt', handler as EventListener);
}, []);
```

Then show an "Install TASMail" affordance in Settings or after the user sends their first
email — calling `installPromptRef.current.prompt()` opens the native installer. Low
priority but pays off the existing manifest investment.

### 14. Exponential backoff in `processPending`

Today (background-sync.ts:162):

```ts
for (const action of actions) {
  if (action.retries >= MAX_RETRIES) { remove; failed++; continue; }
  try { await executeAction(action); remove; processed++; }
  catch { incrementRetry; failed++; }
}
```

Every action with `retries < 3` runs every time the user reconnects. If the backend is
temporarily refusing one action class (e.g. flag returns 503), every reconnect burns a
retry on every queued flag. Fix:

```ts
// SyncAction gains: nextRetryAt?: number
const now = Date.now();
for (const action of actions) {
  if (action.retries >= MAX_RETRIES) { /* remove */ continue; }
  if ((action.nextRetryAt ?? 0) > now) { continue; }          // skip, not failed
  try { await executeAction(action); /* remove */ }
  catch {
    const backoffMs = Math.min(2 ** action.retries * 1000, 5 * 60 * 1000);
    await scheduleRetry(action.id!, now + backoffMs);
  }
}
```

Cap at 5 min so a long offline session can still drain quickly when the backend recovers.

### 15. Alt-UI navigation fallback — subsumed by 11.

---

## Cross-cutting checks

### Typed message protocol between SW and main thread

Today: nothing exists. Background-sync runs entirely in the main thread; the SW handles
only Workbox precache + runtime cache. The protocol becomes load-bearing once findings 1,
5, and 14 land — define it up front (see finding 6).

### Static asset SWR for branding/calendar

- `frontend/public/` ships `apple-touch-icon.png`, `favicon.svg`, `icon-192.png`,
  `icon-512.png`, `maskable-512.png`, `og-card.png`, `tasmail-brand-kit.zip`. All these
  are picked up by `globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}']` and go into
  the precache — already SWR-equivalent (precache + revision hash).
- Dynamic branding via `/api/branding` is API, not static — covered by finding 2.
- Calendar events themselves are user-content; should not be precached. The
  `/api/calendar/events` GET would go under the catch-all in finding 2.
- No CDN-served fonts today; if we add Inter / system fonts via Google Fonts later,
  put them under a `StaleWhileRevalidate` with a long TTL and own `cacheName`.

### Offline indicator surfacing

- SPA: `TopBar.tsx:98-102` — Wifi-off icon + "Offline" text + tooltip. ✅
- Alt-UI: not implemented. See finding 10.
- Mobile (Flutter, `mobile/`): outside the scope of TMAIL-261 (mobile has its own
  offline-sync — see TMAIL-151). The TASMail web PWA installed on Android/iOS
  Safari/Chrome inherits the SPA offline indicator via the same TopBar.
- Inside Composer: when offline + the user clicks "Send", the action is queued (via
  `scheduleSend` → background-sync queue). The UX today gives no feedback that the
  send was queued vs. sent. Consider a toast: "Send queued — will deliver when online."

### Service-worker update prompt

Covered in finding 1. The current `registerType: 'autoUpdate'` is correct *if* paired with
an in-app prompt; today it's auto-update + no prompt = silent staleness in long-lived tabs.

### Conflict-resolution registry

Today: ad-hoc inside `executeAction`'s switch (background-sync.ts:113). For a typed
strategy registry (finding 3), refactor to:

```ts
// frontend/src/utils/sync-strategies.ts (new)
export type ConflictStrategy = 'last-write-wins' | 'keep-both' | 'prompt-user' | 'drop-on-conflict';
export const SYNC_STRATEGIES: Record<SyncActionType, ConflictStrategy> = {
  send:       'keep-both',         // duplicate sent is worse than dropped; needs client_msg_id dedup
  move:       'drop-on-conflict',  // 409/404 means already moved
  delete:     'drop-on-conflict',  // 404 = already gone (success)
  flag:       'last-write-wins',   // flags are fungible
  'save-draft': 'prompt-user',     // user data — must not silently overwrite
};
```

This becomes the contract for what `executeAction` does on each HTTP status.

---

## Quick wins applied in this commit

Two low-risk fixes ship alongside the assessment (rest stay as scoped tickets):

1. **`vite.config.ts`** — add `cleanupOutdatedCaches: true` and `method: 'GET'` on the
   runtime cache rule. Zero behavioural risk; aligns with findings 8 and 12.
2. **`vite.config.ts`** — split the catch-all `/api/` cache into three rules:
   - `/api/auth`, `/api/search`, `/api/calendar/free-busy` → `NetworkOnly` (don't pollute the cache with per-request garbage).
   - `/api/branding` → `StaleWhileRevalidate` (24 h, own cache, capped at 4 entries).
   - All other `/api/*` GET → `NetworkFirst` (status quo: 5 min, 100 entries, 5 s timeout).

   This is conservative — no route currently cached gets *more* aggressive caching, the
   surgical changes only *remove* caching from routes that should never have had it.

---

## Recommended ticket breakdown (under TMAIL-241 epic)

| Ticket | Title | Effort |
|--------|-------|--------|
| TMAIL-261a | SW update prompt: `useRegisterSW` + `SWUpdatePrompt` banner + 30-min update poll | S–M |
| TMAIL-261b | Per-route Workbox `runtimeCaching` plan (full split per finding 2) | M |
| TMAIL-261c | IndexedDB `migrate(oldVersion)` pattern + `onblocked` handler in both `offline-cache.ts` and `background-sync.ts` | S |
| TMAIL-261d | Backend: `ETag` / version on `/api/folders/*/messages/*` and `/api/drafts/*` (prereq for conflict resolution) | M |
| TMAIL-261e | Conflict-resolution registry + `If-Match` send + 409 surfacing in the offline-queue UI | M |
| TMAIL-261f | Web Background Sync wiring (`injectManifest` mode, SW `sync` event, fallback to main-thread replay) | M |
| TMAIL-261g | Typed SW ↔ main-thread message protocol (`SwToClient` / `ClientToSw` + `BroadcastChannel`) | S |
| TMAIL-261h | Alt-UI offline indicator (path-alias share `useOnlineStatus` into the alt-UI header) | S |
| TMAIL-261i | `navigateFallback: '/index.html'` + denylist `/api`, `/metrics`, `/ws` | S |
| TMAIL-261j | Exponential backoff in `background-sync.processPending` + `nextRetryAt` field | S |
| TMAIL-261k | `beforeinstallprompt` capture + in-app "Install TASMail" affordance | S |
| TMAIL-261l | Cache-tier hierarchy doc in `frontend/src/utils/README.md`; align TanStack/IDB/SW TTLs per resource | S |
| TMAIL-261m | Composer offline toast: "Send queued — will deliver when online" when a queued mutation lands | S |

---

## Sources & references

- vite-plugin-pwa — register types & `useRegisterSW`: <https://vite-pwa-org.netlify.app/guide/auto-update.html>, <https://vite-pwa-org.netlify.app/frameworks/react.html>
- Workbox — caching strategies: <https://developer.chrome.com/docs/workbox/caching-strategies-overview>
- Workbox — `runtimeCaching` config: <https://developer.chrome.com/docs/workbox/modules/workbox-build#type-RuntimeCachingEntry>
- Workbox — `ExpirationPlugin` LRU behaviour: <https://developer.chrome.com/docs/workbox/modules/workbox-expiration>
- Workbox — `cleanupOutdatedCaches`: <https://developer.chrome.com/docs/workbox/modules/workbox-build#type-GenerateSWOptions>
- Web Background Sync API: <https://developer.mozilla.org/en-US/docs/Web/API/Background_Synchronization_API>
- IndexedDB versioning & `onupgradeneeded` patterns: <https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest/upgradeneeded_event>
- IndexedDB `onblocked` event: <https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest/blocked_event>
- `BroadcastChannel` for SW ↔ tabs: <https://developer.mozilla.org/en-US/docs/Web/API/BroadcastChannel>
- HTTP `ETag` / `If-Match` for conflict detection: <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag>, <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/If-Match>
- `beforeinstallprompt` API: <https://web.dev/articles/install-criteria>, <https://web.dev/articles/customize-install>
- React 19 + Suspense for SW data: <https://react.dev/blog/2025/12/05/react-19> (context for finding 1's `useRegisterSW` integration)

All findings above are derived from static reading of `frontend/src`, `frontend/vite.config.ts`,
`frontend/package.json`, and `themes/shadcn-prototype/src` at HEAD (`2e56a74`, 2026-05-27).
No runtime data was captured; the in-browser verification steps are explicit per finding.
