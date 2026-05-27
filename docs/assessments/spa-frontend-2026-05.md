# SPA Frontend Assessment — May 2026

**Ticket:** TMAIL-254 (axis of TMAIL-241 frontend modularisation review)
**Scope:** `frontend/src/api/`, `frontend/src/stores/`, `frontend/src/hooks/`,
`frontend/src/components/{layout,mail,settings}/`, `frontend/src/utils/`.
**Method:** static read at HEAD plus `rg` counts across `frontend/src`. The seven
sibling assessments listed in **§ Cross-references** already cover bundle size,
PWA/Workbox, render perf, state management, a11y, alt-UI, and the migration
imports — this report focuses on the **four dimensions** they don't:
`ApiClient` shape contracts, Settings router/registry pattern, the hook
data/presentation split, and the background-sync message protocol.

---

## TL;DR

The SPA's structural foundation is solid:

- **`ApiClient` is fully generic.** Zero `Promise<any>`, zero `as any`, zero
  `: any` in production code. Every consumer module asserts an explicit
  response type. Confidence in route shapes lives in `types/` + the
  `api/*.ts` wrappers.
- **Zustand stores are tiny and typed.** Two stores, ~90 lines combined,
  fully-typed setters, no `setState(prev: any)`. Covered in
  `frontend-state-2026-05.md`.
- **Lazy loading of Settings managers is in place** (TMAIL-259). 40 managers,
  one `React.lazy` each, one shared `Suspense`. Initial entry is now 27.86 kB
  gzip.

The **two real structural debts** that aren't fixed elsewhere are:

1. **The Settings shell is a hardcoded `viewMode` ladder, not a registry.**
   Adding one manager touches four files in three repos of conditional logic:
   the `ViewMode` union in `mailStore.ts`, a `lazy()` import + render branch
   in `AppShell.tsx`, a hand-rolled `<button>` + lucide icon + section in
   `Sidebar.tsx` (363 lines), and the manager's own file. This is the
   single biggest source of friction for adding the next 5–10 settings
   surfaces.
2. **`background-sync.executeAction` uses a tagged union for action types but
   a `Record<string, unknown>` for payloads**, with manual `as string` /
   `as number` casts in the switch arms. Add one action type and TypeScript
   gives you no help on payload shape. The protocol *looks* typed; it isn't.

Detail and ROI ranking below.

---

## What was checked

| Axis | Result | Detail |
|---|---|---|
| `ApiClient`: typed response shapes per route or `Promise<any>` | ✅ Pass | Generic `<T>` throughout; zero `any` in production code |
| `apiClient.delete(...)` calls that discard the result | ✅ Safe | T defaults to `unknown`; no value consumed |
| `api/*.ts` modules: do they import named types from `types/` | ✅ Pass | All consumed via `import type` |
| Settings managers: registry vs hardcoded `<Route>` / conditional list | ❌ Hardcoded ladder | 40-arm `viewMode === 'X' && <X />` chain in `AppShell.tsx` |
| Sidebar: data-driven entries or hand-rolled buttons | ❌ Hand-rolled | 363 lines, ~40 repetitive button blocks |
| Admin shell: registry vs hardcoded | ⚠️ Partial registry | Uses `<Route>` children under `AdminShell`; closer to right shape |
| Zustand stores: typed actions, no `setState(prev: any)` | ✅ Pass | Both stores fully typed; see state assessment |
| Hooks split data layer from presentation | ⚠️ Mixed | 3 hooks wrap TanStack Query, 60 components use it inline |
| Every Settings page lazy-loaded | ✅ Pass | TMAIL-259 landed 40 `React.lazy()`s |
| Workbox cache strategy scoped per route group | ⚠️ Partial | Auth/search/branding/free-busy split out (TMAIL-261); folders/messages still on catch-all |
| Background-sync: typed message protocol | ⚠️ Tag typed, payload untyped | `SyncActionType` is a string union but `payload: Record<string, unknown>` |

Numbers from `rg` over `frontend/src`:

- `Promise<any>` occurrences: **0**
- `: any` in type positions (excluding JSDoc/tests): **0**
- `as any`: **0**
- `apiClient.<method>(...)` calls without a generic, in `api/`: 10, all are
  `await`-and-discard delete/post, so `T = unknown` is safe
