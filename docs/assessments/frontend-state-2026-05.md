# Frontend State Management Assessment — May 2026

**Ticket:** TMAIL-258 (axis of TMAIL-241 frontend modularisation review)
**Scope:** `frontend/src/stores/`, `frontend/src/hooks/`, and every component
that uses `useState` / `useReducer` / `useEffect` for what could be query state
or shared store state.
**Method:** static read of every store + hook file, plus a sample of
~10 representative components (high-traffic mail surfaces + a sample of
settings managers). Counts come from `rg` across `frontend/src`.

---

## TL;DR

The state-management story is mostly healthy: stores are tiny and
single-responsibility, TanStack Query is the dominant server-state
mechanism (58 components, every server resource), every effect with a
subscription has a matching cleanup, and there is **not a single**
`// eslint-disable-next-line react-hooks/exhaustive-deps` in the
repository. There is also **no `useReducer`** at all — flat `useState`
is uniformly used.

The two real weak points are:

1. **Optimistic updates are almost entirely absent.** Only
   `FeatureFlagsManager.tsx` uses `onMutate` / `setQueryData` / rollback.
   The high-traffic mail actions (star, mark-read, delete, archive,
   move) all wait for the server round-trip plus a `messages` refetch
   before the UI updates. The keyboard-shortcut path is even worse — it
   bypasses the React Query cache entirely and fires-and-forgets.
2. **Form-mirror anti-pattern in ~30 settings managers.** They `useQuery`
   the server resource, then copy each field into local `useState` on
   load via `useEffect`. The local copy can drift from the cache after
   an invalidation, and adding a field means editing two places.

Three quick wins below buy real UX latency and a per-render cost
reduction without touching architecture.

---

## What was checked

| Axis | Result |
|---|---|
| Zustand stores: single-responsibility | ✅ Pass |
| Server state stored in Zustand instead of TanStack Query | ✅ None found |
| Derived state in components vs `useMemo` / store selector | ⚠️ Mixed |
| `staleTime` / `gcTime` tuned per route or defaults everywhere | ✅ Tuned per query |
| Optimistic updates on read/unread, star, delete | ❌ Missing (1 of N) |
| Subscription leaks (WebSocket, intervals, listeners) | ✅ Clean |
| `useEffect` dependency arrays clean (no eslint-disable) | ✅ Zero disables |
| `useReducer` used anywhere | ❌ None (flat `useState` everywhere) |

Numbers from `rg`:

- `useQuery` / `useMutation` / `useQueryClient` / `useInfiniteQuery`: 58 components
- `useState(` in `src/components/`: 337 calls across 68 files
- `useMemo` / `useCallback` in `src/components/`: 57 calls across 16 files
- `onMutate` / `setQueryData` / `previousData`: 1 component (`FeatureFlagsManager.tsx`)
- `// eslint-disable-next-line react-hooks/exhaustive-deps`: 0
- `useReducer`: 0
- `addEventListener` / `setInterval` / `setTimeout` cleanup pairs: every occurrence has a matching `removeEventListener` / `clearInterval` / `clearTimeout` in the effect return

---

## Zustand stores

### `mailStore.ts` (65 lines)

```ts
interface MailState {
  selectedFolder: string;
  selectedUid: number | null;
  viewMode: ViewMode;
  searchQuery: string;
  advancedSearch: AdvancedSearchParams | null;
  // … five setters
}
```

**Single-responsibility:** ✅ — only mail navigation and search state.
No quota, no message bodies, no folders, no contacts. Server data is
delegated to TanStack Query.

**State transitions encoded in the setters** (e.g. `setSelectedFolder`
also clears `selectedUid` and resets `viewMode` to `'list'`) — good,
keeps related fields atomic.

**The one thing to flag:** `ViewMode` is a 41-string union. Every new
settings page widens the union. The store is now the de facto router for
the entire app, and the URL is not synced (except `?q=...` via
`useSearchUrlSync`). Hitting the browser back button after opening
"Webhooks" does not return you to the inbox — it leaves the SPA. This is
not a state-management bug per se, but it is the natural consequence of
making `viewMode` a store-owned enum instead of a URL segment. See
"Recommendations" below.

### `uiStore.ts` (22 lines)

```ts
interface UiState {
  sidebarOpen: boolean;
  theme: 'light' | 'dark';
  // … three setters
}
```

