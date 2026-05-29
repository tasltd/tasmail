# TMAIL-283 — E2E sweep: folders + paginated message list + unread + multi-select

- **Issue:** TMAIL-283 (sibling of TMAIL-280 / TMAIL-281 / TMAIL-282 E2E sweeps)
- **Date:** 2026-05-29
- **Spec:** [`frontend/e2e/folder-messagelist.spec.ts`](../../frontend/e2e/folder-messagelist.spec.ts)
- **Screenshots:** [`frontend/e2e/screenshots/folder-messagelist/`](../../frontend/e2e/screenshots/folder-messagelist/)
- **Target:** Live mail.techatscale.io against the noreply@techatscale.io BYOK mailbox on Stalwart (`swmail.techatscale.io`).
- **Browser:** Firefox (per the E2E HARD RULE).
- **Workers:** 1 (suite uses `mode: 'serial'` so the mutation tests act on a shared BYOK setup).

---

## TL;DR

All 8 specs pass after the **one bug fix** this commit ships. The sweep proved the read-path E2E contract end-to-end against a real Stalwart-backed mailbox of ~1 050 messages, and uncovered one real bug + two UI gaps already on the backlog under TMAIL-263.

| # | Surface | Outcome | Follow-up |
|---|---------|---------|-----------|
| 1 | Folder tree renders system folders with badges | ✅ Pass — folder list + INBOX badge match `/api/folders` |
| 2 | Clicking INBOX renders the paginated message list (page 0 default) | ✅ Pass — `/api/folders/INBOX/messages?page=0&page_size=50` honoured; `page=1` cross-check confirms backend pagination works | UI: no "Load more" button → **TMAIL-263 finding #4** (already filed) |
| 3 | Opening an unread message flips `\Seen` and decrements the unread badge | ✅ Pass — API state before/after asserted | — |
| 4 | Starring persists `\Flagged` to IMAP | ✅ Pass — `expect.poll` confirms flag flips both directions | — |
| 5 | Moving a message decrements the source folder count | ✅ Pass — API total before/after asserted | — |
| 6 | Deleting reduces the INBOX total | ⚠️ Required a **backend fix** to ship in this commit (see §1 below) — now passes | Fixed in `backend/src/services/imap_service.rs` this commit |
| 7 | Empty folder renders the "no messages" empty state | ✅ Pass — Drafts on the BYOK account is empty | — |
| 8 | Multi-select / bulk action bar | ⚠️ Surface **does not exist** in the production SPA — gap asserted | New ticket: **TMAIL-284 (suggested)** — see §3 |

---

## 1. Bug fixed in this commit — `delete_message` hardcoded the trash folder name

**Symptom (reproduced live):** clicking the trash-icon button in `MessageView` for a message in INBOX returned 200 but the message stayed in INBOX. The poll in the test caught it (`expected 1048, received 1049`). Screenshots: [`before-delete.png`](../../frontend/e2e/screenshots/folder-messagelist/before-delete.png), [`after-delete.png`](../../frontend/e2e/screenshots/folder-messagelist/after-delete.png).

**Root cause:** `ImapService::delete_message` (`backend/src/services/imap_service.rs`) hardcoded the destination folder name as the literal string `"Trash"`. Stalwart's special-use trash folder on the noreply mailbox is `"Deleted Items"`. The IMAP `COPY "INBOX" "Trash"` silently failed (server returned `NO` because the folder doesn't exist), no expunge ran, and the message stayed put. Same fault hits Gmail (`[Gmail]/Trash`), Outlook 365 (`Deleted Items`), iCloud (`Deleted Messages`), and ProtonMail Bridge.

This is **exactly** the finding the prior code-review assessment already filed:

