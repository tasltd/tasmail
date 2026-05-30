# TASMail Classic UI — Gap Analysis (TMAIL-299)

**Date:** 2026-05-30
**Scope:** Server-rendered, no-JavaScript HTML/CSS webmail surface for low-bandwidth users, older browsers, screen readers, and text-only clients (lynx/w3m).
**Driver:** TMAIL-299 (Classic UI gap analysis & implementation)
**Companion reports:**
- `docs/gap-analysis/modern-ui.md` (TMAIL-298)
- `docs/gap-analysis/backend.md` (TMAIL-297)

---

## Executive Summary

**Classic UI does not exist.** A repository-wide scan for `classic`, `legacy`, `nojs`, `no-js`, `noscript`, `lite`, `mini`, `/m/`, or any server-side HTML templating engine (`tera`, `askama`, `maud`, `handlebars`, `minijinja`, `sailfish`, `ramhorns`) finds **zero** matches in the routing layer or the backend `Cargo.toml`. The 254 mounted backend routes are all JSON over HTTP, intended for SPA/mobile consumption. The two existing UIs are both JavaScript-heavy SPAs:

| Surface | Path | Tech | Requires JS? |
|---|---|---|---|
| Classic SPA | `/` | React 19 + Vite + TanStack Query | ✅ Yes (hard requirement) |
| Modern UI / alt-UI | `/modern/` | React 19 + shadcn/ui + Tailwind | ✅ Yes (hard requirement) |
| **Classic UI (no-JS)** | **`/classic`** *(does not exist)* | — | **N/A** |