**Single-responsibility:** ✅ — only chrome UI. Theme reads from
`localStorage` at module init and writes on toggle, which is correct
for first-paint but means two open tabs do not sync theme until a
reload. Acceptable.

### Test coverage

Both stores have colocated `*.test.ts` files (`mailStore.test.ts` 82
lines, `uiStore.test.ts` 52 lines). ✅

---

## TanStack Query configuration

`App.tsx` configures the singleton client:

```ts
new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false },
  },
});
```

No global `staleTime` / `gcTime`. Every query in the codebase sets its
own `staleTime`. Sample:

| Query | `staleTime` | `refetchInterval` | Notes |
|---|---|---|---|
| `useFolders` | 30 s | — | high traffic |
| `useMessages` | 15 s | — | invalidated by WebSocket on `new_mail` |
| `useMessage` | (none) | — | reader view; relies on `enabled` gating |
| `useSearch` / `useAdvancedSearch` | 30 s | — | |
| `useBranding` | 1 h | — | rarely changes; invalidated on admin save |
| Quota (`QuotaBar`) | 2 min | 5 min | poll fallback for WebSocket |
| Queue (`QueueManager`) | (none) | 10 s | live progress |
| Migration / PST import | (none) | 5 s | long-running operations |
| Ollama / Phishing report | 60 s | — | |
| Feature flags | 60 s | — | |

This is tuned, not blanket. ✅

**`refetchOnWindowFocus: false`** is a deliberate choice — the
WebSocket (`useWebSocket.ts`) invalidates `folders` / `messages` /
`quota` on push events, so tab focus does not need to trigger a refetch.
Combined with the polling fallbacks above, the staleness model is
defensible.

---

## Hooks audit (`frontend/src/hooks/`)

12 production hooks, ~1100 lines including tests. All have colocated
test files. Findings:

### ✅ `useMailbox.ts` — exemplary

Pure TanStack Query wrappers with offline-cache fallback. Folders /
messages / message / search / advanced-search all share one pattern
(`fetchWithCache → cache → IndexedDB fallback on throw`). `useCurrentMessages`
and `useCurrentMessage` are derived selectors that subscribe to the
store and call the underlying query.

### ✅ `useBranding.ts` — exemplary

`useQuery` for `getBranding`, then a single `useEffect([query.data])`
that applies CSS variables, document title, favicon, and custom CSS.
Cache-key shared with `BrandingManager` so admin saves invalidate the
running app.

### ⚠️ `useWebSocket.ts` — one footgun

The connection effect depends on `connect`, which depends on
`token`, `onEvent`, `reconnectDelay`, `queryClient`. If a caller passes
an inline arrow function as `onEvent`, **every parent render re-creates
`onEvent`, which re-creates `connect`, which triggers the `useEffect`
to close and re-open the socket**. Result: a parent re-rendering more
than once a second silently drops every WebSocket message.

Today nobody passes `onEvent` (no caller in the repo provides it), so
this is latent — but it is a footgun the moment someone wires up
"toast on new mail." Fix is one of:

- accept `onEvent` via a ref the effect reads but doesn't depend on,
- or wrap `onEvent` consumers in `useCallback` and document it.

### ⚠️ `useKeyboardShortcuts.ts` — bypasses the query cache

`#` (delete), `e` (archive), `s` (star) call the API directly:

```ts
deleteMessage(selectedFolder, selectedUid).then(() => setSelectedUid(null));
moveMessage(selectedFolder, selectedUid, 'Archive').then(...);
flagMessage(selectedFolder, selectedUid, '\\Flagged', true);
```

No `queryClient.invalidateQueries`. The user deletes a message via
keyboard, then sees it linger in `MessageList` until the next 15 s
`messages` stale tick or a WebSocket `new_mail`. The mouse path
(`MessageView` toolbar) goes through `useMutation` with `onSuccess →
invalidate`, so the two paths drift.

Fix: extract a shared `useMessageActions(folder)` hook returning
`{ del, move, flag, archive }` as `useMutation` objects, and have both
the toolbar and keyboard handler invoke them.

### ✅ The rest

`useAuth`, `useDragAndDrop`, `useLowBandwidth`, `useMediaQuery`,
`useOnlineStatus`, `useResponsive`, `useSearchUrlSync` — each is
narrowly scoped, has tests, and cleans up its subscriptions. Of note,
`useOnlineStatus` uses `useSyncExternalStore` (the correct React 19
primitive for external store subscriptions) — well done.

---

## Component patterns — sampled

