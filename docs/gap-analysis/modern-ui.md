# TASMail Modern UI — Gap Analysis (TMAIL-298)

**Date:** 2026-05-30
**Scope:** `themes/shadcn-prototype/src/` — the alt-UI shipped at `/modern/`
**Driver:** TMAIL-298 (Modern UI gap analysis & implementation — React 19)
**Companion report:** `docs/gap-analysis/backend.md` (TMAIL-297)

---

## Executive Summary

The Modern UI is a **functional but minimal** re-skin of TASMail. Only three feature screens exist
(`EmailClient`, `AdminDashboard`, `CalendarView`), backed by **~15 of the 254 backend routes**.
The classic SPA at `frontend/src/` exposes **30+ settings managers** and consumes the full
backend surface; the Modern UI implements roughly **6% of that surface**. Critical daily-mail
interactions are visually present but **wired to dead handlers**: star, archive, delete,
reply prefill, attachment download, attachment upload on send, search bar, settings button,
rich-text formatting, folder add/delete (server-side), and message-move all render UI without
firing the corresponding backend mutation.

Beyond dead handlers, the Modern UI lacks:

- **Standalone auth** (login/signup/onboarding wizard) — it bounces to the classic SPA
- **Any Settings surface** — signatures, vacation responder, 2FA, IMAP/SMTP config, AI, push, theme persistence
- **Contacts, signatures, templates, filters, tasks, snooze, queue, delegation, webhooks, AI features**
- **Threading, search results, message move, multi-select, pagination, keyboard shortcuts**
- **WebSocket live updates** for new mail
- **Most admin tooling** — audit log, branding, SAML/OIDC/LDAP, retention/legal-holds, DLP, eDiscovery, payment providers, feature flags, warmup, custom hostnames
- **PWA** (no service worker, no install manifest, no push registration)
- **Theme polish** — Navbar uses `blue-600` despite the project's stated Stone/Emerald palette; dark mode is in-memory only; reduced-motion is unhandled

Gap counts: **16 P0**, **24 P1**, **18 P2**, **9 P3** = **67 gaps total**.

The P0+P1 set (40 items) is what TMAIL-298 will spawn as child tasks for auto-fix. P2+P3
items are catalogued for the next pass but not auto-queued by this driver.

---

## Screen Inventory

| Route | Component | File | API surface used | Status |
|-------|-----------|------|------------------|--------|
| `/#/` | `EmailClient` | `features/email/EmailClient.tsx` | `/api/folders`, `/api/folders/{f}/messages` | Live but read-only-ish; star/archive/delete/move/multi-select missing |
| `/#/` (modal) | `ComposeModal` | `features/email/ComposeModal.tsx` | `/api/messages/schedule`, `/api/drafts` | Send works; attachments + rich-text + html_body dead |
| `/#/` (right pane) | `EmailReader` | `features/email/EmailReader.tsx` | `/api/folders/{f}/messages/{uid}` | Reads body; all action buttons dead except Reply→empty composer |
| `/#/admin` | `AdminDashboard` | `features/admin/AdminDashboard.tsx` | `/api/admin/users`, `/api/admin/domains`, `/api/quota` | Users + domains + quota only; nothing else from `/api/admin/*` |
| `/#/calendar` | `CalendarView` | `features/calendar/CalendarView.tsx` | `/api/calendar/events`, `/api/calendar/events/{id}` (DELETE) | Create + delete + list; no edit, attendees, RSVP, ICS, free-busy, recurrence |

**Components-only (no route):**
- `components/layout/Root.tsx` — Outlet shell, dark-mode toggle (in-memory only)
- `components/layout/Navbar.tsx` — Settings + Search buttons are dead
- `components/layout/Sidebar.tsx` — Folder CRUD is local-only; never POSTs to backend
- `components/ui/*` — shadcn primitives (49 files), all consumable, no gaps

**API clients shipped:** `auth`, `client`, `constants`, `folders`, `messages`, `scheduled`, `calendar`, `admin-users`, `admin-domains`, `quota` (10 modules)

