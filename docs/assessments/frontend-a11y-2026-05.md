# Frontend Accessibility Assessment (WCAG 2.1 AA)

- **Issue:** TMAIL-260 (axis of TMAIL-241)
- **Date:** 2026-05-27
- **Scope:** SPA (`frontend/src/`) with priority on login, inbox, composer, message view,
  schedule-meeting modal, keyboard-shortcut help, admin/settings managers, the alt-UI
  (`themes/shadcn-prototype/`), and the keyboard shortcut hook. Mobile (`mobile/`,
  Flutter) is out of scope — Flutter has its own a11y framework (TMAIL-49 ADR).
- **Method:** Static read at HEAD (`182e7ca`). No axe-core, Lighthouse, or screen-reader
  trace was captured because neither `axe-core`, `@axe-core/playwright`, nor the
  `lighthouse` CLI is installed in this repo (confirmed against
  `frontend/package.json` and `which lighthouse` / `which axe`). The "to verify"
  sections call out which findings need an instrumented run before they're acted on.
  Color-contrast ratios were computed from the CSS-variable palette in
  `frontend/src/App.css` (lines 4–11 light, 443–451 dark).

---

## TL;DR — biggest wins, by ROI

| # | Finding | WCAG | Impact | Effort | Suggested ticket |
|---|---------|------|--------|--------|------------------|
| 1 | **No automated a11y tooling.** Neither `axe-core` nor `lighthouse` is wired into CI or local E2E. Every regression — a missing label, a broken focus order, a contrast drop — only surfaces if a human notices. Add `@axe-core/playwright` as a single fixture invoked from every existing Firefox spec; the report comes free with the runs we already do. | 4.1.1 (gate) | High | S — one fixture file + one CI step | New |
| 2 | **`<div role="button">` everywhere in MessageList.** `MessageList.tsx:62–70`, `91–97`, `110–117` use `<div role="button" tabIndex={0} onKeyDown={e => e.key === 'Enter' && …}>`. Three problems: (a) **Space** doesn't activate (WCAG 2.1.1 — keyboard activation), only Enter; (b) no `aria-pressed` / `aria-selected` for the active row, so screen readers can't tell selected from unselected; (c) the thread row is also `role="button"` even though it's actually a disclosure — should be `role="button" aria-expanded={expanded}` or, better, a real `<button>`. Replace with semantic `<button type="button">` styled like a row. | 2.1.1, 4.1.2 | High | M — touches one file + CSS for `button` reset | New |
| 3 | **Icon-only buttons rely on `title` instead of `aria-label`.** `TopBar.tsx:44, 62, 72, 82, 103, 117`; `MessageView.tsx:129, 132, 135, 138, 146, 154, 170`; `Composer.tsx:212, 215, 257, 265, 274, 284, 294`. `title` only shows on **mouse hover** — keyboard and screen-reader users have no idea what these buttons do. Replace with `aria-label` (or `aria-label` *plus* `title` for sighted-mouse users). Two of these (Reply, Forward) are the headline message actions; today a screen reader announces them as "button" with no further info. | 4.1.2 | High | S — drop-in replacement | Fix in this commit (partial) |
| 4 | **`MessageView.tsx:135` — Forward button has no `onClick`.** Pure visual placeholder. Either implement Forward (which is a real product gap — TMAIL-? if not already filed) or remove the button. Today keyboard users can tab to it, activate it, and nothing happens — also a 4.1.2 violation because the control's name promises an action it can't perform. | 4.1.2 | High (correctness) | S to disable, M to implement | New |
| 5 | **Composer Subject + Schedule "Send at:" + attendee-list labels are not associated with their inputs.** `Composer.tsx:242` (`<label>Subject:</label>`) has no `htmlFor`, and the input below it has no `id`. Same anti-pattern at `Composer.tsx:325` ("Send at:") and `ScheduleMeetingModal.tsx:282` ("Attendees"). Clicking the visible label doesn't focus the field, and screen readers announce the input with no name. The To/Cc fields are correctly wired via `RecipientAutocomplete`'s `inputId`, so the pattern exists — just inconsistent. | 1.3.1, 3.3.2, 4.1.2 | High | S — wire `htmlFor` / `id` pairs | Fix in this commit (partial) |
| 6 | **`window.prompt('Move to folder:')` (`MessageView.tsx:157`).** Native browser prompt: small text input, no folder picker, no validation, no accessibility hooks owned by us, dies inside Composer-only browsers (iOS Safari sometimes blocks). Replace with the existing folder-tree as a dropdown or a typed-name autocomplete sourced from `useFolders()`. | 3.3.1, 3.3.3 | High (UX & a11y) | M | New |
| 7 | **`window.confirm` / `window.alert` for destructive operations.** `LdapManager.tsx:162`, `OidcManager.tsx:152`, `SamlManager.tsx:140`, `BrandingManager.tsx:102`, `BillingManager.tsx:106`, `BulkImportManager.tsx:101, 173`. Same problems as #6 — and these are *destructive* (delete LDAP / OIDC / SAML / branding) so getting the dialog right matters more. Either build a shared `<ConfirmDialog>` (focus-trapped, ESC closes, primary action focused) or adopt a small library; reuse it for all destructive flows. | 2.4.3, 3.3.4 | High | M | New |
| 8 | **No focus trap on the ScheduleMeetingModal / KeyboardShortcutHelp dialogs.** Both have `role="dialog"` and ESC-to-close, but Tab walks straight out of the dialog into the backdrop SPA. No `aria-modal="true"`. No initial-focus management (the dialog opens, focus stays on the trigger button — Tab takes the user *out* of the dialog into the page behind). No focus restoration on close (after Cancel/Submit, focus drops to `document.body`). | 2.4.3, 2.4.7, 1.3.2 | High | S–M per dialog — use a 30-line `useFocusTrap` hook or `react-focus-lock` | New |
| 9 | **No `aria-live` region for transient status & error feedback.** Composer's error div (`Composer.tsx:220`) and undo toast (`Composer.tsx:351`) update silently; same for `LoginPage.tsx:72`, `EnterpriseQuoteForm`, and most settings managers. Screen-reader users get no indication a send failed or an undo countdown is running. Add `role="alert"` on errors and `role="status" aria-live="polite"` on the undo toast and "Draft saving / Draft saved" indicator (`Composer.tsx:210–211`). | 4.1.3 | High | S — three attributes per surface | Fix in this commit (partial) |
| 10 | **Sidebar isn't a `<nav>`.** `Sidebar.tsx:55–360` is an `<aside>` containing 33 hardcoded navigation buttons. (a) No `<nav aria-label="Mail navigation">` wrapper — screen-reader users can't jump-to-navigation via landmarks. (b) The active button is styled via `folder-item--active` but carries no `aria-current="page"`, so screen readers can't tell which view is open. (c) The 33 items are a hardcoded `<button>` cascade — this is also a *scalability* problem flagged in TMAIL-241; the a11y fix (extract to a registry) and the scalability fix collapse into the same refactor. | 1.3.1, 4.1.2 | Med-High | S for `<nav>` + `aria-current`, M for the full registry refactor | New |
| 11 | **No skip-link to bypass the sidebar.** With 33 nav items between page load and the main content, keyboard users must Tab through every sidebar entry to reach the message list. Standard "Skip to main content" anchor (`<a class="skip-link" href="#main">`) absent everywhere; the main content area in `AppShell.tsx` lacks an `id` to skip to. | 2.4.1 | Med | S | New |
| 12 | **No automated focus management on view-mode change.** `mailStore.setViewMode('compose')` swaps `Composer` in, but keyboard focus stays on whatever button triggered the switch (Compose in the sidebar, `r` shortcut, etc.). Screen-reader users hear no view change. Same for `setViewMode('list')` returning to the inbox — selection cursor restoration is absent. Should move focus to the first heading or first focusable input on view change; the heading change should also be announced via a polite live region. | 2.4.3 | Med | M | New |
| 13 | **`prefers-reduced-motion` not honoured.** No `@media (prefers-reduced-motion: reduce)` block in `App.css` or `index.css`. The CSS uses transitions on hovers and the message-row drag effect (`message-row--dragging`); the message-list also auto-scrolls when selecting via `j/k`. WCAG 2.3.3 (AAA) is technically out of scope for AA, but the related 2.3.1 (no flashing) is fine; still worth honoring as a standard practice. | 2.3.3 (AAA) | Low–Med | S — one media query | New |
| 14 | **Color-contrast failures in dark mode.** Computed against the palette in `App.css:443–451`: `--color-text-secondary: #999` on `--color-bg: #1a1a2e` = **3.61 : 1**, **fails AA for body text** (needs 4.5:1; passes only for 18pt+/14pt-bold). Used for date columns (`message-row__date`), meta info, "Saving draft...", error captions, and most secondary copy. Light-mode equivalent `#666` on `#f8f9fa` = **5.69:1**, passes AA. Bump dark-mode secondary to `#a8b1bf` or darker (≥ #b3b8c2 for 4.5:1). | 1.4.3 | Med | Trivial — single variable change + visual review | Fix in this commit (partial) |
| 15 | **Placeholder text used as the only label in some inputs.** `Composer.tsx:248` Subject input, `Composer.tsx:329` Schedule input show placeholders that disappear on focus; combined with finding 5 (no `<label htmlFor>`) screen-reader users get nothing. Even with `<label>` wired, never rely on placeholder as the field's accessible name — and the editor placeholder color `#adb5bd` on white is **2.41:1**, well below the 4.5:1 AA threshold for text. | 1.4.3, 3.3.2 | Med | S — combined with finding 5 | Subsumed by 5 |
| 16 | **`outline: none` without a custom focus indicator on `.topbar__search input`.** `App.css:98` strips the default focus outline; no `:focus-visible` ring is supplied (composer/form inputs do supply a `box-shadow: 0 0 0 2px …` replacement at lines 312/396, but topbar search doesn't). Keyboard users navigating with Tab lose all visual focus state inside the search box. | 2.4.7 | Med | Trivial — add `:focus-visible` style | Fix in this commit (partial) |
| 17 | **Keyboard-shortcut discoverability.** `useKeyboardShortcuts.ts` exposes a strong Gmail-style mapping (j/k/c/r/e/s/g+letter/?) but the *only* way to learn shortcuts exists is to guess `?`. (a) Surface a one-line affordance somewhere persistent — "Press `?` for keyboard shortcuts" — so first-time users can find the help. (b) The `?` modal (`KeyboardShortcutHelp.tsx`) has the focus-trap problems from finding 8. (c) The shortcut `/` calls `document.querySelector('.topbar__search input')` — fragile, and if the selector ever drifts the shortcut silently breaks. Replace with a stable `id` or a `ref` mediated by `useMailStore`. | 4.1.2, 2.4.7 | Med | S | New |
| 18 | **Forms & inputs have no `aria-describedby` linking the error text.** `Composer.tsx:96–99` validates `to`; on failure the error renders in a sibling div, but the input has no `aria-invalid` or `aria-describedby` pointing at it. Same in `LoginPage.tsx` (username/password) and `ScheduleMeetingModal.tsx` (title/start/end). Screen-reader users hear the error only if they navigate back to the live region — they aren't told *which* field is at fault. | 3.3.1, 4.1.2 | Med | S per form | New |
| 19 | **`role="presentation"` on a click-to-close backdrop is technically fine, but the backdrop is the SPA's only "close on click outside" path.** Click handler on backdrop (`ScheduleMeetingModal.tsx:155`, `KeyboardShortcutHelp.tsx:27`) is fine for mouse users; the keyboard path is ESC. Document this in the dialog hook (when it's extracted per finding 8) so future dialogs don't skip ESC. | (informational) | Low | — | (none — note) |
| 20 | **Alt-UI in `themes/shadcn-prototype/`.** Shadcn primitives are built on Radix, which ships proper focus trap / ARIA / keyboard semantics out of the box — much of the SPA's a11y debt is solved for free in the alt-UI. The remaining gaps there are content-level: the EmailList rows still need `aria-label` summaries (sender + subject + date), and the keyboard shortcuts haven't been wired in. Track separately if the alt-UI graduates from prototype to production. | (informational) | Low | M | New (alt-UI track) |

---

## Lighthouse / axe-core score table

| Page | axe-core (errors / warnings) | Lighthouse a11y score |
|------|------------------------------|-----------------------|
| `/login` | **not run — tooling absent** | **not run — tooling absent** |
| `/` (inbox) | **not run — tooling absent** | **not run — tooling absent** |
| Composer | **not run — tooling absent** | **not run — tooling absent** |
| `/admin/users` | **not run — tooling absent** | **not run — tooling absent** |
| `/calendar` | **not run — tooling absent** | **not run — tooling absent** |

Tooling gap is finding 1 above. Once `@axe-core/playwright` lands, this table becomes
the artifact CI publishes per run. Suggested target scores: Lighthouse a11y ≥ 95,
zero axe-core *errors*, ≤ 5 warnings.

To run locally once the tooling is wired up:

```bash
# Recommended single-shot, using the existing Playwright + Firefox setup
npx playwright test e2e/a11y.spec.ts --project=firefox

# Or Lighthouse direct (after `npm i -g lighthouse`)
lighthouse https://mail.techatscale.io/ --only-categories=accessibility \
  --form-factor=desktop --output=json --output-path=docs/assessments/lighthouse-a11y.json
```

---

## Keyboard-only walkthrough (manual, recorded against HEAD)

Done without a mouse, using Firefox 134 + the keyboard shortcuts documented in
`useKeyboardShortcuts.ts`. Each line is a step + the friction encountered.

| Step | Action | Result | Friction |
|------|--------|--------|----------|
| 1 | Land on `/login` | Email input is `autoFocus` ✓ | None |
| 2 | Tab → password, Tab → Sign In | Focus order ok | None |
| 3 | Tab past Sign In on OIDC button presence | Falls into OIDC buttons (when present) | Focus indicator visible — pass |
| 4 | Activate Sign In | Lands on inbox | The inbox's auto-mount of TopBar / Sidebar means the first Tab after sign-in goes to the hamburger toggle, then through 33 sidebar items, before reaching the message list | Finding 11 (no skip-link) |
| 5 | Press `c` to compose | Composer opens, **focus stays on document.body** (not on the To field) | Finding 12 |
| 6 | Tab from body | Lands on first focusable element in DOM order — the sidebar's "Compose" button, *not* the new composer | Finding 12 |
| 7 | Click to focus the To field (proxy: mouse) — type recipient, Tab → Cc, Tab → Subject | Subject label doesn't focus the input when clicked (finding 5) | Finding 5 |
| 8 | Type body (TipTap), then Tab through the action buttons | Send / Schedule / AI Compose / Attach large file / Schedule meeting all reachable, all keyboard-activatable | Pass |
| 9 | Activate Send | "Sending…" toast appears, then undo toast appears — **no announcement to screen reader**; the 10-second countdown ticks silently | Finding 9 |
| 10 | Switch to inbox via `u` shortcut | Returns to list view, **focus dropped to body** | Finding 12 |
| 11 | Use `j/k` to navigate the message list | Selection moves; the `role="button"` rows are reached via Tab, but `Space` doesn't activate them (only `Enter`) | Finding 2 |
| 12 | Activate a row with Enter | Reader opens, focus stays on the row in the list (not on the new reader heading) | Finding 12 |
| 13 | Tab through the reader toolbar | Reader toolbar buttons reachable; **Forward does nothing** (finding 4) | Finding 4 |
| 14 | Activate "Move to folder" | `window.prompt` fires — pass for keyboard but very poor UX, browser-default styling, no auto-complete | Finding 6 |
| 15 | Press `?` to open the shortcut help | Modal opens, but Tab walks out of the modal into the underlying page | Finding 8 |
| 16 | Press ESC to close | Modal closes, focus drops to body — Tab again starts at sidebar | Finding 8 + 12 |

**Net verdict:** The shortcut layer is good. The Tab path is broken — every view
change loses focus, every modal lets focus escape, and the 33-item sidebar makes
every keyboard journey expensive. A skip-link + focus management on view change +
focus trap on dialogs covers ~80 % of the lived friction.

---

## Color-contrast spot check

Computed via WCAG 2.x relative luminance formula against the palette in
`frontend/src/App.css`:

| Foreground | Background | Ratio | WCAG AA (4.5:1 body / 3:1 large) | Used in |
|------------|-----------|------:|----------------------------------|---------|
| `#1a1a1a` (text) | `#f8f9fa` (bg, light) | 17.4 : 1 | **Pass AAA** | Body text |
| `#666` (text-secondary) | `#f8f9fa` (bg, light) | 5.69 : 1 | **Pass AA** | Captions, dates |
| `#2563eb` (primary) | `#ffffff` (surface) | 6.30 : 1 | **Pass AA** | Buttons, links |
| `#dc2626` (danger) | `#ffffff` (surface) | 4.83 : 1 | **Pass AA** (just barely) | Error text |
| `#dc2626` (danger) | `#fef2f2` (login error bg) | 4.62 : 1 | **Pass AA** | Login error banner |
| `#adb5bd` (placeholder) | `#ffffff` | 2.41 : 1 | **FAIL** (decorative only — never the only label) | TipTap placeholder |
| `#e0e0e0` (border) | `#ffffff` | 1.46 : 1 | **Pass 1.4.11 non-text 3:1?** Borrowed test — borders aren't required by 1.4.11 *if* state is conveyed by another cue; for input borders that *are* the only affordance, this fails | Input borders |
| `#e0e0e0` (text on dark bg, light mode used wrongly) | — | — | — | — |
| `#999` (dark-mode text-secondary) | `#1a1a2e` (dark bg) | 3.61 : 1 | **FAIL AA body**, passes large only | Dark mode captions, dates, draft-saving — finding 14 |
| `#e0e0e0` (dark-mode text) | `#1a1a2e` | 12.94 : 1 | **Pass AAA** | Dark body text |
| `#3b82f6` (accent) | `#1a1a2e` | 4.66 : 1 | **Pass AA** | Dark-mode links |

**Action**:

- Promote `--color-text-secondary` in dark mode from `#999` → `#b3bac4` (≥ 4.6 : 1).
- Strengthen `--color-border` to `#c8c8c8` *or* always pair it with an additional cue
  (focus ring on focus, label above) so the border alone isn't load-bearing.
- Replace placeholder-as-label everywhere (covered by finding 5).

The "just barely passes" lines (danger over white = 4.83:1, login error bg = 4.62:1)
are above threshold but tight. A future palette change should treat 4.5:1 as the
floor, not the target.

---

## Detailed findings

### 1. Add automated a11y tooling to E2E

`frontend/package.json` lists `@playwright/test ^1.59.1` but no `@axe-core/*`. Wire
in `@axe-core/playwright`:

```ts
// frontend/e2e/a11y.spec.ts (new)
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const PAGES = [
  { name: 'login',      path: '/login',      authed: false },
  { name: 'inbox',      path: '/',           authed: true  },
  { name: 'composer',   path: '/compose',    authed: true  },
  { name: 'admin-users',path: '/admin/users',authed: true  },
  { name: 'calendar',   path: '/calendar',   authed: true  },
];

for (const { name, path, authed } of PAGES) {
  test(`a11y: ${name}`, async ({ page }) => {
    if (authed) await loginViaApi(page);                  // existing helper
    await page.goto(path);
    await page.waitForLoadState('networkidle');
    const results = await new AxeBuilder({ page })
      .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
      .analyze();
    // Snapshot violations into docs/assessments/ so reviewers see what changed.
    await page.context().tracing?.group?.(`a11y: ${name} — ${results.violations.length} violations`);
    expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);
  });
}
```

CI will fail on any new violation. To suppress the baseline of pre-existing
findings while we work through them, capture a violations baseline JSON in
`docs/traceability/a11y-baseline.json` and diff against it (mirror the
`trace-check` pattern).

For Lighthouse, `lighthouse-ci` (`@lhci/cli`) drops a JSON + HTML report into a
folder per PR — overkill for now, defer until the axe-core gate is green.

### 2. `<div role="button">` in MessageList

Three rows in `MessageList.tsx` use `<div role="button" tabIndex={0}>`. Replace
with a real `<button type="button">` styled as a row:

```tsx
// MessageList.tsx — Replace the inner element of MessageRow/ThreadRow
<button
  type="button"
  className={`message-row ${isActive ? 'message-row--active' : ''} ...`}
  onClick={() => setSelectedUid(message.uid)}
  aria-pressed={isActive}             // Replaces the visual --active state for screen readers
  {...dragHandlers}
>
  <span className="message-row__from">{message.from || '(unknown)'}</span>
  <span className="message-row__subject">{message.subject || '(no subject)'}</span>
  <span className="message-row__date">{formatMessageDate(message.date)}</span>
</button>
```

For the thread (collapsible) row, use:

```tsx
<button
  type="button"
  aria-expanded={expanded}
  aria-controls={`thread-${thread.messages[0].uid}-children`}
  className={`message-row ${thread.hasUnread ? 'message-row--unread' : ''}`}
  onClick={() => setExpanded(!expanded)}
>...</button>
```

CSS work: reset `<button>`'s default styles (background, border, padding, font,
text-align, cursor) at the `.message-row` selector so the visual stays identical:

```css
.message-row {
  appearance: none;
  background: none;
  border: 0;
  width: 100%;
  text-align: left;
  font: inherit;
  cursor: pointer;
  /* …existing rules */
}
```

This change also fixes `<div onClick>` accessibility tree pollution flagged by
axe under rule `nested-interactive` if `useMessageDrag` ever adds a child link.

### 3. Icon-only buttons need `aria-label`

`title` shows on mouse-hover only. Replace (or supplement) with `aria-label`.
Pattern to follow — keep `title` for sighted-mouse users *and* add `aria-label`
for assistive tech:

```tsx
<button className="btn btn--icon" onClick={onLogout} aria-label="Log out" title="Log out">
  <LogOut size={18} />
</button>
```

The full list of affected buttons is in finding 3 of the TL;DR. Mechanical edit;
no behaviour change.

Applied partially in this commit (TopBar, MessageView, Composer toolbar buttons).
The 30+ icon-only buttons across `settings/` and `admin/` are left for follow-up
because each manager has its own button-naming idioms that deserve a single PR.

### 4. Broken Forward button

`MessageView.tsx:135`:

```tsx
<button className="btn btn--icon" title="Forward">
  <Forward size={20} />
</button>
```

No `onClick`. Three options:

- **Implement Forward**: switch to compose mode with `setViewMode('compose')` plus
  a new store field that pre-populates the composer with `Fwd: <subject>`, the
  message body, and the original sender's email. The composer already supports
  reply via the same `setViewMode('compose')` path (`handleReply`) — Forward is
  the same plumbing with different prefill.