- API modules in `api/`: 64 (`*.ts` excluding tests), each one a thin
  wrapper over `apiClient`
- Settings managers in `components/settings/`: 45 component files (40 wired
  into the `AppShell` ladder + 5 sub-components such as `CalendarView`,
  `ContactsApp`, `EdiscoveryManager`'s detail views)
- Components using `useQuery`/`useMutation`/etc.: **60**
- Hooks using `useQuery`: **3** (`useMailbox`, `useBranding`, `useSearchUrlSync`)

---

## 1. `ApiClient` — typed contracts per route

`api/client.ts` is 141 lines, exposes one class instance:

```ts
class ApiClient {
  private async request<T>(path: string, options: RequestInit = {}): Promise<T> { … }
  get<T>(path: string): Promise<T>           { return this.request<T>(path); }
  post<T>(path: string, body?: unknown): Promise<T> { … }
  put<T>(path: string, body?: unknown): Promise<T>  { … }
  patch<T>(path: string, body?: unknown): Promise<T>{ … }
  delete<T>(path: string, body?: unknown): Promise<T> { … }
}
```

**Why this is fine:**

- The generic is at the call site, not the class. Each `api/*.ts` wrapper
  asserts the response shape: `apiClient.get<MessageListResponse>(...)`,
  `apiClient.post<{ id: string }>(...)`, etc.
- The class has no opinion about shape; that responsibility is delegated
  to the per-domain modules, which is the right place for it — the
  `ApiClient` doesn't need to know about 64 different response types.
- `unknown` (the default when T is omitted) is sound, not unsafe. The
  10 untyped call sites I found all `await` the call and discard the
  result, which type-checks cleanly.

**Sample contract — `api/messages.ts`:**

```ts
import type { FullMessage, MessageListResponse, SearchResponse, SendEmailRequest } from '../types/mail';
import { apiClient } from './client';

export async function fetchMessages(folder, page, pageSize): Promise<MessageListResponse> {
  return apiClient.get<MessageListResponse>(`/folders/.../messages?page=...`);
}

export async function fetchMessage(folder, uid): Promise<FullMessage> {
  return apiClient.get<FullMessage>(`/folders/.../messages/${uid}`);
}

export async function sendMessage(request: SendEmailRequest): Promise<void> {
  await apiClient.post('/messages/send', request);
}
```

Each function declares its response, imports the type from `types/`, and the
generic on `apiClient.get` matches the declared return. No drift. **The
discipline is consistent across all 64 modules.**

**One minor cleanup worth flagging:**

`request<T>` has two `return undefined as T` branches (204 and empty-body)
for routes whose declared return is `void`. The cast is reasonable but
silently coerces `undefined → T` for any `T`. If a caller writes
`apiClient.get<{ id: string }>('/204-endpoint')` they get back `undefined`
typed as `{ id: string }`. The mitigation today is that the type at the
call site matches reality (every `apiClient.post('/messages/send', …)` is
typed `Promise<void>` because `sendMessage` returns `Promise<void>`).

If you want to harden this, narrow the empty-body branches to
`Promise<undefined>` and force callers that expect a body to keep the
generic explicit. Cost is low; benefit is also low (no current bug). Park
unless someone trips over it.

### Verdict — section 1

✅ **No action.** The shape contract is enforced by the per-domain
wrappers + the `types/` interfaces, and there is zero `any` in the way.
This is the shape we want; future endpoints should keep wrapping at the
`api/*.ts` layer rather than calling `apiClient` directly from components.

---

## 2. Settings shell — hardcoded ladder, not a registry

**This is the most significant structural finding in this report.** It is
also the most fixable.

### What the code looks like today

Adding (for example) the "Push Devices" settings page (TMAIL-204) required
four edits:

1. **`stores/mailStore.ts`** — append `'push-devices'` to the 41-string
   `ViewMode` union.
2. **`components/layout/AppShell.tsx`** — add a `lazy()` import:

   ```ts
   const PushDevicesManager = lazy(() => import('../settings/PushDevicesManager')
     .then((m) => ({ default: m.PushDevicesManager })));
   ```

   and add a render branch inside the ~40-arm conditional ladder:

   ```tsx
   {viewMode === 'push-devices' && <PushDevicesManager />}
   ```

3. **`components/layout/Sidebar.tsx`** (363 lines) — append a lucide-react
   icon import to the giant import line and add a hand-rolled button block:

   ```tsx
   <button
     className={`folder-item ${viewMode === 'push-devices' ? 'folder-item--active' : ''}`}
     onClick={() => handleNavClick('push-devices')}
   >
     <Bell size={18} />
     <span className="folder-item__name">Push Devices</span>
   </button>
   ```

4. The new manager file under `components/settings/`.

Steps 1–3 are all *coupling tax* — none of them are about the new feature.
Each is a separate place where a typo silently breaks the link between the
sidebar button and the rendered view. Across 40 entries, that's 40 chances
for the union string in `mailStore.ts` to drift from the render arm in
`AppShell.tsx` to the click handler in `Sidebar.tsx`.

**By contrast, the admin shell (TMAIL-197) is closer to right:**

```tsx
// App.tsx
<Route path="/admin" element={<AdminShell />}>
  <Route index element={<Navigate to="feature-flags" replace />} />
  <Route path="feature-flags"     element={<FeatureFlagsManager />} />
  <Route path="quote-requests"    element={<QuoteRequestsManager />} />
  <Route path="audit-log"         element={<AuditLogManager />} />
  <Route path="cache"             element={<CacheManager />} />
  <Route path="domains"           element={<DomainsManager />} />
  <Route path="payment-providers" element={<PaymentProvidersManager />} />
  <Route path="users"             element={<UsersManager />} />
  <Route path="warmup"            element={<WarmupManager />} />
</Route>
```

URL-driven, browser back/forward works, no `ViewMode` enum to maintain,
no `Sidebar` conditional. The admin shell's own internal sidebar (not
read here) likely reads the route, but even if it's still hardcoded, the
*route table* is the single source of truth.