### ✅ `MessageList.tsx`

- Subscribes to store via three discrete selectors (folder, uid,
  setSelectedUid) instead of pulling the whole state — minimises
  re-render scope.
- `useMemo([data?.messages])` for thread grouping — correct.
- TMAIL-263 fix already in place (`key={thread.messages[0]?.uid}` not
  array index).
- One local `useState` for `threaded` (UI-only toggle) and a `useRef`
  for the hidden EML file input. Correct.
- The EML import mutation invalidates `['messages']` and `['folders']`.

### ⚠️ `MessageView.tsx`

Five `useMutation`s (delete, move, flag, exportEml, phishing-scan,
phishing-action) all using **pessimistic** `onSuccess → invalidate`.
The flag (star) mutation invalidates only `['message']`, not
`['messages']`, so the star icon flips in the reader but the list view
keeps the stale star until the next list refetch.

### ⚠️ `Composer.tsx` (392 lines, 11 `useState` calls)

```ts
const [to, setTo] = useState('');
const [cc, setCc] = useState('');
const [subject, setSubject] = useState('');
const [sending, setSending] = useState(false);
const [error, setError] = useState('');
const [draftStatus, setDraftStatus] = useState<'idle' | 'saving' | 'saved'>('idle');
const [undoState, setUndoState] = useState<…>(null);
const [showSchedulePicker, setShowSchedulePicker] = useState(false);
const [scheduleDate, setScheduleDate] = useState('');
const [showAiCompose, setShowAiCompose] = useState(false);
const [showLargeFile, setShowLargeFile] = useState(false);
const [showMeetingModal, setShowMeetingModal] = useState(false);
```

11 separate React state slots in one component. Typing a single
character in the "To" field triggers a render through all 11 hooks.
Three options to clean this up, in order of cost:

1. **Cheapest:** group the four `show*` booleans into a single
   `panel` enum (`useState<null | 'schedule' | 'ai' | 'largeFile' | 'meeting'>`).
2. **Medium:** move form fields (`to`, `cc`, `subject`) into a single
   `useReducer({ to, cc, subject })` action object. Reduces dispatches
   and makes "reset on send" a single action.