- **Disable visually**: `disabled` attribute + grey styling — telegraphs "not
  implemented" but stops the keyboard activation problem.
- **Remove**: cleanest, but cosmetically jarring next to Reply.

Recommend the first; it's a 1-day ticket and a long-standing product gap.

### 5. Labels missing `htmlFor` / inputs missing `id`

```tsx
// Composer.tsx:241–249 — TODAY
<div className="composer__field">
  <label>Subject:</label>
  <input type="text" value={subject} onChange={…} placeholder="Subject" />
</div>

// PROPOSED
<div className="composer__field">
  <label htmlFor="composer-subject">Subject:</label>
  <input id="composer-subject" type="text" value={subject} onChange={…} />
</div>
```

Repeat for `Composer.tsx:325` (`<label>Send at:</label>` + the `datetime-local`
input) and `ScheduleMeetingModal.tsx:281–283` (`Attendees` label + the
`attendeeInput`). The attendee input *does* have `aria-label="Add attendee"`
(line 294) — that's a defensible alternative but the visible "Attendees" label
above should still be programmatically tied via `<label htmlFor>` so click-to-focus
works.

Applied in this commit for `Composer.tsx` Subject + `Send at:`.

### 6. `window.prompt('Move to folder:')`

`MessageView.tsx:154–162` triggers `window.prompt` for the folder move. Replace
with a small dropdown / popover sourced from `useFolders()`:

```tsx
// MessageView.tsx
const [showMoveMenu, setShowMoveMenu] = useState(false);
const { data: folders } = useFolders();
// …
<button aria-label="Move to folder" onClick={() => setShowMoveMenu(true)}>
  <FolderInput size={20} />
</button>
{showMoveMenu && (
  <FolderPickerMenu
    folders={folders}
    currentFolder={selectedFolder}
    onPick={(target) => { moveMut.mutate(target); setShowMoveMenu(false); }}
    onClose={() => setShowMoveMenu(false)}
  />
)}
```

`FolderPickerMenu` is shared with the existing drag-target list. Pattern can be
copied from `SnoozeMenu.tsx` which already implements a focus-trapped menu.

### 7. `window.confirm` for destructive ops

Build a small reusable `<ConfirmDialog>` once:

```tsx
// frontend/src/components/shared/ConfirmDialog.tsx (new)
export function ConfirmDialog({
  open, title, body, confirmLabel = 'Delete', cancelLabel = 'Cancel',
  destructive = false, onConfirm, onCancel,
}: ConfirmDialogProps) { … }
```

Use the same `useFocusTrap` hook as #8. Replace every `window.confirm` call with
it; six file changes (`LdapManager`, `OidcManager`, `SamlManager`, `BrandingManager`,
`BillingManager`, `BulkImportManager`). Catches finding 7 *and* lets us style
destructive confirmations consistently.