### What "registry" looks like for the user settings

A data-driven registry is two arrays:

```ts
// settings-registry.tsx
export interface SettingsEntry {
  id: SettingsId;        // discriminator, used in URL or viewMode
  title: string;
  icon: LucideIcon;
  section: 'mail' | 'identity' | 'security' | 'admin' | 'integrations' | 'comms';
  component: React.LazyExoticComponent<React.ComponentType>;
  feature?: string;      // optional feature-flag gate
  adminOnly?: boolean;   // moves it under /admin instead
}

export const SETTINGS_REGISTRY: readonly SettingsEntry[] = [
  { id: 'signatures', title: 'Signatures', icon: FileSignature, section: 'identity',
    component: lazy(() => import('../settings/SignatureManager').then(m => ({ default: m.SignatureManager }))) },
  { id: 'contacts',   title: 'Contacts',   icon: Users,         section: 'identity',
    component: lazy(() => import('../settings/ContactManager').then(m => ({ default: m.ContactManager }))) },
  // … 38 more
] as const;

export type SettingsId = typeof SETTINGS_REGISTRY[number]['id'];
```

`Sidebar.tsx` becomes:

```tsx
const groups = groupBy(SETTINGS_REGISTRY, e => e.section);
return groups.map(group => (
  <SidebarSection key={group.section} title={group.title}>
    {group.entries.map(entry => (
      <SidebarLink key={entry.id} entry={entry} active={viewMode === entry.id} onClick={handleNavClick} />
    ))}
  </SidebarSection>
));
```

`AppShell.tsx`'s conditional ladder becomes:

```tsx
<Suspense fallback={<ViewLoading />}>
  {(() => {
    const entry = SETTINGS_REGISTRY.find(e => e.id === viewMode);
    if (!entry) return null;
    const Component = entry.component;
    return <Component />;
  })()}
</Suspense>
```

`SettingsId` is now derived from the registry — `ViewMode` either disappears
or becomes `'list' | 'reader' | 'compose' | 'search' | SettingsId`, which is
self-maintaining.

### Migration steps (suggested ticket scope)

1. Land `frontend/src/components/settings/settings-registry.ts` with the
   40 entries pulled from `AppShell.tsx`'s current `lazy()` imports.
2. Rewrite `AppShell.tsx`'s ladder as a registry lookup (~10 lines).
3. Rewrite `Sidebar.tsx`'s 363-line button list as a `.map()` over the
   registry, grouped by `section` (≤ 80 lines).