**API clients in classic SPA but missing from Modern UI:** `activesync`, `admin-payment-providers`, `admin-warmup`, `ai-config`, `archive`, `attachments`, `audit`, `auto-reply`, `branding`, `bulk-import`, `byok` (BYOK IMAP config), `cache`, `chat-integrations`, `comments`, `contact-groups`, `contacts`, `custom-hostnames`, `dane`, `dav-config`, `delegation`, `deliverability`, `dlp`, `ediscovery`, `eml`, `enterprise-quote`, `feature-flags`, `filters`, `groups`, `imap-configs`, `ldap`, `mfa` (2fa/sms-otp/webauthn), `migration`, `oidc`, `ollama`, `phishing`, `plugins`, `pop3`, `push`, `queue`, `retention`, `saml`, `search` (advanced), `semantic-search`, `shared-files`, `shared-mailboxes`, `signatures`, `smtp-configs`, `spam`, `sync`, `tasks`, `templates`, `webhooks` (~50 modules)

---

## P0 — Critical (blocks basic daily mail use)

These break the user's primary expectations about a webmail UI. Auto-fix will spawn one task per row.

| # | Title | File:line | Proposed fix | PM task title |
|---|-------|-----------|--------------|---------------|
| 1 | Star toggle is a dead button in list | `features/email/EmailList.tsx:32-44` | Wire onClick to `PATCH /api/folders/{folder}/messages/{uid}/flag` with `{add:["\\Flagged"]}` / `{remove:["\\Flagged"]}`; invalidate `['messages',folder]` query. Add `aria-pressed`, keyboard activation. | `[ModernUI][P0] Star button in EmailList must call /flag endpoint` |
| 2 | Star toggle is a dead button in reader | `features/email/EmailReader.tsx:64-68` | Same fix as #1, plus invalidate `['message',folder,uid]`. | `[ModernUI][P0] Star button in EmailReader must call /flag endpoint` |
| 3 | Archive button does nothing | `features/email/EmailReader.tsx:102-105` | Wire to `POST /api/folders/{folder}/messages/{uid}/move` targeting `Archive` (create folder if absent); clear `selectedUid`, invalidate folder + messages queries. | `[ModernUI][P0] Archive button must move message to Archive folder` |
| 4 | Delete button does nothing | `features/email/EmailReader.tsx:106-109` | Wire to `/move` targeting `Trash`, or `DELETE /api/folders/{folder}/messages/{uid}` for permanent delete from Trash. Confirm before permanent delete. | `[ModernUI][P0] Delete button must move to Trash (or permanent-delete from Trash)` |
| 5 | Reply / Reply All / Forward open empty composer | `features/email/EmailReader.tsx:88-100` | Pass the open message into `ComposeModal` with prefilled to/cc/subject (`Re:` / `Fwd:` prefix), quoted body, and original `In-Reply-To` / `References` headers wired into the send request. | `[ModernUI][P0] Reply / Reply All / Forward must prefill recipients, subject, quoted body, References header` |
| 6 | Attachment download button has no onClick | `features/email/EmailReader.tsx:152-155` | Use `/api/folders/{folder}/messages/{uid}/parts/{part_id}` (or whichever attachment route the backend exposes — confirm in handlers/attachments.rs) and trigger a browser download via blob URL. | `[ModernUI][P0] Attachment Download button must fetch and save the attachment` |
| 7 | Compose attachments tracked in state but never uploaded | `features/email/ComposeModal.tsx:79-95, 36-44` | Extend `scheduledApi.scheduleSend` (and backend if needed) to accept `multipart/form-data` with file parts, OR upload files to `/api/attachments` first and pass IDs in the JSON body. Enforce 25 MB total. | `[ModernUI][P0] Compose attachments must actually be sent with the message` |
| 8 | Search bar has no onSubmit / no results page | `components/layout/Navbar.tsx:38-45` | Add controlled input, debounced query, push to `/#/search?q=...` route; render results page consuming `/api/search` (or `/api/search/nlp` if AI provider configured). | `[ModernUI][P0] Search bar must submit to /api/search and render a results page` |
| 9 | Settings button has no onClick / no Settings page exists | `components/layout/Navbar.tsx:31-33` | Create `/#/settings` route with side-tab layout (Profile, Identities, Signatures, Vacation, Filters, MFA, Theme, IMAP/SMTP). Wire Settings icon to it. (Settings sub-panes are P1 items below.) | `[ModernUI][P0] Wire Settings button to a /settings route with side-tab shell` |
| 10 | Folder add/delete are local-only (never persisted) | `components/layout/Sidebar.tsx:57-81` | Backend exposes IMAP folder CRUD via mailbox provisioning — confirm endpoint, then wire add to a real POST and delete to a real DELETE; remove `extraLocalFolders` state in favour of `useQuery` invalidation. | `[ModernUI][P0] Sidebar folder add/delete must persist to backend (not local-only)` |
| 11 | No pagination / "Load more" in EmailList | `features/email/EmailClient.tsx:45-49` | Switch to `useInfiniteQuery` driven by `page` param; render sentinel + intersection observer to auto-load next page. | `[ModernUI][P0] EmailList needs infinite scroll / pagination (currently hardcoded 50)` |
| 12 | No multi-select / bulk operations | `features/email/EmailList.tsx`, `features/email/EmailClient.tsx` | Add per-row checkbox + `selectedUids` state; surface a bulk-action bar (mark read/unread, star, archive, delete, move). Wire to existing `/flag` and `/move` endpoints per UID. | `[ModernUI][P0] EmailList must support multi-select with bulk flag / move / delete` |
| 13 | Modern UI cannot stand alone (bounces to classic /login) | `app/App.tsx:14-24` | Build native `/#/login` and `/#/signup` screens inside the Modern UI using existing `/api/auth/login` + `/api/auth/signup` clients; only fall back to classic if no JWT and user explicitly clicks "Use classic login". | `[ModernUI][P0] Build native login + signup screens so Modern UI runs standalone` |
| 14 | WebSocket not wired — no live new-mail updates | (absent) | Open `WS_URL` on mount, subscribe to `new_mail` / `unread_changed`, invalidate folder + messages queries on event. Reconnect with backoff. | `[ModernUI][P0] Subscribe to /ws for new-mail events and refresh folder counts live` |
| 15 | List rows show blank preview (no body excerpt) | `features/email/EmailClient.tsx:66-82` | Backend message envelope already exposes `preview` (verify in `handlers/folders.rs`) — map it. If absent, add a `preview` field to the envelope endpoint. | `[ModernUI][P0] EmailList rows must show message preview (currently blank)` |
| 16 | Rich-text toolbar buttons (Bold/Italic/Link/List) are dead; html_body never sent | `features/email/ComposeModal.tsx:184-213, 36-44` | Swap the `<Textarea>` for a TipTap (or Lexical) instance — classic SPA already uses TipTap; reuse its composer module. Send the rendered HTML as `html_body` alongside `text_body` (plain-text fallback derived from the doc). | `[ModernUI][P0] Composer must use a real rich-text editor and send html_body` |