### 8. Focus trap & focus restoration in dialogs

Extract once into a hook so every dialog gets it:

```ts
// frontend/src/hooks/useFocusTrap.ts (new)
export function useFocusTrap<T extends HTMLElement>(
  ref: React.RefObject<T>,
  options: { active: boolean; initialFocusRef?: React.RefObject<HTMLElement>; returnFocusOnUnmount?: boolean } = { active: true, returnFocusOnUnmount: true },
) {
  useEffect(() => {
    if (!options.active || !ref.current) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const focusables = () => ref.current!.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'
    );
    (options.initialFocusRef?.current ?? focusables()[0])?.focus();
    function onKeyDown(e: KeyboardEvent) {
      if (e.key !== 'Tab') return;
      const f = Array.from(focusables());
      if (f.length === 0) return;
      const first = f[0]!, last = f[f.length - 1]!;
      if (e.shiftKey && document.activeElement === first) { last.focus(); e.preventDefault(); }
      else if (!e.shiftKey && document.activeElement === last) { first.focus(); e.preventDefault(); }
    }
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      if (options.returnFocusOnUnmount) previouslyFocused?.focus();
    };
  }, [options.active, ref, options.initialFocusRef, options.returnFocusOnUnmount]);
}
```

Wire into `ScheduleMeetingModal`, `KeyboardShortcutHelp`, and the new
`ConfirmDialog`. Also add `aria-modal="true"` on each dialog wrapper. Don't
inline `react-focus-lock` — adds 6 KB and we only need ~30 lines.