4. Migrate `ViewMode` to `'list' | 'reader' | 'compose' | 'search' |
   SettingsId`, with `SettingsId` derived from the registry. Compiler
   immediately surfaces any string that doesn't have a registry entry.
5. Optional: change `setViewMode(id)` to `navigate(\`/app/settings/${id}\`)`
   and follow the admin-shell pattern fully. Adds browser back/forward
   support but is a separate change with its own UX implications (some
   managers expect to be openable from a CTA on another manager — those
   would need a `to=` prop).

Effort estimate: half a day for steps 1–4, separate ticket for step 5.

### Verdict — section 2

❌ **Take action.** This is the single biggest structural cleanup
available in the SPA right now. The admin shell already shows the shape;
the user settings just need to follow it. Effort is small (one new file,
two existing files shrink), and the payoff compounds: every future
settings manager (TMAIL-142, -149, -150, -151, -152, all the mobile-led
features that grow user settings, plus whatever lands after) becomes
one registry row instead of four edits in three files.

---

## 3. Hooks: data-layer / presentation split

The HARD RULE in `~/.claude/rules/all-rules.md` says
"separate **data layer** (hooks/queries) from **presentation layer**
(components)". Today the SPA is mixed:

- 3 hooks own data fetching: `useMailbox` (folders, messages, search),
  `useBranding`, `useSearchUrlSync`.
- 60 components call `useQuery` / `useMutation` directly inline.

That's the same pattern the state assessment flagged for the settings
managers — `useQuery` + local `useState` mirror copies. Some of that is
fine: a one-page settings manager that owns its own fetch + form + save
mutation is genuinely self-contained, and extracting `useFooSettings()`
would just be moving the same code one file over.

The cases where the split *does* matter, and isn't done today:

| Pattern in code | Why it's worth extracting |
|---|---|
| `MessageView.tsx` has five `useMutation`s (delete, move, flag, exportEml, phishing-scan, phishing-action) | The state assessment already proposed `useMessageActions(folder)` here so the toolbar mutations and `useKeyboardShortcuts` share one definition (today they drift). |
| `Composer.tsx` has its own draft-save logic with `useMutation` + a `setTimeout`-driven autosave loop | Extracting `useDraftAutosave(draft)` keeps the Composer presentational and lets the same hook power `LargeFileAttacher` and the alt-UI's `ComposeModal`. |
| `QueueManager`, `MigrationManager`, `PstImport` all wire `refetchInterval` polling inline | `useLiveResource(key, fetcher, intervalMs)` would centralise the "poll until done" pattern; today it's repeated three times. |
| 30+ settings managers each have the same `useQuery → useEffect → useState → useMutation` form-mirror loop | The state assessment proposed killing the form-mirror anti-pattern; the natural form of that fix is `useSettingsResource(key, fetcher, mutator)` returning `{ data, dirty, save, reset }`. |

Outside those four hot spots, leaving `useQuery` inline in a one-off
settings manager isn't a code smell — it's just colocation. Don't extract
hooks for the sake of it.

### Verdict — section 3

⚠️ **Selective action.** Three new hooks would cover ~90 % of the
"shared mutation logic in two places" cases in the app:

1. `useMessageActions(folder)` — the four message-row mutations as a
   shared `useMutation` pack (already proposed in
   `frontend-state-2026-05.md`).
2. `useDraftAutosave(draft)` — extracted from `Composer.tsx`'s inline
   debounce + mutation, then reused in `LargeFileAttacher` and alt-UI.
3. `useLiveResource(queryKey, fetcher, intervalMs)` — DRY out the
   poll-until-done pattern used in 3 long-running operation managers.

Leave the rest of the settings managers' inline `useQuery` alone until
the form-mirror anti-pattern fix (state assessment recommendation 5)
forces them through a common hook anyway.

---

## 4. `utils/background-sync.ts` — tagged but payload-untyped

The action protocol today:

```ts
export type SyncActionType = 'send' | 'move' | 'delete' | 'flag' | 'save-draft';

export interface SyncAction {
  id?: number;
  type: SyncActionType;
  payload: Record<string, unknown>;
  createdAt: number;
  retries: number;
}
```

`executeAction` then casts each field at use:

```ts
case 'move': {
  const { moveMessage } = await import('../api/messages');
  await moveMessage(
    p.folder as string,
    p.uid as number,
    p.toFolder as string,
  );
  break;
}
case 'send': {
  const { scheduledApi } = await import('../api/scheduled');
  await scheduledApi.scheduleSend({
    to: p.to as string[],
    subject: p.subject as string,
    text_body: p.text_body as string | undefined,
    html_body: p.html_body as string | undefined,
    cc: p.cc as string[] | undefined,
    bcc: p.bcc as string[] | undefined,
    delay_seconds: p.delay_seconds as number | undefined,
  });
  break;
}
```

**The discriminator is typed; the payload is not.** Add a new
`SyncActionType` and TypeScript will narrow the `case` exhaustiveness but
nothing about the payload. Rename `to → recipients` server-side and the
`as string[]` cast still compiles — the bug surfaces at runtime when the
queue replays into a stranger's outbox.

### Proposed shape

A discriminated union, payload-per-type:

```ts
export type SyncAction =
  | { id?: number; createdAt: number; retries: number;
      type: 'send';
      payload: { to: string[]; subject: string; text_body?: string; html_body?: string;
                 cc?: string[]; bcc?: string[]; delay_seconds?: number } }
  | { id?: number; createdAt: number; retries: number;
      type: 'move';
      payload: { folder: string; uid: number; toFolder: string } }
  | { id?: number; createdAt: number; retries: number;
      type: 'delete';
      payload: { folder: string; uid: number } }
  | { id?: number; createdAt: number; retries: number;
      type: 'flag';
      payload: { folder: string; uid: number; flag: string; add: boolean } }
  | { id?: number; createdAt: number; retries: number;
      type: 'save-draft';
      payload: { to: string[]; subject: string; cc?: string[];
                 html_body?: string; text_body?: string } };
```

`executeAction` switches on `action.type` and TypeScript narrows `payload`
in each arm — every `as string` cast goes away, and any payload-shape
drift becomes a compile error.

Move-by-move:

1. Introduce the union (above) and replace the four `as` casts in each
   arm with direct field reads.
2. `enqueue(type, payload)` becomes overloaded so each `type` accepts
   only the right payload shape — callers (today inside Composer,
   MessageView, useKeyboardShortcuts when offline) immediately get
   "Argument of type X is not assignable" if they pass the wrong shape.
3. The IDB-stored records keep working unchanged at the wire level since
   the structure is identical; the only difference is at the type
   boundary.

### Cross-reference — PWA assessment finding 6

`frontend-pwa-offline-2026-05.md` finding 6 already calls out the
**SW ↔ main-thread** message protocol (which is unstructured-today and
needs `{ type, payload, msgId }`). The fix in this section is the
**main-thread offline-queue** protocol. Both deserve the same
discipline. When the SW-side `sync` event handler is added (PWA
finding 5), it will need to call into `processPending()` from the SW
context — at that point both protocols want to share the same `SyncAction`
union.

### Verdict — section 4

⚠️ **Action recommended.** Cost is one file, ~40 lines of net new
type, zero behavioural change. Payoff is that the offline queue stops
silently disagreeing with the API modules on payload shape — which is
the failure mode most likely to lose user data without any signal.

---

## 5. Items already covered by sibling assessments

The TMAIL-254 brief asks about bundle size, Workbox cache strategy,
and Zustand actions — those are not re-litigated here.

| Question | Answered in | Verdict |
|---|---|---|
| Every Settings page lazy-loaded? | `frontend-bundle-2026-05.md` (TMAIL-259) | ✅ All 40 are `React.lazy()` |
| Composer / FullCalendar lazy-loaded? | `frontend-bundle-2026-05.md` | ✅ Both behind their own chunks |
| Workbox scoped per route group? | `frontend-pwa-offline-2026-05.md` (TMAIL-261) | ⚠️ Partial — auth/search/branding/free-busy split; folders/messages still catch-all |
| Zustand stores have typed actions, no `setState(prev: any)` | `frontend-state-2026-05.md` (TMAIL-258) | ✅ Both stores fully typed |
| Optimistic updates on read/unread, star, delete | `frontend-state-2026-05.md` | ❌ Only `FeatureFlagsManager` (1/N) |
| Render-perf hot spots (memoisation, key churn) | `frontend-render-perf-2026-05.md` (TMAIL-260) | Mixed — see report |
| A11y across the SPA | `frontend-a11y-2026-05.md` (TMAIL-260) | Mixed — see report |
| Alt-UI (`themes/shadcn-prototype/`) status | `alt-ui-2026-05.md` (TMAIL-255) | EmailClient wired; CalendarView + AdminDashboard still on mock data |
| Backend ↔ SPA route imports (orphans, drift) | `migration-imports-2026-05.md` (TMAIL-256) | Tracked via baseline + CI gate |