---

## P1 — Major gaps vs industry-standard webmail / vs classic SPA

| # | Title | Affected area | Proposed fix | PM task title |
|---|-------|---------------|--------------|---------------|
| 17 | No Signatures settings UI | `/api/signatures` | Inside `/#/settings/signatures`: list, create, edit (TipTap), set default, delete. Insert default on compose. | `[ModernUI][P1] Settings — Signatures CRUD + default selection + compose insertion` |
| 18 | No Vacation responder UI | `/api/auto-reply` | `/#/settings/vacation`: enable toggle, subject, body, start/end dates, "external only" toggle. | `[ModernUI][P1] Settings — Vacation responder / auto-reply UI` |
| 19 | No Contacts UI | `/api/contacts`, `/api/contact-groups` | `/#/contacts`: list/search/add/edit/delete contacts; manage groups; vCard import/export. Plus autocomplete in compose To/Cc/Bcc. | `[ModernUI][P1] Contacts management UI + recipient autocomplete in composer` |
| 20 | No Templates UI | `/api/templates` | `/#/settings/templates`: list, create with variables, render preview. Insert into composer via toolbar. | `[ModernUI][P1] Settings — Email templates with variable rendering and composer insertion` |
| 21 | No Sieve Filters UI | `/api/filters` | `/#/settings/filters`: ordered list, create/edit/delete rules, reorder via drag, `Test` button using `/filters/{id}/test`. | `[ModernUI][P1] Settings — Sieve filter rules with reorder + test` |
| 22 | No Tasks UI (email-linked todos) | `/api/tasks` | `/#/tasks` + per-message "Add to tasks" action; due dates, status toggle. | `[ModernUI][P1] Tasks UI (email-linked todos) with per-message attach action` |
| 23 | No Snooze UI | `/api/messages/snooze`, `/api/messages/snoozed` | Per-message snooze button + duration picker; `Snoozed` view in sidebar; unsnooze action. | `[ModernUI][P1] Snooze message UI + Snoozed folder view` |
| 24 | No Scheduled-send list / cancel UI | `/api/messages/scheduled`, `/api/messages/cancel/{token}` | `Scheduled` view in sidebar listing pending sends with cancel button (uses existing cancel_token). | `[ModernUI][P1] Scheduled-send list with cancel-before-send` |
| 25 | No Queue management UI | `/api/queue`, `/api/queue/{id}/retry`, `/api/queue/stats` | `/#/settings/queue` or admin sub-panel: list, retry failed, cancel queued, show stats. | `[ModernUI][P1] Outbound email queue UI (list, retry, cancel, stats)` |
| 26 | No Delegation UI | `/api/delegation`, `/api/delegation/granted` | `/#/settings/delegation`: grant send-as / read access to other mailboxes, view granted access. | `[ModernUI][P1] Settings — Delegation (send-as / shared-mailbox grants) UI` |
| 27 | No 2FA setup UI | `/api/2fa/*`, `/api/sms-otp/*`, `/api/webauthn/*` | `/#/settings/security`: TOTP enrol (QR), SMS-OTP enrol, WebAuthn/Passkey register, recovery codes, list & revoke methods. | `[ModernUI][P1] Settings — Multi-factor auth (TOTP / SMS-OTP / WebAuthn) setup UI` |
| 28 | No Webhook config UI | `/api/webhooks` | `/#/settings/webhooks`: CRUD endpoints, pick events, view delivery log via `/webhooks/{id}/deliveries`, retry. | `[ModernUI][P1] Settings — Outbound webhooks UI` |
| 29 | No Chat-integration UI (Slack/Teams/Discord) | `/api/chat-integrations` | `/#/settings/integrations`: CRUD with `Test` button. | `[ModernUI][P1] Settings — Chat integrations (Slack / Teams / Discord) UI` |
| 30 | No AI config UI (BYOK provider) | `/api/ai/config`, `/api/ai/*` | `/#/settings/ai`: pick provider (OpenAI/Anthropic/Ollama/Gemini), store key, test; enable compose-assist / smart-reply / summarize / thread-summary toggles. Wire composer "AI" button when enabled. | `[ModernUI][P1] Settings — AI provider config (BYOK) + compose-assist + summarize features` |
| 31 | No Migration UI | `/api/migration/*` | `/#/settings/import`: IMAP migration wizard, MBOX upload, PST upload; progress + cancel. | `[ModernUI][P1] Settings — Email migration (IMAP/MBOX/PST) UI` |
| 32 | No Onboarding wizard for BYOK IMAP/SMTP | `/api/imap-configs/*`, `/api/smtp-configs/*` | Native wizard in Modern UI mirroring classic SPA's `OnboardingWizard.tsx`: pick preset, fill creds, test, save. Required for standalone-Modern-UI signup flow (P0 #13). | `[ModernUI][P1] Onboarding wizard for BYOK IMAP/SMTP setup` |
| 33 | No Phishing banner / report UI | `/api/folders/{f}/messages/{uid}/phishing`, `.../phishing/scan`, `/api/phishing/{id}/action` | On message open, GET phishing status; render banner with severity + actions (Mark safe / Report). | `[ModernUI][P1] Phishing detection banner + Report action in EmailReader` |
| 34 | No Email comments thread | `/api/folders/.../comments`, `/api/comments/{id}` | Below reader body: list comments, post new, edit/delete own. | `[ModernUI][P1] Per-message comments thread UI in EmailReader` |
| 35 | No EML export / import | `/api/folders/.../eml`, `.../export-mbox`, `.../import-eml` | `Export EML` action on message; folder-level `Export MBOX` and `Import EML` in folder header / settings. | `[ModernUI][P1] EML export per-message + MBOX export/import per folder` |
| 36 | No thread / conversation view | (cross-cutting) | Group messages by `In-Reply-To` / `References`; collapse all but the latest; expand-on-click; show participants. Use existing `/api/folders/.../messages` data plus reference headers. | `[ModernUI][P1] Conversation/thread grouping in EmailList with collapse-expand` |
| 37 | Calendar — no edit, attendees, RSVP, recurrence, ICS download, free-busy | `features/calendar/CalendarView.tsx`, `/api/calendar/events/{id}`, `.../rsvp`, `.../ics`, `/api/calendar/free-busy`, `/api/calendar/suggest-slots` | Edit dialog reusing the create form; attendees chip input; RSVP responder when event_id has invite; recurrence picker (RRULE); ICS download button; free-busy column in attendee picker; `suggest-slots` button. | `[ModernUI][P1] Calendar — edit, attendees, RSVP, recurrence, ICS, free-busy, suggest-slots` |
| 38 | Admin — no audit log view | `/api/admin/audit-log` | Add admin tab with paginated, filterable audit log. | `[ModernUI][P1] Admin — Audit log viewer` |
| 39 | Admin — no branding, SAML, OIDC, LDAP config | `/api/admin/branding`, `/api/admin/saml`, `/api/admin/oidc`, `/api/admin/ldap` | Sub-tabs under Admin: branding (logo/colors), SAML providers, OIDC providers, LDAP sources. CRUD + test. | `[ModernUI][P1] Admin — Branding + SAML + OIDC + LDAP configuration UIs` |
| 40 | Admin — no retention, legal-holds, DLP, eDiscovery | `/api/admin/retention`, `/api/admin/legal-holds`, `/api/admin/dlp/*`, `/api/admin/ediscovery/*` | Sub-tabs under Admin: retention policies CRUD, legal-hold create/release, DLP rules + violations, eDiscovery cases + execute + export. | `[ModernUI][P1] Admin — Retention + Legal-holds + DLP + eDiscovery UIs` |