> [`folders-messages-2026-05.md` finding #7](folders-messages-2026-05.md) — "Folder names are hardcoded strings everywhere — the `FOLDER_*` registry is dead code. … High (correctness for non-Dovecot BYOK servers — Gmail, Outlook, ProtonMail Bridge all use non-standard special-use names). New (P0 for Gmail/Outlook BYOK users)."

That assessment proposed the long-term fix (RFC 6154 `LIST … RETURN (SPECIAL-USE)` auto-discovery on connect). This commit ships the **minimal path** that unblocks the BYOK fleet today without paying for the auto-discovery work yet:

| Change | File | Diff shape |
|---|---|---|
| `ImapService` gains a `user_trash_folder: Option<String>` field, populated from `ImapConfiguration.trash_folder` in `for_user()` | `backend/src/services/imap_service.rs` | +1 field, +5 lines in `for_user()` |
| New `trash_folder()` accessor returns the configured value, falling back to `"Trash"` for legacy / global-config callers | `backend/src/services/imap_service.rs` | +6 lines |
| `delete_message` calls `self.trash_folder()` instead of the literal `"Trash"` | `backend/src/services/imap_service.rs` | -2 / +3 lines |
| Two new unit tests cover both branches (default + override) | `backend/src/services/imap_service.rs` (tests mod) | +30 lines |
| BYOK setup in the new E2E spec now sets `trash_folder: "Deleted Items"` (the Stalwart name) so the live test runs through the fixed path | `frontend/e2e/folder-messagelist.spec.ts` | +4 lines |

**Backward compatibility:** legacy `ImapService::new(config)` callers (the single-tenant Dovecot self-host path used in tests and in `EML import/export` handlers) get `user_trash_folder: None`, which resolves to `"Trash"` — bit-identical behaviour to before.

**Scope intentionally NOT covered in this commit:**
- The same hardcoding applies to `Drafts`, `Sent`, `Junk`/`Spam`, and `Archive` (drafts save in `save_draft`, move-to-junk for phishing, etc.). The fix pattern is identical (read from `ImapConfiguration.{drafts,sent,spam,archive}_folder`) but is out-of-scope for TMAIL-283's read-path sweep. **Recommend a new ticket** (TMAIL-285 suggested) to roll the same change across the rest of the special folders, plus the RFC 6154 auto-discovery to populate the columns when the wizard isn't given explicit values.

---

## 2. Confirmed working — flag / move / mark-read all round-trip correctly

The three mutation tests cross-check the backend state with a fresh API GET — never trusting UI assertions on their own (per the E2E SPA HARD RULE).

| Spec | Before-state probe | UI action | After-state probe | Result |
|---|---|---|---|---|
| Mark-read (test 3) | `GET /api/folders` → `INBOX.unseen` | Click unread row → MessageView opens | `GET /api/folders/INBOX/messages/:uid` → `\Seen` in `flags`; `GET /api/folders` → `INBOX.unseen` strictly lower | ✅ |
| Star (test 4) | `GET /api/folders/INBOX/messages/:uid` → `\Flagged` presence | Click Star/Unstar button | `expect.poll` on the same GET — toggle takes effect | ✅ |
| Move (test 5) | `GET /api/folders/INBOX/messages?page=0&page_size=50` → `total` | Click Move button → accept native `prompt()` with destination folder | Same GET; INBOX `total` decremented by exactly 1 | ✅ |

Notes for future maintainers:

- **MessageView's Move button uses `window.prompt()`.** That's a debug-grade UX — the production target is a folder picker dropdown. Filed as a UX nicety, not part of this commit. The test uses `page.on('dialog', d => d.accept(destFolder))` to drive it.
- **MessageList groups by normalised subject by default.** ThreadRows toggle expansion on click instead of opening MessageView. The spec untoggles the "Conversations" checkbox after entering INBOX so each row opens MessageView on click. The helper is reusable as `openInboxFlat(page)` if more specs need the same affordance.
- **Stalwart returns INBOX with ~1050 messages on the noreply test mailbox** (mostly `Mail Delivery Subsystem` bounces from auto-fix retries). Large enough to exercise the read path realistically; small enough that the SPA's "fetch 50, render all" pattern doesn't surface OOM yet.

---

## 3. UI gaps confirmed (no production code in scope to add — filing for follow-up)

### 3.1 No multi-select / bulk-action bar in `MessageList` — suggest **TMAIL-284**

`frontend/src/components/mail/MessageList.tsx` renders:

- Per row: `MessageRow` = `{from, subject, date}` — **no checkbox**, no selection state, no bulk-affordances.
- Above the list: `{count text} {EML import button} {Conversations toggle}` — no Select-all, no "0 selected" indicator, no Mark-as-read / Delete / Move on the bar.

The spec asserts the absence so a future regression would force an explicit update to both the spec and this doc:

```ts
await expect(page.locator('.message-list .message-row input[type="checkbox"]')).toHaveCount(0);
await expect(page.locator('.message-list__bulk-actions, .message-list__action-bar')).toHaveCount(0);
```

Screenshot: [`multi-select-gap.png`](../../frontend/e2e/screenshots/folder-messagelist/multi-select-gap.png).

**Suggested ticket — TMAIL-284 "MessageList multi-select + bulk action bar"** (feature):

- Per-row checkbox in `MessageRow` (skip the ThreadRow header for now — bulk-on-threads is a v2).
- `mailStore` gains `selectedUids: Set<number>` + `toggleSelectedUid`, `clearSelectedUids`.
- Sticky `.message-list__bulk-actions` shows when `selectedUids.size > 0`: count, Mark read/unread, Star, Move (folder picker), Delete, Clear.
- Backend already supports per-uid mutations — the bulk endpoint is a quality-of-life optimisation (POST `/api/folders/{folder}/messages/bulk-action`) but not a blocker. Ship sequential per-uid calls first; add the bulk endpoint when CPU profiling shows it.
- Keyboard: `Shift+click` to range-select, `Ctrl/Cmd+A` to select-all visible. Wire to `useKeyboardShortcuts`.
- Test coverage: extend `folder-messagelist.spec.ts` with `select-3-and-mark-read.spec.ts`-style cases asserting API state before/after.

### 3.2 No "Load more" affordance — already filed as **TMAIL-263 finding #4**

The backend honours `?page=N&page_size=N` (the spec calls `?page=1&page_size=10` and asserts a 200 with the same `total`), but `useCurrentMessages()` (`frontend/src/hooks/useMailbox.ts:78–81`) hardcodes `useMessages(folder, 0, 50)`, so a user with 200 messages in Inbox sees only the 50 newest with no way to scroll past them.

Screenshot: [`inbox-page-2-attempt.png`](../../frontend/e2e/screenshots/folder-messagelist/inbox-page-2-attempt.png) — list scrolled to the bottom of the rendered 50 rows, no "Load more" button below.

No new ticket required — TMAIL-263 owns this. The TMAIL-263 fix should switch to `useInfiniteQuery` + `@tanstack/react-virtual` (both already in `package.json`) so the same component scales to ~50 k rows without DOM bloat.

### 3.3 Stalwart's INBOX has ~1 050 messages — confirms TMAIL-263 finding #1

Same `MessageList` rendering all 50 rows in the DOM at once: no virtualisation. With Stalwart returning 1 049 in `total`, the SPA still only renders the page-0 slice (50), so the DOM isn't bloated yet — but the moment infinite scroll is wired, virtualisation becomes mandatory or render time spikes linearly. Filed under TMAIL-263 finding #1.

---

## 4. Validation methodology

Every spec in the suite follows the same recipe and the same HARD RULES:

1. **Login is UI-driven** (`page.goto('/login')` then fill + submit). The only `page.goto()` calls in the file are the documented exception (initial login URL). All folder navigation is menu-click only.
2. **BYOK provisioning is API-driven** in `beforeAll` — the wizard is already covered by `signup-byok-flow.spec.ts` and `byok-noreply-end-to-end.spec.ts`, so re-walking it here would just burn auth-rate-limit budget without adding coverage.
3. **Seeding is idempotent.** If INBOX has < 3 messages at the start of the suite, three messages are self-sent via `/api/messages/send` so the mutation tests have something to act on. (Against the live noreply mailbox there were already 1 049 messages, so seeding was a no-op.)
4. **Every key validation point has a screenshot** under `e2e/screenshots/folder-messagelist/{action}.png` per the E2E_SCREENSHOTS HARD RULE.
5. **Every mutation cross-checks the backend state with a fresh `GET`** — UI assertions on toast / DOM changes are never trusted alone.
6. **Cleanup is wired** — `afterAll` deletes the throwaway `e2e-folder-*@e2e.tasmail` mailbox so re-runs stay idempotent.

---

## 5. Known limitations / follow-up

| Item | Status |
|------|--------|
| Threading default makes single-row click ambiguous in the production UI — the spec works around with `openInboxFlat()` (untoggle Conversations). A nicer UX would be: clicking a `ThreadRow` header opens the *latest* message in the thread; the chevron next to it toggles expand. | Out of scope for TMAIL-283. Suggested: small UX ticket. |
| `delete_message` is fixed; `save_draft` ("Drafts"), `move_to_junk` (Spam scanner output), and the `Sent` / `Archive` paths still hardcode their folder names. | **Suggested ticket — TMAIL-285 "Honour ImapConfiguration.{drafts,sent,spam,archive}_folder across the IMAP service"** + RFC 6154 SPECIAL-USE auto-discovery to populate the columns. |
| The Move button uses `window.prompt()`. Works, but ugly — and bypasses folder validation. | Suggested UX ticket (folder picker dropdown). |
| The spec uses one BYOK account for all 8 tests in serial mode. Parallel runs across distinct accounts would surface RLS-tenant-bleed regressions in `/api/folders` / `/api/folders/:f/messages`. | Out of scope; the existing per-tenant tests in `backend/src/handlers/messages.rs` cover the tenant boundary. |

---

## 6. Test artefacts

- **Spec:** [`frontend/e2e/folder-messagelist.spec.ts`](../../frontend/e2e/folder-messagelist.spec.ts) — 8 tests, ~480 LOC, serial.
- **Screenshots:** 14 files under [`frontend/e2e/screenshots/folder-messagelist/`](../../frontend/e2e/screenshots/folder-messagelist/) — pristine folder tree, page 1 + page-2-attempt, before/after for each mutation, empty-folder, multi-select-gap evidence.
- **Backend unit tests:** `test_trash_folder_defaults_to_legacy_name` + `test_trash_folder_honours_per_user_override` in `backend/src/services/imap_service.rs` (tests mod) — both green.
- **Live run:** all 8 tests green, total wall-clock ~2.6 minutes against `https://mail.techatscale.io`.

---

## 7. Related work

- [`folders-messages-2026-05.md`](folders-messages-2026-05.md) — TMAIL-243 code-review assessment of the IMAP read path. Finding #4 (no pagination UI) and finding #7 (hardcoded special folders, P0 for Gmail/Outlook BYOK) are referenced above. This commit ships the minimal slice of finding #7.
- [`e2e-auth-2026-05.md`](e2e-auth-2026-05.md) — TMAIL-281 sister sweep (auth + onboarding).
- [`e2e-mfa-2026-05.md`](e2e-mfa-2026-05.md) — TMAIL-282 sister sweep (MFA).
- `TMAIL-263` — open ticket for the frontend render-perf gaps (no virtualisation, no infinite scroll, missing `React.memo`).