---

## Recommendations — ranked by ROI

### Quick wins (under a day each)

1. **Type the `SyncAction` union by payload, per `type` arm.** One file,
   eliminates the `as string` cast cluster in `background-sync.ts`,
   makes payload-shape drift a compile error. (§ 4)
2. **Extract `useMessageActions(folder)`.** Already proposed in the state
   assessment; this report adds the data-layer-split justification. Use
   the `FeatureFlagsManager` optimistic-update pattern as the template
   so the four message-row mutations land typed + optimistic in one go. (§ 3)
3. **Extract `useDraftAutosave(draft)` and `useLiveResource(key, fn, ms)`.**
   Each kills duplicated logic in two consumers. (§ 3)

### Medium (a day or two)

4. **Build `settings-registry.ts` and rewrite `AppShell.tsx` +
   `Sidebar.tsx` against it.** Biggest structural payoff in the SPA;
   pattern is already in production for the admin shell. (§ 2)

### Larger (separate ticket)

5. **URL-driven settings routes.** Once the registry is in place,
   converting `setViewMode(id)` to `navigate(/app/settings/${id})` is
   small per-file but touches every CTA that opens a settings page.
   Defer; the registry stands alone without it.
6. **Workbox per-route cache for `folders` and `messages`.** The PWA
   assessment recommends this; today they still ride the catch-all
   `NetworkFirst`. Best to land alongside the broader Workbox rework
   in the TMAIL-261 follow-up tickets.

---

## Cross-references

| File | Issue | Owns |
|---|---|---|
| `docs/assessments/frontend-bundle-2026-05.md` | TMAIL-259 | Bundle sizing, lazy splits, manualChunks |
| `docs/assessments/frontend-pwa-offline-2026-05.md` | TMAIL-261 | Service worker, Workbox, IndexedDB, offline queue UX |
| `docs/assessments/frontend-state-2026-05.md` | TMAIL-258 | Zustand stores, hooks, optimistic updates, form-mirror |
| `docs/assessments/frontend-render-perf-2026-05.md` | TMAIL-260 | Re-render hotspots, memoisation |
| `docs/assessments/frontend-a11y-2026-05.md` | TMAIL-260 | ARIA, contrast, keyboard nav |
| `docs/assessments/alt-ui-2026-05.md` | TMAIL-255 | shadcn prototype status |
| `docs/assessments/migration-imports-2026-05.md` | TMAIL-256 | Trace-check gate, orphan baseline |

---

## Verification

Re-runnable from `frontend/`:

```bash
# Any-types in production code (should print 0 / 0 / 0)
rg -c 'Promise<any>' src --type ts --type tsx | awk -F: '{s+=$2} END{print s+0}'
rg -c '\bas any\b' src --type ts --type tsx | awk -F: '{s+=$2} END{print s+0}'
rg -c ':\s*any\b' src --type ts --type tsx -g '!*.test.*' | awk -F: '{s+=$2} END{print s+0}'

# Settings managers vs registry
rg -c "viewMode === '" src/components/layout/AppShell.tsx          # expect ~40
wc -l src/components/layout/Sidebar.tsx                            # expect ~360
test -f src/components/settings/settings-registry.ts && echo OK    # expect not yet

# Hook split
rg -l 'useQuery|useMutation' src/hooks | wc -l                     # expect 3
rg -l 'useQuery|useMutation' src/components | wc -l                # expect ~60

# Background-sync typing
rg "payload: Record<string, unknown>" src/utils/background-sync.ts # expect 1
rg "as string\|as number\|as boolean" src/utils/background-sync.ts # expect ~12
```

Re-run after the recommendations above land. The registry check
(`settings-registry.ts` existing) and the `as string` count in
`background-sync.ts` are the two most informative signals — if neither
changes, recommendations 1 and 4 haven't been picked up.