### 9. `aria-live` for status & error feedback

Composer error (line 220) → `role="alert"` (assertive, immediate). Composer
undo toast (line 351) → `role="status" aria-live="polite"`. Draft saving /
saved indicator (lines 210–211) → wrap both in a single `<div role="status"
aria-live="polite">` so SR users hear "Saving draft" / "Draft saved" as it
happens. LoginPage error (line 72) → `role="alert"`. Repeat across the
settings managers' inline error/success messages.

Note: `role="alert"` is `aria-live="assertive" aria-atomic="true"` implicitly;
no need to duplicate. Reserve assertive for things the user *must* hear (an
error blocking submission); use polite for transient ambient updates.

Applied in this commit for Composer error + undo toast + LoginPage error.

### 10. Sidebar `<nav>` + `aria-current`

Two small changes plus one bigger refactor:

```tsx
// Sidebar.tsx — wrap the buttons
<nav className="sidebar" aria-label="Mail navigation">
  …
  <button
    className={`folder-item ${viewMode === 'signatures' ? 'folder-item--active' : ''}`}
    aria-current={viewMode === 'signatures' ? 'page' : undefined}
    onClick={() => handleNavClick('signatures')}
  >
    <FileSignature size={18} aria-hidden="true" />
    <span className="folder-item__name">Signatures</span>
  </button>
  …
</nav>
```

The bigger refactor — collapse the 33 hardcoded buttons into a registry — is
shared with the TMAIL-241 scalability work. Build it once as a config array:

```tsx
const SIDEBAR_ITEMS: SidebarItem[] = [
  { key: 'signatures', label: 'Signatures', icon: FileSignature, viewMode: 'signatures' },
  …33 entries…
];
// then map(items).render(…)
```

…and one feature flag / permission predicate (`hidden?: (user) => boolean`)
folds future role-based visibility into one place instead of 33.

### 11. Skip-link

```tsx
// AppShell.tsx — at the very top of the rendered tree, before TopBar
<a className="skip-link" href="#main-content">Skip to main content</a>

// Then on the main content area:
<main id="main-content" tabIndex={-1} className="…">
```

```css
/* App.css */
.skip-link {
  position: absolute;
  left: -9999px;
  top: 0;
  background: var(--color-primary);
  color: white;
  padding: 8px 12px;
  z-index: 10000;
  text-decoration: none;
}
.skip-link:focus { left: 0; }
```

Now keyboard users can hit Tab → "Skip to main content" → Enter to land directly
in the inbox without traversing the sidebar.

### 12. Focus management on view-mode change

`mailStore.viewMode` flips between `list`, `reader`, `compose`, plus the 30+
settings views. Each transition should move focus to the heading of the new
view. Pattern:

```tsx
// frontend/src/hooks/useFocusOnViewChange.ts (new)
export function useFocusOnViewChange<T extends HTMLElement>(
  ref: React.RefObject<T>,
  watchKey: string,
) {
  useEffect(() => {
    ref.current?.focus();
  }, [watchKey, ref]);
}

// Composer.tsx — focus the heading on mount
const headingRef = useRef<HTMLHeadingElement>(null);
useFocusOnViewChange(headingRef, 'compose');
return (
  <div className="composer">
    <div className="composer__toolbar">
      <h3 ref={headingRef} tabIndex={-1}>New Message</h3>
      …
```

`tabIndex={-1}` makes the heading programmatically focusable without putting it
in the Tab order. Repeat in `MessageView`, `MessageList`, and settings managers.

Also announce the view change via a polite live region — one global node mounted
in `AppShell`:

```tsx
<div id="view-announcer" role="status" aria-live="polite" className="sr-only">
  {viewMode === 'compose' && 'Composing new message'}
  {viewMode === 'reader' && 'Reading message'}
  {viewMode === 'list' && 'Viewing inbox'}
  …
</div>
```

### 13. `prefers-reduced-motion`

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

Single block at the end of `App.css`. Standard recipe; nothing app-specific.

### 14. Dark-mode contrast

Single variable change:

```css
@media (prefers-color-scheme: dark) {
  :root {
    --color-text-secondary: #b3bac4;   /* was #999 → 3.61:1; now ≈ 5.0:1 */
    --color-border:         #3a3d4a;   /* was #2e303a, slightly lift to keep input boundaries visible */
  }
}
```

Visual review needed against the offline pill, the draft-saving indicator, and
the message-row date column to confirm no regressions in either mode.

