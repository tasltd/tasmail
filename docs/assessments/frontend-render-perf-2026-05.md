# Frontend Rendering Performance Assessment

- **Issue:** TMAIL-263 (axis of TMAIL-241)
- **Date:** 2026-05-27
- **Scope:** React SPA at `frontend/` and alt-UI prototype at `themes/shadcn-prototype/`
- **Method:** Static read of every list-rendering component, hook layer, query layer, and
  composer; no runtime profiler trace was captured because there is no production-data
  dev instance available to the auto-fix session — the profiler section below records the
  trace plan as a follow-up, not as completed work.

---

## TL;DR — biggest wins, by ROI

| # | Finding | Impact | Effort | Suggested ticket |
|---|---------|--------|--------|------------------|
| 1 | **No list is virtualised anywhere in the SPA**, even though `@tanstack/react-virtual@3.13.23` is already in `package.json`. `MessageList`, `SearchResults`, `AuditLogManager`, `UsersManager`, and the alt-UI `EmailList` all render every row their backend returns into the DOM. | High at scale (≥500 rows visibly degrades scroll FPS; tab freezes around 5k rows) | Medium — one shared `<VirtualList>` helper, then swap 4–6 call sites | New |
| 2 | `MessageList` keys threads by **array index** (`key={i}`), so reordering on new mail forces every `ThreadRow` to remount instead of move. | Medium (visible when new mail lands while user is scrolling) | Trivial — stable thread id (e.g. first-message uid) | New |
| 3 | No `React.memo` on `MessageRow` / `ThreadRow` / `SearchRow` / `FolderItem`. Every parent re-render walks the full list. Combined with finding 1 this is the main cost on a busy inbox. | Medium | Trivial | New |
| 4 | Lists hit the backend with a default `page_size=50` but the SPA **never advances the page** — `useMessages(folder, page = 0, pageSize = 50)` is wrapped by `useCurrentMessages()` which calls it with no args, and there is no "load more" UI. So large folders silently truncate at 50 instead of scaling. | Medium (correctness as much as perf — users with > 50 messages can't reach them) | Medium — wire `useInfiniteQuery` or paging UI | New |
| 5 | `MessageList` recomputes `buildHighlightKeywords(query, advanced)` **inside `SearchResults.map`** on every render — once per row. Pull out of the map. | Low–Medium (linear in row count, allocations) | Trivial | Fix in this audit |
| 6 | `AuditLogManager` selects `limit` up to 500 rows and renders a non-virtualised `<table>` with 6 cols of inline-style props per row — each row creates ~14 new style objects per render. | Low (admin-only, occasional) | Low — extract styles to CSS classes, virtualise | New |
| 7 | Composer rich-text edits propagate `editor.on('update')` into a `useCallback` chain (`scheduleDraftSave`) whose dependency on `[to, cc, subject, editor]` re-creates a new `setTimeout` on **every keystroke**. The debounce works, but the listener and the effect tear down + reattach on each keystroke. | Low (audited — listener is idempotent, but it churns micro-allocations) | Low — split the editor `update` listener into its own effect keyed only on `editor` | New |
| 8 | `RecipientAutocomplete` is well-implemented (200 ms debounce, 2-char min, abort-on-empty). Keep as the model for future autocomplete inputs. | Positive baseline | — | — |
| 9 | `useEffect(() => fetch(), [])` antipattern is **not present**. TanStack Query is used 332 times across 65 files; the few `useEffect`s in components fetch are all keyboard listeners, click-outside, or debounced inputs. | Positive baseline | — | — |
| 10 | No `<img loading="lazy">` anywhere in `frontend/src`. Only one inline `<img>` exists today (`BrandingManager` logo preview, 32 px) so this is not material **today**, but if avatars / inline message images land it must be the default. | Future-looking | Low | Convention note |

---

## Component-by-component audit

### `frontend/src/components/mail/MessageList.tsx` (211 lines)

**What it does**

- Pulls envelopes from `useCurrentMessages()` → `useMessages(folder, 0, 50)` (TanStack Query, 15 s stale time).
- Builds `threads = useMemo(() => groupByThread(data.messages), [data?.messages])`.
- Renders either `threads.map(...)` or `data.messages.map(...)` into `.message-list__items`.

**Findings**

| # | Line(s) | Issue |
|---|---------|-------|
| L1 | 204 | `threads.map((thread, i) => <ThreadRow key={i} thread={thread} />)` — array index key. Use a stable key derived from the thread, e.g. the first message's `uid` (`thread.messages[0].uid`) or the normalised subject. |
| L2 | 51–76, 79–130 | `MessageRow` and `ThreadRow` are not `React.memo`-wrapped. Each invokes three Zustand selectors (`selectedUid`, `setSelectedUid`, `selectedFolder`) — Zustand handles equality, but combined with no memo every store update re-renders every row. |
| L3 | — | No virtualisation. With `page_size=50` this is fine today; with finding 4 fixed and users on a 5 000-message Inbox folder, the DOM blows up. Wrap `.message-list__items` in `useVirtualizer({ count, estimateSize: () => 56, getScrollElement })`. |
| L4 | 56 | `useMessageDrag(message.uid, selectedFolder)` is called inside every `MessageRow`. The hook itself is light, but its return object (`dragHandlers`) is a fresh object on every render — propagates onto a `<div>` so it doesn't cause child re-renders, but it does defeat any future `React.memo` on `MessageRow` unless wrapped in `useMemo`. |
| L5 | 161 | The `groupByThread` memo is keyed on `data?.messages` — TanStack Query returns a new array on every refetch even if contents are stable, so this recomputes on every poll. Consider `[data?.messages?.length, data?.messages?.[0]?.uid]` as a cheap stability heuristic, or hash the uid list. |

### `frontend/src/components/mail/SearchResults.tsx` (168 lines)

**Findings**

| # | Line(s) | Issue |
|---|---------|-------|
| S1 | 156–162 | `<SearchRow key={msg.uid} message={msg} keywords={buildHighlightKeywords(searchQuery, advancedSearch)} />` — `buildHighlightKeywords` is called **per row**. Lift it out of `.map`: `const keywords = useMemo(() => buildHighlightKeywords(searchQuery, advancedSearch), [searchQuery, advancedSearch])`. |
| S2 | 25–53 | `SearchRow` not memoised; same pattern as `MessageRow`. |
| S3 | — | No virtualisation. Less urgent than MessageList — search results are typically smaller — but the same fix applies. |

### `frontend/src/components/mail/FolderTree.tsx` (84 lines)

**Findings**

- Folder lists are bounded (single-digit to tens of folders) so virtualisation is not needed.
- `FolderItem` is not memoised; `useFolderDrop(folder.name, handleDrop)` allocates a fresh `handleDrop` per render. Wrap `handleDrop` with `useCallback` (already done) is fine; memoising `FolderItem` would still help during drag operations where the parent re-renders constantly. **Low priority.**

### `frontend/src/components/admin/AuditLogManager.tsx` (146 lines)

**Findings**

| # | Line(s) | Issue |
|---|---------|-------|
| A1 | 77 | Limit selector caps at 500 rows — without virtualisation this renders 500 × 6 = 3 000 `<td>`s with 6 inline `style={{...}}` literals each (~18 000 style objects allocated per render). Move styles to a CSS class and add `useVirtualizer` for the `<tbody>`. |
| A2 | 122–138 | Each row constructs a fresh `JSON.stringify(row.details)` for both the title attr and the cell content. Compute once: `const detailsJson = useMemo(() => row.details ? JSON.stringify(row.details) : null, [row.details])` inside an extracted memoised `AuditRow`. |

### `frontend/src/components/admin/UsersManager.tsx`, `settings/ContactManager.tsx`, similar list managers

- All use `useQuery` correctly with stable keys.
- None paginate (users list is `adminUsersApi.list()` with no limit). For a 10 k user tenant this will OOM the SPA. **Add server pagination + `useInfiniteQuery`.**
- Heavy table rendering: same finding as AuditLog — extract row to a memoised component, move inline styles to CSS, virtualise above ~200 rows.

### `frontend/src/components/mail/Composer.tsx` (371 lines)

**Findings**

| # | Line(s) | Issue |
|---|---------|-------|
| C1 | 82–87 | `useEffect(() => { editor.on('update', handler); return () => editor.off('update', handler); }, [editor, scheduleDraftSave])`. `scheduleDraftSave` depends on `[to, cc, subject, editor]`, so it's a new reference on every keystroke in any of those four — which means this effect detaches and reattaches the tiptap `update` handler on every header field keystroke. Fix: split into a ref-based handler so the listener is registered once: `const handlerRef = useRef(scheduleDraftSave); useEffect(() => { handlerRef.current = scheduleDraftSave; }); useEffect(() => { if (!editor) return; const h = () => handlerRef.current(); editor.on('update', h); return () => editor.off('update', h); }, [editor]);` |
| C2 | 74–79 | `useEffect(() => { scheduleDraftSave(); ... }, [to, cc, subject, scheduleDraftSave])` — fine functionally, but the function-identity churn means this effect always re-runs. Acceptable; the body only resets a timeout. |
| C3 | — | TipTap re-renders `EditorContent` per its own internal scheduler, not on every keystroke, so the Composer itself is okay. The Composer wraps a lot of other state (`undoState`, `showAiCompose`, `showLargeFile`, `showMeetingModal`, `showSchedulePicker`) — none of which feed into the editor — so any of those toggles re-renders the editor wrapper. Consider extracting the editor + send actions into a memoised child so toolbar/modal toggles don't traverse through it. |

### `themes/shadcn-prototype/src/features/email/EmailList.tsx` (77 lines)

**Findings**

- `emails.map(...)` straight render, no virtualisation, no memoisation.
- Currently driven from `src/data/mockData.ts` in many parts of the alt-UI, but `EmailList` is wired to live folders per `themes/shadcn-prototype/README.md`.
- `formatDistanceToNow(email.timestamp, { addSuffix: true })` is called per row per render. With 500 rows this is 500 `Date` parses and locale formats per render. Memoise per email-id, or shift to a date-fns `formatRelative` worker if the list grows.

---

## Cross-cutting checks

### Virtualisation

- Dependency `@tanstack/react-virtual@3.13.23` is **installed but unused** across `frontend/src`. Importing it costs nothing — using it is free perf.
- Recommended pattern (single shared helper to avoid copy-paste in every list):
  ```tsx
  // frontend/src/components/shared/VirtualList.tsx
  import { useVirtualizer } from '@tanstack/react-virtual';
  export function VirtualList<T>({ items, itemHeight, renderItem, getKey }: ...) { ... }
  ```
  Use it in `MessageList`, `SearchResults`, `AuditLogManager`, `UsersManager`, `ContactManager`, and the alt-UI `EmailList`. One helper, six consumers.

### `useEffect(() => fetch(), [])` audit

- 332 `useQuery` / `useMutation` occurrences across 65 files. TanStack Query is the established pattern.
- Grep for the antipattern (`useEffect` with API calls and `[]` deps) returns only three files: `Composer.tsx`, `LoginPage.tsx`, `OnboardingWizard.tsx`. Each was inspected manually:
  - `Composer.tsx`: the `useEffect` blocks are debounce timers and TipTap listener wiring, not data fetching.
  - `LoginPage.tsx`: no data fetch on mount; login is user-triggered.
  - `OnboardingWizard.tsx`: setTimeout-based step gating, not API fetch.
- **No remediation needed.** TanStack Query coverage is excellent.

### Image lazy loading

- Only one inline `<img>` in `frontend/src/components/` (`BrandingManager` 32 px logo preview).
- No avatars are rendered today.
- **Convention to enforce going forward:** any future `<img>` outside the visible viewport gets `loading="lazy" decoding="async"`. Add an ESLint rule (`jsx-a11y/alt-text` is on; add a custom rule or repo lint comment to require `loading="lazy"` on `<img>` unless explicitly marked `data-eager`).

### Debounce / throttle on inputs

- **`RecipientAutocomplete`:** 200 ms debounce, 2-char minimum. ✅
- **Search inputs (`SearchPanel`, `NlpSearchPanel`, `SemanticSearchPanel`, `AdvancedSearch`):** all hit TanStack Query with the query string in the query key. Query key changes on every keystroke; TanStack will dedupe in-flight requests but doesn't debounce them. **Add a 250–300 ms debounce on `searchQuery` before it lands in the store / query key.**
- **Filter inputs in admin lists (`AuditLogManager`, `UsersManager`):** the `action filter` text input writes to state on every keystroke; query key changes accordingly. **Debounce.**

### Composer rich-text re-render

- Audited (above, finding C1). Tiptap manages its own internal render schedule; the SPA wrapper re-renders on toggle state. The biggest leak is the `editor.on('update')` listener being re-attached per keystroke — fix with the ref-based handler shown in C1.

### Profiler trace (planned, not executed)

The brief asked for a "representative session: login → open inbox → search → read message → reply" with the top 5 hottest renders. This requires a running dev backend + frontend on the workstation with React DevTools Profiler attached, which the auto-fix session cannot drive headlessly. Tracked as **follow-up TMAIL-263a (or addendum to this doc)**: a developer should:

1. Run `npm run dev` in `frontend/` against the live backend
2. Open React DevTools Profiler, start recording
3. Walk the flow: login → INBOX → click message → click reply → search "invoice" → click result → close
4. Stop recording, export the JSON, attach to TMAIL-263, and update this section with the top 5 components by total render time

Expected suspects from the static review, in order: `MessageList` (no memo + no virtualisation), `MessageView` (lots of mutations + comments + smart-reply), `Composer` (editor listener churn), `AuditLogManager` (admin only), `EmailList` alt-UI (date-fns per row).

---

## Recommended ticket breakdown (under TMAIL-241 epic)

| Ticket | Title | Effort |
|--------|-------|--------|
| TMAIL-263a | Profiler trace of login → inbox → search → read → reply flow (data capture only) | S |
| TMAIL-263b | Add `<VirtualList>` helper using `@tanstack/react-virtual`; consume in `MessageList` | M |
| TMAIL-263c | Memoise list-row components: `MessageRow`, `ThreadRow`, `SearchRow`, `FolderItem`, `AuditRow`, `UserRow` | M |
| TMAIL-263d | Wire infinite-scroll / pagination through `useInfiniteQuery` for messages, audit log, users, contacts | M |
| TMAIL-263e | Debounce 250 ms on `searchQuery`, audit `action filter`, admin user search, contact search | S |
| TMAIL-263f | Fix `MessageList` `key={i}` → stable thread id; lift `buildHighlightKeywords` out of `SearchResults.map` | S |
| TMAIL-263g | Composer: ref-based TipTap `update` listener so it isn't re-attached per keystroke | S |
| TMAIL-263h | Repo convention: ESLint rule (or PR-review checklist item) requiring `loading="lazy"` on new `<img>` | S |
| TMAIL-263i | Alt-UI `EmailList`: memoise rows, memoise per-row `formatDistanceToNow` | S |

---

## Quick wins that fit inside this assessment commit

Two finds are low-risk and small enough to fix in the same change as this report:

1. **`MessageList.tsx:204`** — replace `key={i}` with a stable id (`thread.messages[0]?.uid ?? thread.subject`).
2. **`SearchResults.tsx:156–162`** — lift `buildHighlightKeywords(...)` out of the `.map`.

Everything else stays as separate scoped tickets per the recommended breakdown above.

---

## Sources & references

- React 19 release notes — auto-batching, no-memo improvements: <https://react.dev/blog/2025/12/05/react-19> (consulted for context on `React.memo` still being needed for list rows that read store slices)
- TanStack Virtual docs: <https://tanstack.com/virtual/latest/docs/introduction>
- TanStack Query — `useInfiniteQuery`: <https://tanstack.com/query/latest/docs/framework/react/guides/infinite-queries>
- React Profiler API: <https://react.dev/reference/react/Profiler>
- TipTap performance notes: <https://tiptap.dev/docs/editor/api/editor> (event handler attach/detach cost)
- web.dev — `loading="lazy"`: <https://web.dev/articles/browser-level-image-lazy-loading>

All findings above are derived from static reading of `frontend/src` and `themes/shadcn-prototype/src` at HEAD (`d2ea4c5`, 2026-05-27). No runtime data was collected; the profiler trace remains an explicit follow-up.