3. **Heavier:** adopt `react-hook-form` (already in `package.json` per
   the bundle assessment — Composer is the highest-value place to use
   it, and currently doesn't).

### ⚠️ `VacationResponder.tsx` — form-mirror anti-pattern

```ts
const { data: rule } = useQuery<AutoReplyRule | null>({ … });
const [enabled, setEnabled] = useState(false);
const [subject, setSubject] = useState('Out of Office');
// … five more local copies
useEffect(() => {
  if (rule) {
    setEnabled(rule.enabled);
    setSubject(rule.subject);
    // … copy every field from cache to local state
  }
}, [rule]);
```

This pattern repeats across ~30 settings managers (Calendar, Twofactor,
Contacts, Ldap, Saml, Oidc, Dlp, Dane, Archive, ActiveSync, etc.).
Problems:

- After `saveMutation.onSuccess → invalidateQueries`, the form keeps
  showing the **pre-save** values because the local state was never
  re-synced (the effect only fires when `rule` changes by reference,
  which it doesn't if the server returns the same payload).
- Adding a field means: server model, API client, query, **local
  state**, `useEffect` sync, form input, save mutation payload — six
  places.

The fix is **uncontrolled form + initial-values** or a single
`formState` object that resets when the query data updates by version
(server-provided `updated_at` as the dependency, not the whole object).

### ✅ `FeatureFlagsManager.tsx` — the gold standard, copy this

```ts
const toggle = useMutation({
  mutationFn: ({ key, enabled }) => featureFlagsApi.update(key, { enabled }),
  onMutate: async ({ key, enabled }) => {
    await queryClient.cancelQueries({ queryKey: ['admin', 'feature-flags'] });
    const previous = queryClient.getQueryData<FeatureFlag[]>(['admin', 'feature-flags']);
    queryClient.setQueryData<FeatureFlag[]>(['admin', 'feature-flags'], (old) =>
      (old ?? []).map((f) => (f.key === key ? { ...f, enabled } : f))
    );
    return { previous };
  },
  onError: (_err, _vars, ctx) => {
    if (ctx?.previous) queryClient.setQueryData(['admin', 'feature-flags'], ctx.previous);
  },
  onSettled: () => queryClient.invalidateQueries({ queryKey: ['admin', 'feature-flags'] }),
});
```

This is the right shape: cancel in-flight, snapshot, apply, roll back
on error, refetch on settle. Star / mark-read / delete / move / archive
should all use this pattern.

---

## Subscription leak audit

Every `addEventListener` / `setInterval` / `setTimeout` / `editor.on`
that I could find has a matching cleanup in its effect return:

| Hook / component | Subscription | Cleanup |
|---|---|---|
| `useKeyboardShortcuts` | `document.keydown` + pending-key timer | both |
| `useOnlineStatus` | `online` / `offline` | both, via `useSyncExternalStore` |
| `useMediaQuery` | `mql.change` | yes |
| `useWebSocket` | `WebSocket` + reconnect timer | both (the **only** subtle case — see footgun above) |
| `KeyboardShortcutHelp` | `document.keydown` (Escape) | yes |
| `SnoozeMenu` / `ScheduleMeetingModal` | `document.mousedown` / `keydown` | yes |
| `Composer` | auto-save `setTimeout` + undo `setInterval` + tiptap `editor.on('update')` | all three |
| `RecipientAutocomplete` | debounce `setTimeout` + `document.mousedown` | both |

Score: clean. No leaks identified.

---

## Recommendations — ranked by ROI

### Quick wins (under a day each, scoped commits)

1. **Optimistic star (and mark-read).** Add `onMutate` / `setQueryData`
   / `onError` rollback to `flagMut` in `MessageView.tsx` and to the
   mark-read mutation. Pattern: copy `FeatureFlagsManager`. Touches one
   file, fixes the highest-frequency interaction in the app.
2. **Shared `useMessageActions(folder)` hook.** Extract delete / move /
   flag / archive mutations into `frontend/src/hooks/useMessageActions.ts`,
   re-use from `MessageView` toolbar AND `useKeyboardShortcuts`. Kills
   the keyboard-vs-mouse drift and gives one place to add optimistic
   handling.
3. **Stabilise `useWebSocket.onEvent` via ref.** One-line refactor:
   ```ts
   const onEventRef = useRef(onEvent);
   useEffect(() => { onEventRef.current = onEvent; }, [onEvent]);
   // then call onEventRef.current?.(data) inside ws.onmessage
   // and remove onEvent from the useCallback deps
   ```

### Medium (a day or two)

4. **Composer `useReducer` for form fields** + `panel` enum for the
   four `show*` booleans. Mid-effort, low-risk, measurable re-render
   reduction (TMAIL-263 already flagged Composer as a candidate for
   `react-hook-form`).
5. **Kill the form-mirror anti-pattern in settings managers** —
   migrate `VacationResponder`, `BrandingManager`, `LdapManager`,
   `SamlManager`, `OidcManager`, `DlpManager`, etc. to either
   uncontrolled inputs with `defaultValue={query.data?.field}` or a
   single `formState` reducer that resets on query data version
   change. ~30 files but each change is small and self-contained.

### Larger (separate ticket)

6. **URL-driven `viewMode`.** Move the 41-string `ViewMode` enum from
   the store into the router. Each settings page becomes a `Route`,
   browser back/forward works, the store shrinks to (folder, uid,
   search) — the actual mail-state subset. This is a bigger refactor;
   file separately if pursued.

---

## Anti-patterns NOT present (worth recording)

- ❌ Server data stored in Zustand
- ❌ `useEffect` with empty dep array fetching data
- ❌ `// eslint-disable-next-line react-hooks/exhaustive-deps`
- ❌ Subscription leaks (intervals, listeners, WebSockets)
- ❌ Whole-store `useStore()` subscriptions (every consumer uses
  selectors)
- ❌ `useReducer` over-engineering simple toggles

---

## Verification

Numbers in this document are reproducible with `ripgrep` from
`frontend/`:

```bash
rg -l 'useQuery|useMutation|useQueryClient|useInfiniteQuery' src
rg -c 'useState\(' src/components | awk -F: '{s+=$2} END{print s}'
rg -l 'useReducer' src
rg -n 'eslint-disable.*exhaustive-deps' src
rg -l 'onMutate|setQueryData|previousData' src
rg -c 'staleTime|gcTime|cacheTime|refetchInterval' src
rg -n 'addEventListener|setInterval|setTimeout' src/hooks src/components/mail src/components/shared
```

Re-run quarterly. The optimistic-update count (`onMutate`) is the
single most informative metric; if it stays at 1, the recommendations
above haven't landed.