---

## P2 — Important polish, accessibility, completeness

| # | Title | Affected area | Proposed fix |
|---|-------|---------------|--------------|
| 41 | Empty preview/from values in list show "(unknown sender)" / "(no subject)" rather than skeleton | `EmailClient.tsx:67-78` | Show shimmer skeleton rows while loading instead of placeholder strings. |
| 42 | Dark mode is in-memory only (lost on reload) | `Root.tsx:6-21` | Persist to `localStorage` and (when authed) to `/api/users/me/preferences`. Respect `prefers-color-scheme` on first visit only. |
| 43 | No keyboard shortcuts (`j/k`, `c`, `?`, `r`, `e`) | (cross-cutting) | Add a `useKeyboardShortcuts` hook + `?` overlay. Match classic SPA's shortcut set. |
| 44 | No focus trap on Compose modal | `ComposeModal.tsx` | Use Radix Dialog primitives (already in shadcn) instead of a hand-rolled fixed-position div. |
| 45 | Many icon-only buttons missing `aria-label` | `Sidebar.tsx`, `EmailReader.tsx`, `ComposeModal.tsx`, `AdminDashboard.tsx`, `CalendarView.tsx` | Audit every `<Button size="icon">` and add `aria-label`. |
| 46 | No skip-to-main-content link | `Root.tsx` | Add visually-hidden skip link as first focusable element. |
| 47 | No reduced-motion handling | (cross-cutting) | Wrap transitions in `motion-safe:` Tailwind variant or `prefers-reduced-motion` media query. |
| 48 | Theme palette inconsistent — uses blue/zinc, not the stated Stone/Emerald | `Navbar.tsx`, `EmailReader.tsx`, `AdminDashboard.tsx` | Either commit to Stone/Emerald via Tailwind tokens or document the actual choice in `BRAND.md`. |
| 49 | No PWA manifest or service worker for `/modern/` | `themes/shadcn-prototype/` | Mirror classic SPA's `vite-plugin-pwa` config; cache shell, runtime cache for `/api`. |
| 50 | No push device registration | `/api/push/register`, `/api/push/devices` | Add to Settings → Notifications: register service worker, request permission, enable/disable per-account, set quiet hours. |
| 51 | No offline / online indicator | (cross-cutting) | Banner / status dot driven by `navigator.onLine`; queue mutations via background-sync when offline. |
| 52 | No error boundary — a single render crash blanks the app | `App.tsx` | Add `<ErrorBoundary>` around `<RouterProvider>` (mirror classic SPA's shared/ErrorBoundary). |
| 53 | Star button in EmailList is a `<button>` inside a clickable `<div>` row — keyboard activates the row, not the star | `EmailList.tsx:23-46` | Either lift the row to a `<button>` and isolate the star with `pointerdown` stop, or use a properly labelled `<button aria-pressed>` outside the row. |
| 54 | No drag-and-drop folder moves | `EmailList.tsx`, `Sidebar.tsx` | DnD via `react-dnd` or HTML5 native: drop on folder triggers `/move`. |
| 55 | No `Mark as read/unread` action | `/api/folders/.../flag` | Toolbar action in EmailReader + bulk action with multi-select. |
| 56 | No `Print` action | `EmailReader.tsx` | Print stylesheet + button. |
| 57 | No `View raw / View source` action | `EmailReader.tsx` | Surface source EML via existing `.../eml` endpoint. |
| 58 | No date-grouping headers in list (Today / Yesterday / This week / Older) | `EmailList.tsx` | Sticky headers between groups. |
| 59 | No quota bar in main mail UI (only on admin) | `Sidebar.tsx`, `/api/quota` | Mini quota bar in Sidebar footer with `bytesUsed / quota`. |

---

## P3 — Nice-to-have / future

| # | Title | Affected area | Proposed fix |
|---|-------|---------------|--------------|
| 60 | Admin — no Custom hostnames | `/api/admin/hostnames` | Sub-tab with CRUD + verify. |
| 61 | Admin — no Payment-provider config | `/api/admin/payment-providers` | Sub-tab to manage Paystack/MPGS/Cybersource/BankTransfer keys. |
| 62 | Admin — no Feature-flags toggle | `/api/admin/feature-flags` | Sub-tab with live toggle. |
| 63 | Admin — no Ollama/AI provider admin | `/api/admin/ollama/*` | Sub-tab: status, models, pull, config. |
| 64 | Admin — no IP-warmup scheduler | `/api/admin/warmup/*` | Sub-tab with schedule + start + status. |
| 65 | Admin — no Bulk-import UI (api client exists, no UI) | `/api/admin/users/bulk-import*` | Upload CSV, preview rows, run import, view past imports. |
| 66 | Admin — no Cache stats / flush | `/api/admin/cache/*` | Sub-tab. |
| 67 | No CalDAV config UI | `/api/dav/configs` | Settings → Calendars: CRUD configs with test + sync. |

---

## Cross-cutting issues (not in priority tables — handled by individual fixes above)

- **Token-bearer for `multipart/form-data` uploads** — `ApiClient` only sends `Content-Type: application/json`; needs an overload that omits content-type so the browser sets the multipart boundary. (Already worked around in `admin-users.ts:bulkImport` via raw fetch — needs a first-class helper.)
- **No global toast / notification system** — errors are surfaced inline per-mutation. shadcn ships `sonner` (`components/ui/sonner.tsx`) but no `<Toaster />` is mounted in `Root.tsx`. Mount it and route mutation errors through it.
- **No tests in this directory** — `themes/shadcn-prototype/` has zero `*.test.ts(x)` files. Classic SPA colocates `.test.ts` next to every module. Adopt the same pattern as features land.

---

## Methodology

1. Enumerated every `.tsx` / `.ts` under `themes/shadcn-prototype/src/` (75 files, 49 of which are shadcn primitives).
2. Read every route, feature, layout, and API module in full.
3. Extracted backend routes via `grep -E '"/api/[^"]+"' backend/src/router.rs` → 254 unique paths.
4. Diffed: Modern-UI API surface (~15 routes) vs backend total (254) vs classic SPA API surface (~50 modules in `frontend/src/api/`).
5. Cross-checked each interactive element (button, link, form) for a real handler vs a placeholder / empty arrow / "would go here" comment.
6. Verified loading / error / empty states on every `useQuery` and `useMutation`.
7. Scanned for a11y violations (icon-only buttons without `aria-label`, focus traps, keyboard activation, contrast).

## Companion docs

- `docs/gap-analysis/backend.md` — Backend gaps (TMAIL-297)
- `docs/GAP-ANALYSIS.md` — Workspace-wide gap analysis
- `themes/shadcn-prototype/README.md` — Modern UI status notes

## Audit-fix queue

This document is the source-of-truth gap list for the Modern UI. TMAIL-298 will spawn a child
PM task per P0 and P1 row above and queue each for auto-fix. P2/P3 items are catalogued for a
later pass but not auto-queued by this driver.