> **Terminology trap.** Several files (`PendingSyncBanner.tsx`, the alt-UI's `← Classic` back link, `themes/shadcn-prototype/README.md`) refer to the **classic SPA** — that is the React 19 app, not the no-JS surface this report covers. The no-JS path is what's missing.

A user on a 2G/EDGE Ghanaian provincial network, on a feature-phone browser (Opera Mini, UC Browser legacy), on an old corporate Windows-7-locked-down image, or using a screen reader on a flaky connection has **no viable TASMail surface**: the React bundles fail to hydrate, time out, or simply refuse to load. The BYOK positioning ("works against any IMAP/SMTP") loses credibility when the UI itself can't run on the customer's actual hardware. The text-only / screen-reader story is also unsatisfying — `<noscript>` in the current SPA index falls back to a generic "this app requires JavaScript" message.

**This report's primary recommendation:** ship a small, server-rendered, form-based, CSP-tight, no-JS-required Classic UI mounted under `/classic` that covers the **mail golden path** — login → inbox → read → reply/compose → delete/move/star — plus the operations a beta customer **must** be able to perform from a stuck-on-2G phone (password change, 2FA challenge, sign-out, signature edit). Everything beyond that golden path stays in the SPA tiers.

Gap counts: **15 P0**, **18 P1**, **12 P2**, **8 P3** = **53 gaps total**.

The P0+P1 set (33 items) is what TMAIL-299 will spawn as child tasks for auto-fix. P2+P3 items are catalogued for the next pass but not auto-queued by this driver.

---

## Existence Check

| Probe | Command | Result |
|---|---|---|
| Directory `frontend/classic/` | `find . -type d -name classic*` | (none) |
| Directory `frontend/legacy/` | `find . -type d -name legacy*` | (none) |
| Backend `/classic`-prefixed route | `grep -nE '"/classic' backend/src/router.rs` | (none) |
| Backend `/legacy`-prefixed route | `grep -nE '"/legacy' backend/src/router.rs` | (none) |
| Backend `/m/`, `/lite/`, `/nojs/` route | `grep -nE '"/(m\|lite\|nojs\|mini)/' backend/src/router.rs` | (none) |
| Server-side templating crate | `grep -E '^(tera\|askama\|maud\|handlebars\|minijinja\|sailfish\|ramhorns)' backend/Cargo.toml` | (none) |
| Existing `<noscript>` body content | `grep -rn 'noscript' frontend/` | Generic "requires JavaScript" message in `index.html` only |
| References to "classic UI" in code | `grep -rn 'classic' frontend/src backend/src` | All matches refer to the **React SPA** ("classic SPA"), the "← Classic" back-link in alt-UI, or an unrelated comment about "classic interval-cover pattern" — none describe a no-JS surface |

**Verdict:** the Classic UI as defined by this driver task — *"a lightweight HTML/CSS path for low-bandwidth users and older browsers"* — does not exist in any form.

---

## Feature Parity Matrix (Modern UI vs Classic UI)

Source-of-truth for "Modern UI" in this column is `themes/shadcn-prototype/src/` per `docs/gap-analysis/modern-ui.md` (TMAIL-298), which itself is a re-skin / partial re-implementation of the classic SPA's surface. "Classic UI" reflects the *target* state once TMAIL-299's children land; today the entire Classic column is **Missing**.

Legend:
- ✅ Available
- ⚠️ Stub / partial / dead handler
- ❌ Missing
- ◻️ N/A with reason (e.g. JS-only feature has no useful no-JS analogue)

### Mail (golden path)

| Feature | Modern UI | Classic UI (target) | Notes |
|---|---|---|---|
| View inbox / folder listing | ⚠️ paginated to 50, no infinite scroll | ❌ Missing → P0 | `GET /classic/folders/{folder}?page=N` |
| Read a message (HTML body) | ✅ | ❌ Missing → P0 | Server-side sanitise via `ammonia`; strip `<script>`, inline events, external trackers |
| Read a message (text body) | ✅ | ❌ Missing → P0 | Text/plain fallback; `<pre>` block when no HTML body exists |
| Compose new message | ⚠️ no html_body, no attachments | ❌ Missing → P0 | `POST /classic/compose` with multipart for attachments |
| Reply / Reply All / Forward | ⚠️ buttons open empty composer | ❌ Missing → P0 | `GET /classic/compose?reply_to={folder}/{uid}` prefills body + headers |
| Delete (move to Trash) | ⚠️ button dead | ❌ Missing → P0 | `POST /classic/folders/{f}/messages/{uid}/delete` |
| Permanent delete (from Trash) | ⚠️ button dead | ❌ Missing → P1 | Same endpoint, `?permanent=1` confirm step |
| Move to folder | ⚠️ button dead | ❌ Missing → P1 | `POST .../move` with `target` form field |
| Mark read / unread | ❌ | ❌ Missing → P1 | `POST .../flag` |
| Star / unstar | ⚠️ button dead | ❌ Missing → P1 | `POST .../flag` `{add:["\Flagged"]}` |
| Multi-select bulk operations | ❌ | ❌ Missing → P1 | Form-based: checkbox per row + `<button name=action value=delete>` |
| Search | ⚠️ search bar dead | ❌ Missing → P1 | `GET /classic/search?q=...` |
| Attachment download | ⚠️ button dead | ❌ Missing → P0 | `GET /classic/folders/{f}/messages/{uid}/parts/{part_id}` |
| Attachment upload on compose | ⚠️ never sent | ❌ Missing → P1 | `<input type=file multiple>` + multipart POST |
| Conversation / thread view | ❌ | ◻️ N/A in v1 | Flat list is acceptable; thread grouping is a SPA feature |
| Folder list in sidebar | ⚠️ local-only CRUD | ❌ Missing → P0 | Read-only nav; full CRUD is P2 |
| Folder add/delete | ⚠️ local-only | ❌ Missing → P2 | Form-based |
| Quota usage | ❌ (admin only) | ❌ Missing → P1 | Shown in footer or sidebar |

### Auth

| Feature | Modern UI | Classic UI (target) | Notes |
|---|---|---|---|
| Login (password) | ⚠️ bounces to classic SPA | ❌ Missing → P0 | `POST /classic/login` with CSRF, sets signed session cookie |
| Logout | ❌ | ❌ Missing → P0 | `POST /classic/logout` with CSRF |
| Refresh token rotation | ◻️ JS-managed | ◻️ Cookie-managed | Server-driven via `Set-Cookie` on each request near expiry |
| Signup (BYOK) | ❌ | ❌ Missing → P1 | `GET /classic/signup` + multi-step BYOK wizard |
| Password reset | ❌ | ❌ Missing → P1 | Email-link flow |
| 2FA challenge (TOTP) | ❌ | ❌ Missing → P0 | Required if user has TOTP enrolled; login must short-circuit to challenge |
| 2FA challenge (SMS-OTP) | ❌ | ❌ Missing → P1 | |
| WebAuthn / Passkey | ❌ | ◻️ N/A (requires JS) | Force user to fall back to TOTP/SMS if passkey is sole factor |
| SAML / OIDC IdP redirect | ❌ | ❌ Missing → P2 | Server-side redirect to `/api/auth/{saml,oidc}/...` |
| Account lockout banner | ◻️ inline | ❌ Missing → P1 | Show remaining attempts / lockout-until time |

### Settings (the must-haves for a real beta)

| Feature | Modern UI | Classic UI (target) | Notes |
|---|---|---|---|
| Change password | ❌ | ❌ Missing → P1 | Form: current + new + confirm |
| Sign-out everywhere (revoke all refresh tokens) | ❌ | ❌ Missing → P1 | Single button + confirm |
| Signature CRUD + default | ❌ | ❌ Missing → P1 | Single signature form sufficient for v1 |
| Vacation responder | ❌ | ❌ Missing → P1 | Toggle + dates + subject + body |
| TOTP enrolment | ❌ | ❌ Missing → P2 | QR PNG inline + verify-code form |
| BYOK IMAP / SMTP edit | ❌ | ❌ Missing → P1 | Edit existing config (creation is signup wizard, P1 above) |
| Filters (Sieve) read-only list | ❌ | ❌ Missing → P2 | View-only — CRUD too form-heavy for v1 |
| Contacts list / search / add | ❌ | ❌ Missing → P2 | Minimum: list + view + add |
| Templates list / insert into composer | ❌ | ❌ Missing → P2 | Dropdown in composer |
| Push device list | ❌ | ◻️ N/A (no JS = no service worker) | Surface in SPA only |
| AI config (BYOK provider) | ❌ | ◻️ N/A in v1 | Power feature; admin SPA |

### Other surfaces

| Feature | Modern UI | Classic UI (target) | Notes |
|---|---|---|---|
| Calendar list view | ⚠️ create/delete only | ❌ Missing → P2 | Today / week list; no creation |
| Calendar create event | ⚠️ no attendees / RSVP / ICS | ❌ Missing → P3 | |
| Admin: users + domains + quota | ⚠️ read-only-ish | ❌ Missing → P3 | Admin-tier; ship after beta |
| Admin: all other panels (10+) | ❌ | ◻️ N/A in v1 | Power-admin surface; SPA only |
| Billing: view subscription | ❌ | ❌ Missing → P2 | Read-only; payment captures stay in SPA |
| Tasks (email-linked todos) | ❌ | ◻️ N/A in v1 | |
| Snooze | ❌ | ◻️ N/A in v1 | |
| Scheduled-send | ⚠️ broken backend (TMAIL-297 P0) | ◻️ N/A in v1 | Surface after backend P0-1 lands |
| EML export | ❌ | ❌ Missing → P2 | `GET .../eml` direct download |
| Print message | ❌ | ❌ Missing → P2 | Print stylesheet + browser print |
| Phishing banner on read | ❌ | ❌ Missing → P2 | CSS-only severity colours, no interactivity |
| Webhooks / chat integrations / SAML config / DLP / eDiscovery / migration / payment-provider config | ❌ in Modern | ◻️ N/A | Power-admin only |

---

## P0 — Critical (Classic UI scaffold + golden mail path)

Without these, there is no Classic UI. Auto-fix will spawn one task per row.

| # | Title | Affected area / proposed file paths | Proposed fix | PM task title |
|---|---|---|---|---|
| 1 | No Classic UI surface exists at all | `backend/Cargo.toml`, `backend/src/router.rs`, new `backend/src/handlers/classic/` module | Add `askama` (compile-time templates, no runtime parsing, no GPL dependency, type-checked context structs) to `[dependencies]`. Create `backend/src/handlers/classic/mod.rs` with a sub-router mounted at `/classic` from `router.rs`. Wire a 404 catch-all and `GET /classic/` → redirect to `/classic/login` or `/classic/folders/INBOX` based on session presence. | `[ClassicUI][P0] Scaffold /classic sub-router with Askama templates + base layout` |
| 2 | No HTML base layout / shell | new `backend/templates/classic/base.html` | Single `base.html` extends-able layout: HTML5 doctype, `<html lang>`, `<meta viewport>`, `<title>`, semantic `<header><nav><main><footer>`, skip-to-main link, inline `<style>` block keyed off a CSP nonce (so we keep `style-src 'self' 'nonce-xxx'` and avoid `unsafe-inline`). No `<script>` tags at all. WCAG AA contrast palette (Stone/Emerald per `branding/BRAND.md`). Print-friendly. | `[ClassicUI][P0] Base HTML layout: semantic, no-JS, WCAG AA, CSP-nonce inline styles` |
| 3 | No session/cookie auth path (SPA uses Bearer tokens in localStorage — useless without JS) | new `backend/src/handlers/classic/auth.rs`, new `backend/src/middleware/classic_session.rs` | Session = signed cookie (`tasmail_classic_sid`, `HttpOnly`, `Secure`, `SameSite=Lax`) holding an opaque server-side session id keyed to a row in `classic_sessions` (UUID PK, user_id FK, csrf_token, expires_at, last_seen_at). New migration `076_classic_sessions.sql`. Middleware resolves cookie → `AuthUser` and injects CSRF token into template context. | `[ClassicUI][P0] Session cookie + classic_sessions table + classic_session middleware` |
| 4 | No CSRF protection on form POSTs | `backend/src/handlers/classic/*`, all forms in `templates/classic/*` | Per-session CSRF token (random 32 bytes, base64) stored on `classic_sessions` row, injected as a hidden `<input name=_csrf>` into every form. Server validates on POST/PUT/DELETE. Reject mismatched / missing tokens with 403. | `[ClassicUI][P0] CSRF token on every form POST + server-side validation` |
| 5 | No login form | new `backend/src/handlers/classic/auth.rs::{login_form, login_submit}`, `templates/classic/login.html` | `GET /classic/login` renders form (email + password + CSRF). `POST /classic/login` validates via `auth_service::evaluate_password_login`, branches to 2FA challenge (#7) if enrolled else creates session row + sets cookie + 303-redirects to `/classic/folders/INBOX`. Honours existing brute-force lockout (mailbox `failed_login_attempts`, migration `073`). | `[ClassicUI][P0] Login form (email/password) with lockout-aware error rendering` |
| 6 | No logout | `backend/src/handlers/classic/auth.rs::logout_submit`, footer link in `base.html` | `POST /classic/logout` (form button — not GET — so it CSRF-protects) deletes the `classic_sessions` row, clears cookie, redirects to `/classic/login`. | `[ClassicUI][P0] Logout form button with CSRF + session row delete` |
| 7 | No 2FA challenge step | `backend/src/handlers/classic/auth.rs::{totp_challenge_form, totp_challenge_submit}`, `templates/classic/2fa_totp.html` | When login resolves to a user with TOTP enrolled, short-circuit before creating the full session and render the 6-digit TOTP form gated by a one-shot `pending_2fa_token` (5 min TTL). Validate via existing `totp_service::verify_code`. SMS-OTP variant is P1. | `[ClassicUI][P0] TOTP challenge form gating login when 2FA enrolled` |
| 8 | No inbox / folder list view | `backend/src/handlers/classic/folders.rs::list_messages`, `templates/classic/folder.html` | `GET /classic/folders/{folder}?page=N`: server-side pagination (25/page), `<table>` of `Sender | Subject | Date` with each row linking to read view. `<form>` checkboxes for bulk actions (P1 will hook them up). Folder nav `<ul>` in sidebar showing all folders from `imap_service::list_folders`. | `[ClassicUI][P0] Inbox / folder view with server-paginated message list + folder nav` |
| 9 | No message read view | `backend/src/handlers/classic/messages.rs::read_message`, `templates/classic/message.html` | `GET /classic/folders/{folder}/messages/{uid}`: render headers (`From`, `To`, `Cc`, `Subject`, `Date`), sanitised HTML body via `ammonia` (strip script, on*, javascript:, data:, external trackers — config block in `services/html_sanitizer.rs` to share with mobile-rendered emails too), or `<pre>` for text/plain. Attachment list with download links. Reply / Reply-All / Forward / Delete / Move buttons (all as forms). | `[ClassicUI][P0] Message read view: sanitised HTML, attachment list, action button row` |
| 10 | No HTML email sanitiser shared with backend | new `backend/src/services/html_sanitizer.rs`, dep `ammonia = "4"` | One sanitiser used by both Classic UI render and any mobile-render path. Strict allowlist: block all `<script>`, inline event handlers, `javascript:` URLs, `data:` URLs except images, external `<img src>` (gated by user pref — `block_remote_images` setting, default `true` for unknown senders). | `[ClassicUI][P0] Shared HTML email sanitiser (ammonia, strict allowlist, remote-image gating)` |
| 11 | No attachment download | `backend/src/handlers/classic/attachments.rs::download`, `templates/classic/message.html` | `GET /classic/folders/{folder}/messages/{uid}/parts/{part_id}`: streams attachment bytes with `Content-Disposition: attachment; filename=...` and `Content-Type` from the part. Re-uses existing IMAP fetch path. Filename sanitised against path traversal. | `[ClassicUI][P0] Attachment download endpoint streams part bytes with safe filename` |
| 12 | No compose form | `backend/src/handlers/classic/compose.rs::{compose_form, compose_submit}`, `templates/classic/compose.html` | `GET /classic/compose` (optional `?reply_to=`, `?reply_all=`, `?forward=` query params prefill body + headers from existing message). `POST /classic/compose` with `multipart/form-data`: `to`, `cc`, `bcc`, `subject`, `body`, `attachments[]`. Sends via existing `smtp_service::send` BYOK path. CSRF + size limits (25 MB total). | `[ClassicUI][P0] Compose form (new/reply/forward) with multipart attachments and BYOK send` |
| 13 | No delete (move to Trash) action | `backend/src/handlers/classic/messages.rs::delete_message`, button form in `message.html` and `folder.html` | `POST /classic/folders/{folder}/messages/{uid}/delete`: moves to Trash via `imap_service::move`. From Trash: a `?permanent=1` confirm form deletes via `imap_service::delete`. CSRF-protected, redirects back to folder. | `[ClassicUI][P0] Delete action moves to Trash (or permanently deletes from Trash with confirm)` |
| 14 | No CSP header on classic responses | `backend/src/middleware/security_headers.rs`, new path-aware branch | Apply distinct CSP for `/classic/*` responses: `default-src 'self'; style-src 'self' 'nonce-XXX'; img-src 'self' data: blob:; form-action 'self'; script-src 'none'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'`. `Referrer-Policy: same-origin`. `X-Content-Type-Options: nosniff`. `X-Frame-Options: DENY`. | `[ClassicUI][P0] Strict CSP for /classic/* (no scripts, nonce-only inline styles, no frame)` |
| 15 | No accessibility floor — without it the Classic UI fails its own raison d'être | All Classic UI templates | Every form input has a `<label for>`. Every button has visible text (no icon-only). Single `<h1>` per page. Landmarks `<nav>`, `<main>`, `<footer>`. Skip-link to `#main`. Lang declared. Focus order matches reading order. Tested with axe-core (via Playwright `@axe-core/playwright`) and lynx (smoke-render). | `[ClassicUI][P0] WCAG 2.2 AA pass on Classic UI templates (axe-core + lynx smoke render)` |

---

## P1 — Major (parity-narrowing for v1 closed beta)

| # | Title | Affected area | Proposed fix | PM task title |
|---|---|---|---|---|
| 16 | No mark-read / mark-unread action | `messages.rs`, `folder.html`, `message.html` | `POST .../flag` toggling `\Seen` flag. Bulk action via checkbox + submit. | `[ClassicUI][P1] Mark read / unread action (single + bulk)` |
| 17 | No star / unstar action | `messages.rs`, `folder.html`, `message.html` | `POST .../flag` toggling `\Flagged`. Visible ★ indicator in list. | `[ClassicUI][P1] Star / unstar action (single + bulk)` |
| 18 | No move-to-folder action | `messages.rs`, action row in `message.html` and `folder.html` | `POST .../move` with `target` select (folder dropdown rendered server-side). | `[ClassicUI][P1] Move-to-folder action with server-rendered folder dropdown` |
| 19 | No search results page | `backend/src/handlers/classic/search.rs`, `templates/classic/search.html` | `GET /classic/search?q=...&folder=...`: server-side proxy to `/api/search`, paginated results, query echoed in form. | `[ClassicUI][P1] Search form + paginated results page` |
| 20 | No signup / BYOK onboarding wizard | new `backend/src/handlers/classic/signup.rs`, `templates/classic/signup/{step1,step2,step3}.html` | Three-step server-side wizard: (1) account email + password, (2) pick preset IMAP provider or enter manual, (3) test + save. Mirrors SPA's `OnboardingWizard.tsx` but form-based and no JS. | `[ClassicUI][P1] Signup wizard with BYOK IMAP/SMTP setup (3 steps, no JS)` |
| 21 | No password reset | `backend/src/handlers/classic/password_reset.rs`, `templates/classic/password_reset_{request,confirm}.html` | Email-link flow: request → 1h-TTL token → confirm form. Reuse `auth_service::send_password_reset_link` if present; otherwise add. | `[ClassicUI][P1] Password reset request + confirm forms with email-token` |
| 22 | No change-password form | `backend/src/handlers/classic/settings.rs::password_form/submit`, `templates/classic/settings/password.html` | Current + new + confirm form. Calls existing `auth_service::change_password`. Invalidates other classic sessions. | `[ClassicUI][P1] Change-password form (with concurrent-session revoke)` |
| 23 | No sign-out-everywhere | `settings.rs`, `templates/classic/settings/sessions.html` | Lists active classic + SPA sessions; "Sign out everywhere" button revokes all refresh tokens + classic sessions for the user. | `[ClassicUI][P1] Sign-out everywhere (revokes refresh tokens + classic sessions)` |
| 24 | No signature settings | `settings.rs`, `templates/classic/settings/signature.html` | Single textarea (HTML allowed, sanitised) + default toggle. CRUD via existing `/api/signatures` model. Auto-appended on compose if default set. | `[ClassicUI][P1] Signature settings + auto-append on compose` |
| 25 | No vacation responder settings | `settings.rs`, `templates/classic/settings/vacation.html` | Enable toggle + start + end + subject + body + external-only checkbox. Calls `/api/auto-reply` model. | `[ClassicUI][P1] Vacation responder settings form` |
| 26 | No BYOK IMAP / SMTP edit | `settings.rs`, `templates/classic/settings/byok.html` | Edit existing user IMAP + SMTP configs (host/port/username/password/TLS). Test button posts to a verify endpoint that returns success/failure inline. | `[ClassicUI][P1] BYOK IMAP / SMTP edit form with test-connection button` |
| 27 | No SMS-OTP 2FA challenge | `classic/auth.rs::sms_otp_challenge`, `templates/classic/2fa_sms.html` | Sister of #7 for SMS-enrolled users. Triggers SMS send on form GET; 6-digit verify on POST. | `[ClassicUI][P1] SMS-OTP 2FA challenge form` |
| 28 | No attachment upload on compose | (covered scaffold in #12) | Already in P0 scaffold but flag-out as its own deliverable for the multipart streaming + virus scan hook + per-file size cap UX. | `[ClassicUI][P1] Compose attachment upload — multipart streaming + per-file size cap + DLP scan` |
| 29 | No multi-select / bulk actions on folder view | `folder.html`, `messages.rs` | Add `<input type=checkbox name=uid>` per row + bulk action button bar (`Mark read`, `Star`, `Delete`, `Move to…`). Server reads `uid[]` from form. | `[ClassicUI][P1] Multi-select checkboxes + bulk action bar on folder view` |
| 30 | No quota / storage display | `templates/classic/folder.html`, `base.html` footer | Footer line "Using X of Y" backed by `/api/quota`. Coloured warning at 80% / 95%. | `[ClassicUI][P1] Quota usage indicator in footer / sidebar` |
| 31 | No account-lockout messaging on login | `templates/classic/login.html` | When lockout is active, render countdown + "try again in N minutes". When attempts are close to limit, render `Warning: N attempts remaining`. | `[ClassicUI][P1] Login form surfaces lockout countdown + attempts-remaining warning` |
| 32 | No remote-image blocking control on read view | `templates/classic/message.html`, `services/html_sanitizer.rs` | Block external `<img src>` by default; render "[remote images blocked] · Show images" link that submits a form to re-fetch with images allowed for this UID + this sender. | `[ClassicUI][P1] Remote-image blocking with per-message Show-images opt-in` |
| 33 | No localisation (English-only) | `backend/src/handlers/classic/i18n.rs`, `templates/classic/i18n/*.json` | Server-side template translation via Accept-Language → load JSON dictionaries. Minimum strings: nav, buttons, error messages, form labels. Start with `en`, `tw` (Twi), `ee` (Ewe), `ga`, `ha` (Hausa) — mirrors the mobile app's Ghana-market locales (`mobile/lib/l10n/`). | `[ClassicUI][P1] i18n via Accept-Language: en + Twi + Ewe + Ga + Hausa` |

---

## P2 — Important polish & completeness

| # | Title | Affected area | Proposed fix |
|---|---|---|---|
| 34 | Folder add / delete / rename | `classic/folders.rs`, `templates/classic/settings/folders.html` | Form-based CRUD; calls `imap_service::create_folder` / `delete_folder` / `rename`. |
| 35 | Permanent delete from Trash with double-confirm | (extends P0 #13) | Dedicated confirm page rather than `?permanent=1` query. |
| 36 | EML download per message | `classic/messages.rs::download_eml` | `GET .../eml` returns raw message bytes, `Content-Disposition: attachment`. |
| 37 | Print stylesheet | `base.html` `<style media=print>` block | Hide nav/footer, full-width main, monochrome. Verified via Firefox print preview. |
| 38 | High-contrast / large-text user prefs (no-JS-friendly) | `settings.html` + `base.html` cookie-driven class | User saves preference → cookie → server adds `body class="theme-high-contrast" / "text-large"`. No JS toggle needed. |
| 39 | Calendar list view (today / this week) | `classic/calendar.rs`, `templates/classic/calendar.html` | Read-only list of next 50 events. No create/edit. |
| 40 | Filters (Sieve) read-only view | `classic/settings.rs::filters_view` | List active filters; "edit in modern UI" link. CRUD too form-heavy for v1. |
| 41 | Contacts list / view / add | `classic/contacts.rs` | Minimum surface: list (paginated) + view + add. Edit/delete punt to SPA. |
| 42 | Templates picker in composer | `templates/classic/compose.html` | `<select name=template_id>` populated from `/api/templates`; on submit, server-renders into body before saving. |
| 43 | TOTP enrolment | `classic/settings/2fa.html` | QR PNG inline (data URI) + verify-code form. Generation reuses `totp_service`. |
| 44 | Phishing banner on read | `templates/classic/message.html` | Coloured banner div at top of message body when `phishing_score >= threshold`. Pure CSS. |
| 45 | Error pages (404, 500, 503) | `templates/classic/errors/*.html` | Friendly, brand-tied, no leaking stack traces. 503 includes status-page link. |

---

## P3 — Nice-to-have / future

| # | Title | Affected area | Proposed fix |
|---|---|---|---|
| 46 | SAML / OIDC IdP redirect from login | `classic/auth.rs::login_form` | "Sign in with SSO" links generated from configured SAML/OIDC providers; classic flow leans on IdP-initiated. |
| 47 | Admin: read-only user + domain + quota list | `classic/admin.rs` | Surface only for `is_admin = true`; minimal subset. |
| 48 | Billing: view current subscription + invoices | `classic/billing.rs` | Read-only; new payments stay in SPA. |
| 49 | Snooze / scheduled-send | (cross-cutting) | Defer — both have UX flaws even in SPA (scheduled-send is broken backend-side per TMAIL-297 P0-1). |
| 50 | Tasks (email-linked todos) | — | Defer; power feature. |
| 51 | Phishing report action | `templates/classic/message.html` | Form button → `POST /api/folders/.../phishing/report`. |
| 52 | Push device list (read-only, "to enable, use Modern UI") | — | Just surface what's already enrolled. |
| 53 | Migration wizard (IMAP / MBOX / PST) | `classic/migration.rs` | Form-based upload + status page. Power feature; SPA suffices for v1. |

---

## Cross-cutting design notes (drives the P0/P1 implementations above)

### Tech choice: Askama, not Tera
- **Askama** — compile-time-checked templates, type-safe context, zero runtime parsing, MIT licence. The compile-time check is the load-bearing benefit: a rename of a model field will fail `cargo build` rather than blow up at request time. No template directory shipped at runtime — templates baked into the binary.
- **Tera** — runtime-parsed (Jinja2-like). More flexible but you lose the compile-time guarantee, and it's another moving piece in prod.
- **Maud** — Rust-DSL, no separate template files. Reads like Rust, but designers / external contributors can't touch templates without learning Rust syntax. Bad for the i18n + branding workstreams.
- **Decision:** Askama. Add to `backend/Cargo.toml` and create `backend/templates/classic/`.

### Auth model: session cookie ≠ JWT
- Classic UI cannot store a Bearer token without JS.
- Decision: **server-side session table** (`classic_sessions` row) referenced by an opaque, signed, HttpOnly+Secure cookie. Session row carries `user_id`, `csrf_token`, `created_at`, `expires_at` (24h sliding), `last_seen_ip`, `last_seen_ua`. Rotation handled server-side. Logout deletes the row.
- The Classic UI session and the SPA JWT are independent. Sign-out-everywhere (#23) revokes both.
- Sessions table uses RLS like every other tenant-scoped table.

### CSRF model
- Per-session token (32 random bytes, base64), stored on the session row.
- Injected into every `<form>` as a hidden `<input name=_csrf value="...">`.
- Validated server-side on every POST/PUT/PATCH/DELETE.
- Mismatch → 403 with retry-link to original page.
- No double-submit cookie pattern needed (we already have the server-side session).

### CSP model
- `/classic/*` carries its own CSP (separate from the SPA's CSP) via `security_headers.rs`.
- Strict: `script-src 'none'` (no JS at all), `style-src 'self' 'nonce-XXX'` (one nonce-tagged inline `<style>` in base layout), `img-src 'self' data: blob:` (data URIs for QR codes; blob for attachment previews), `form-action 'self'`, `frame-ancestors 'none'`, `base-uri 'self'`.
- Every Classic UI response generates a fresh nonce per request.

### HTML email sanitisation
- Use `ammonia` (currently absent from Cargo.toml) for HTML sanitisation. Single config block in `services/html_sanitizer.rs`, shared between Classic UI render and any future mobile-render path.
- Strict allowlist: block all `<script>`, `<iframe>`, `<object>`, `<embed>`, inline event handlers (`onclick`, `onload`, …), `javascript:` URLs, `data:` URLs except `data:image/...`.
- Remote `<img>` blocked by default (per #32); user opt-in per message + per sender.

### Browser support floor
- Firefox 60+ / Chrome 70+ / Safari 12+ / Opera Mini.
- Smoke-tested in `lynx` and `w3m` (text-only) — the structural / WCAG check.
- No CSS-grid or features that block IE11; flexbox + floats are fine. Mobile-first via `<meta viewport>` and CSS-only responsive design.

### Routing convention
- `/classic/` → home (redirect)
- `/classic/login` `/classic/logout` `/classic/signup` `/classic/2fa/{totp,sms}` `/classic/password-reset/{request,confirm}`
- `/classic/folders/{folder}` (list) `/classic/folders/{folder}/messages/{uid}` (read) `/classic/folders/{folder}/messages/{uid}/{delete,move,flag,parts/{part_id}}` (actions)
- `/classic/compose` (form + submit) `/classic/search`
- `/classic/settings/{password,signature,vacation,byok,sessions,2fa,folders,filters,contacts}`
- `/classic/calendar` (P2)
- `/classic/admin/{users,domains,quota}` (P3, admin-gated)
- `/classic/errors/{404,500,503}` (rendered by global error handler)

### Tests
- Backend: `#[cfg(test)] mod tests` colocated with each handler. Render template + assert key strings present + assert CSRF token present + assert no `<script>` in output.
- E2E: Playwright spec under `frontend/e2e/specs/classic-ui-*.spec.ts` running against Firefox (per global rule) **plus** a `--browser=webkit` smoke for Safari-compat. Use `page.evaluate("'undefined' === typeof window.fetch ? 'no-js' : 'js-available'")` only to assert pages render without depending on JS. Capture screenshots at every key validation point per global E2E rule.
- Text-only smoke: `lynx -dump /classic/login` etc. as a CI shell job — quick regression catch.
- Axe-core accessibility: `npx playwright test --grep classic-ui-a11y` covers every Classic UI page.

---

## Methodology

1. Repo-wide grep for `classic`, `legacy`, `nojs`, `no-js`, `noscript`, `lite`, `mini`, `/m/`, and every common Rust HTML templating crate name.
2. Inspected `backend/src/router.rs` (254 routes) for any `/classic`-style prefix — none.
3. Inspected `backend/Cargo.toml` `[dependencies]` for templating crates — none.
4. Cross-referenced `docs/gap-analysis/modern-ui.md` (TMAIL-298) for the Modern UI feature inventory used as the parity baseline.
5. Cross-referenced `frontend/src/api/` (~50 modules) and `themes/shadcn-prototype/src/api/` (10 modules) to size what *could* be exposed via Classic UI and prioritise.
6. Validated terminology: every existing "classic" reference in the codebase actually refers to the **React 19 classic SPA**, never to a no-JS surface. Confirmed `frontend/index.html`'s `<noscript>` block is a generic "JS required" message, not a usable fallback.
7. Derived the P0 list by intersecting **mail golden path** (login → inbox → read → compose → delete) with **non-negotiables for any no-JS webmail in 2026** (CSRF, CSP, session cookie, 2FA challenge, HTML sanitisation, attachment download, WCAG AA, accessibility-as-feature).

## Companion docs

- `docs/gap-analysis/modern-ui.md` — Modern UI gaps (TMAIL-298)
- `docs/gap-analysis/backend.md` — Backend gaps (TMAIL-297)
- `docs/GAP-ANALYSIS.md` — Workspace-wide gap analysis (older, mobile-focused)
- `docs/research/feature-comparison.md` — Cross-product feature comparison (raw research)
- `branding/BRAND.md` — Palette + typography source-of-truth that Classic UI must honour

## Audit-fix queue

This document is the source-of-truth gap list for the Classic UI. TMAIL-299 will spawn a child PM task per **P0 and P1** row above and queue each for auto-fix. **P2/P3** items are catalogued for a later pass but not auto-queued by this driver.