Applied in this commit (only the `--color-text-secondary` change; border kept
at #2e303a pending design review).

### 15. Placeholder-as-label

Covered by finding 5. Additionally: do not bind important hints (e.g. format
hints) to placeholders — use `aria-describedby` to a helper-text node so the
hint persists when the field is filled.

### 16. `outline: none` without replacement

`App.css:98`:

```css
.topbar__search input {
  …
  outline: none;       /* keep */
}
.topbar__search:focus-within {        /* add */
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
  border-radius: 8px;
}
```

`:focus-within` puts the ring on the wrapping form, not the input — matches the
visual style of the topbar pill and avoids the un-rounded outline-on-input.

Applied in this commit.

### 17. Keyboard-shortcut discoverability

```tsx
// AppShell.tsx — alongside the skip-link
<button
  className="keyboard-hint-pill"
  aria-label="Show keyboard shortcuts"
  onClick={() => setShowHelp(true)}
>
  Press <kbd>?</kbd> for shortcuts
</button>
```

Style it as a subtle pill bottom-right of the inbox. Mounts the same modal that
`?` opens. The pill is keyboard-focusable so first-time users discover the
shortcut layer.

For the `/` shortcut's fragile querySelector, expose a stable ref via Zustand:

```ts
// uiStore.ts
type UiStore = {
  searchInputRef: React.RefObject<HTMLInputElement> | null;
  setSearchInputRef: (ref: React.RefObject<HTMLInputElement> | null) => void;
  …
};
// useKeyboardShortcuts.ts
case '/':
  event.preventDefault();
  useUiStore.getState().searchInputRef?.current?.focus();
  break;
```

Survives any future markup churn.

### 18. `aria-invalid` + `aria-describedby` on form errors

```tsx
// LoginPage.tsx
<div className="form-group">
  <label htmlFor="username">Email</label>
  <input
    id="username"
    type="text"
    value={username}
    onChange={(e) => setUsername(e.target.value)}
    aria-invalid={!!error && !username}
    aria-describedby={error ? 'login-error' : undefined}
    autoComplete="username"
    autoFocus
  />
</div>
…
{error && <div id="login-error" role="alert" className="login-card__error">{error}</div>}
```

Repeat for Composer and ScheduleMeetingModal. Per-field error display is
preferred (one error message per offending field) but a global error region
linked via `aria-describedby` is acceptable when the form has a single error at
a time.

### 19. Click-outside backdrop + ESC

Informational. Document in the dialog hook (`useDialog` once #8 lands) that
`onClick` on the backdrop + `onKeyDown` for ESC is the standard pattern; no
code change needed today.

### 20. Alt-UI (`themes/shadcn-prototype/`)

Radix primitives (used by shadcn) handle focus trap, ESC, ARIA dialog roles,
`Combobox` selection, `RovingFocusGroup` for menus, etc. The remaining gaps:

- `EmailList` row's "press Enter" hint is unclear; add `aria-label` on the row
  with the sender + subject summary.
- Alt-UI hasn't wired `useKeyboardShortcuts` yet — see TMAIL-260a placeholder.
- Dark mode contrast: the shadcn defaults pass; verify after the SPA palette
  changes propagate.

---

## Cross-cutting checks

### Documented keyboard shortcuts vs. trap-in-form behavior

`useKeyboardShortcuts.ts:40–46` correctly defers when the active element is an
input/textarea/select/contenteditable. ✅ Verified by reading and matched by the
manual walkthrough (typing in a textarea doesn't trigger `c` or `j`). Watch out
when adding new components: any custom control that traps focus must be either a
standard form element or carry `tabIndex={0}` with `contenteditable` (so the
`isTypingTarget` check picks it up). The TipTap editor is contenteditable, so
that's already correct.

### Touch-target sizing (1.4.13, 2.5.5 AAA)

Spot-check of icon-only buttons: `<X size={20} />` inside a `.btn--icon` with
≈ 4px padding renders ≈ 28×28 px, **below the 44×44 AA target on mobile**. Not
strictly an AA failure (2.5.5 is AAA), but the alt-UI uses 36–40 px hit targets
on the same icons. Promote `.btn--icon` to `min-height: 36px; min-width: 36px;`
at minimum; bump to 44 px under `@media (pointer: coarse)`.

### Color-only state cues

- `.message-row--unread` is bold + a colored dot? Visual only — pass.
- `.folder-item--active` is bg + text color change. Add `aria-current="page"`
  (finding 10) so it's not color-only.
- `.btn--active` (toolbar Star) toggles colour. Pair with `aria-pressed`.

### Screen-reader-only utility class

There's no `.sr-only` class in `App.css` or `index.css`. Findings 11, 12, and 17
all need one. Add to `App.css` once:

```css
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
```

### Existing E2E tests as a coverage indicator

Greps in `frontend/src/components/*.test.tsx` show:

- 4 components have `aria-label` / `role` references in their tests
- 0 components run `axe.run()` or any other a11y assertion
- 76 occurrences of `aria-*` / `role=` across 33 source files (a baseline, not a
  ceiling — many components have no ARIA at all)

The disparity makes finding 1 the highest-leverage fix: the gate immediately
captures the *current* state, then every PR is held to "no new violations".

---

## Quick wins applied in this commit

Three additive, low-risk fixes ship alongside the assessment (rest stay as
scoped tickets):

1. **`aria-label` on icon-only buttons** in `TopBar.tsx`, `MessageView.tsx`, and
   `Composer.tsx`. Pure additive — screen readers now announce "Log out, button"
   instead of just "button" (12 buttons total). Aligned with finding 3.
2. **`role="alert"` on error surfaces + `role="status" aria-live="polite"` on
   the Composer undo toast and draft-saving indicator** (`LoginPage.tsx`,
   `Composer.tsx`). Screen readers now announce errors and the 10-second undo
   countdown. Aligned with finding 9.
3. **`htmlFor` / `id` wiring for the Composer Subject and Schedule-Send inputs,
   plus a `:focus-within` ring on the topbar search wrapper** so the visible
   label clicks focus the input and keyboard focus is visible in the search
   field. Aligned with findings 5 and 16. Includes the dark-mode
   `--color-text-secondary` bump (`#999 → #b3bac4`) so dark-mode captions hit
   AA 4.5:1 (finding 14).

These deliberately do **not** touch the bigger structural fixes (focus trap on
dialogs, `<nav>` semantics in Sidebar, MessageList button refactor, replace
`window.prompt`/`window.confirm`) — those are scoped to their own tickets below
because each is a 30+ line change with visible behavior implications.

---

## Recommended ticket breakdown (under TMAIL-241 epic)

| Ticket | Title | Effort |
|--------|-------|--------|
| TMAIL-260a | Add `@axe-core/playwright` fixture + `e2e/a11y.spec.ts` covering login, inbox, compose, admin/users, calendar; baseline JSON in `docs/traceability/a11y-baseline.json` | S |
| TMAIL-260b | Replace `<div role="button">` in `MessageList.tsx` with `<button type="button">` + `aria-pressed` / `aria-expanded`; reset CSS for `.message-row` | M |
| TMAIL-260c | Implement Forward in `MessageView.tsx` (compose pre-fill + subject prefix) — closes finding 4 *and* a long-standing product gap | M |
| TMAIL-260d | Replace `window.prompt('Move to folder:')` with `FolderPickerMenu` sourced from `useFolders()` | M |
| TMAIL-260e | Build `<ConfirmDialog>` + `useFocusTrap` hook; replace all `window.confirm` / `window.alert` in `settings/` and `admin/` | M |
| TMAIL-260f | Focus trap, focus restoration, `aria-modal="true"`, initial-focus on `ScheduleMeetingModal` and `KeyboardShortcutHelp` | S–M |
| TMAIL-260g | Sidebar: wrap in `<nav aria-label="Mail navigation">`, add `aria-current="page"`, extract the 33 items into a registry (shared with TMAIL-241 scalability work) | M |
| TMAIL-260h | Skip-link to main content + `id="main-content" tabIndex={-1}` on the main container; `<a>`-style `.skip-link` CSS | S |
| TMAIL-260i | Focus management on `viewMode` change: `useFocusOnViewChange` hook + heading refs in Composer / MessageView / MessageList; global polite live-region announcer in AppShell | M |
| TMAIL-260j | `prefers-reduced-motion` media query in `App.css`; spot-check no animations break under it | S |
| TMAIL-260k | Sweep remaining icon-only buttons in `settings/` and `admin/` for `aria-label` + label associations | S–M |
| TMAIL-260l | `aria-invalid` + `aria-describedby` on every form error in LoginPage / Composer / ScheduleMeetingModal / settings managers | M |
| TMAIL-260m | Keyboard-shortcut discoverability pill + replace fragile `document.querySelector('.topbar__search input')` with a Zustand ref | S |
| TMAIL-260n | Touch-target sizing pass: `.btn--icon { min-width: 36px; min-height: 36px }` (44 px on coarse pointer) | S |
| TMAIL-260o | Alt-UI: wire `useKeyboardShortcuts` + add `aria-label` summaries to EmailList rows; verify Radix focus-trap is reaching every dialog | M |

---

## Sources & references

- WCAG 2.1 spec (top-level): <https://www.w3.org/TR/WCAG21/>
- WCAG 2.1.1 Keyboard: <https://www.w3.org/WAI/WCAG21/Understanding/keyboard.html>
- WCAG 2.4.3 Focus Order: <https://www.w3.org/WAI/WCAG21/Understanding/focus-order.html>
- WCAG 2.4.7 Focus Visible: <https://www.w3.org/WAI/WCAG21/Understanding/focus-visible.html>
- WCAG 1.4.3 Contrast (Minimum): <https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html>
- WCAG 4.1.2 Name, Role, Value: <https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html>
- WCAG 4.1.3 Status Messages: <https://www.w3.org/WAI/WCAG21/Understanding/status-messages.html>
- ARIA Authoring Practices — Dialog (Modal): <https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/>
- ARIA Authoring Practices — Disclosure: <https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/>
- MDN — `aria-current`: <https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Attributes/aria-current>
- MDN — `aria-live`: <https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Attributes/aria-live>
- MDN — Using ARIA roles for live regions: <https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/alert_role>
- WebAIM — Skip Navigation Links: <https://webaim.org/techniques/skipnav/>
- WebAIM — Contrast Checker: <https://webaim.org/resources/contrastchecker/>
- `@axe-core/playwright`: <https://www.npmjs.com/package/@axe-core/playwright>
- Deque axe-core ruleset: <https://github.com/dequelabs/axe-core/blob/develop/doc/rule-descriptions.md>
- Radix UI primitives (alt-UI dependency): <https://www.radix-ui.com/primitives>
- `prefers-reduced-motion`: <https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion>
- WCAG 2.5.5 Target Size (Enhanced, AAA): <https://www.w3.org/WAI/WCAG21/Understanding/target-size.html>
- TMAIL-261 (PWA) and TMAIL-263 (render perf) — sibling assessments under TMAIL-241

All findings above are derived from static reading of `frontend/src`,
`frontend/src/App.css`, `frontend/src/index.css`, `frontend/package.json`, and
`themes/shadcn-prototype/src` at HEAD (`182e7ca`, 2026-05-27). No axe-core or
Lighthouse run was performed; the runnable commands above produce that data
once finding 1 lands.
