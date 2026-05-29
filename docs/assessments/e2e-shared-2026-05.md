# TMAIL-289 — E2E sweep: shared mailboxes + shared files (token DL)

- **Issue:** TMAIL-289 (sibling of TMAIL-281 / 282 / 283 / 284 / 285 / 286 / 287 / 288 settings sweeps)
- **Date:** 2026-05-29
- **Spec:** [`frontend/e2e/shared-mailboxes-files.spec.ts`](../../frontend/e2e/shared-mailboxes-files.spec.ts)
- **Screenshots:** [`frontend/e2e/screenshots/shared/`](../../frontend/e2e/screenshots/shared/) — 11 PNGs covering the grant form, ACL list, switch-to-shared-mailbox indicator, upload form, file row, copy-link feedback, public download (incognito context), expired-token error, and the empty-state-after-revoke states for both surfaces.
- **Target:** Local backend on `127.0.0.1:3300` via the Vite dev server at `127.0.0.1:5273` (`PLAYWRIGHT_BASE_URL=http://localhost:5273`). The same backend serves `https://mail.techatscale.io` over the reverse tunnel, so the spec also runs against the live URL with no arg overrides.
- **Browser:** Firefox (per the E2E HARD RULE).
- **Workers:** 1 (`mode: 'serial'` — the three tests share two BYOK signups).

---

## TL;DR

All 3 tests pass on a clean run after the **two bug fixes** this commit ships.
The sweep proves that the two shared-collaboration surfaces — shared
mailboxes (Dovecot ACL) and shared files (large-file token DL) — round-trip
through the SPA, backend, and database when navigated through the sidebar,
including the public `/api/dl/{token}` path served against an incognito
browser context.

| # | Surface | Outcome | Bug fix |
|---|---------|---------|---------|
| 1 | Shared mailboxes: grant ACL → grantee sees row + ACL panel → revoke clears it | ⚠️ → ✅ | **Bug 1** + **Bug 2** (below) |
| 2 | Shared files: upload via UI → copy-link → public DL (incognito) → max-downloads expiry → delete | ✅ Pass | — |
| 3 | Public `/api/dl/{token}` with an unknown token returns 404 | ✅ Pass | — |

---

## 1. Bug 1 — `shared_mailbox_acl` RLS was keyed on a session var the auth middleware no longer sets

[`backend/migrations/010_shared_mailboxes.sql`](../../backend/migrations/010_shared_mailboxes.sql)
created the ACL table with `FORCE ROW LEVEL SECURITY` and two policies that
referenced `current_setting('app.mailbox_id', true)`. That was correct at the
time — the auth middleware then SET `app.mailbox_id` on the pool connection
before each request.

[`backend/src/middleware/auth.rs:58–68`](../../backend/src/middleware/auth.rs)
documents the TMAIL-161 refactor that removed the `SET app.mailbox_id` call:
the SET landed on a connection the handler never actually received, since
each handler query acquires a fresh pool connection. The fix at the time was
to switch every protected handler to explicit `WHERE user_id = $N`
filters — defense-in-depth without the SET noise.

But migration 010 was never realigned. Every subsequent table that grew RLS
standardised on `app.current_user_id` (migrations 017 webauthn, 019 attachments,
020 phishing reports, 028 shared files, …), and the `shared_mailbox_acl`
policy was orphaned. Its `current_setting('app.mailbox_id', true)::uuid` cast
would silently return NULL (the policy use of `with_default = true`
suppresses the "unrecognised configuration parameter" error), the comparison
to `granted_to` evaluated to NULL, and FORCE RLS forced even the table owner
to satisfy the policy — meaning **every SELECT against `shared_mailbox_acl`
from the app's pool connection returned zero rows**.

The live system "worked" only because of connection-pool state leakage. Two
unrelated code paths still SET session vars on whichever connection they
borrowed:

- [`backend/src/services/auth_service.rs:390`](../../backend/src/services/auth_service.rs)
  — login still runs `set_config('app.mailbox_id', …, false)` on the pool
  connection it borrows. The setting persists on that connection until the
  pool recycles it.
- [`backend/src/models/audit_log.rs:40`](../../backend/src/models/audit_log.rs)
  — audit-log inserts SET `app.is_admin = 'true'` for the local write,
  which then satisfies the `shared_acl_admin` policy on subsequent queries
  reusing the same connection.

In practice, after a login + an audit-log write, the connection that next
served a `/api/shared-mailboxes` request would happen to evaluate the
policy to `true` and return rows. After a connection recycle or a cold pool,
the same call would return `[]`. Fragile, race-prone, and impossible to
reason about.

Reproduced directly:

```bash
$ psql tasmail
-- Same role the backend uses; no app.mailbox_id set.
=> SHOW row_security;
 row_security
--------------
 on
=> SELECT current_setting('app.mailbox_id', true) AS m;
 m
---

=> SELECT mailbox_id, granted_to FROM shared_mailbox_acl WHERE mailbox_id = '<owner>';
 mailbox_id | granted_to
------------+------------
(0 rows)
```

…even though the row visibly exists when queried as superuser.

**Fix:**
[`backend/migrations/075_shared_mailbox_acl_rls_align.sql`](../../backend/migrations/075_shared_mailbox_acl_rls_align.sql)
drops `FORCE ROW LEVEL SECURITY` and rewrites the policies to use
`app.current_user_id` (matching every post-010 RLS migration). With FORCE
off, the app role (the table owner) bypasses RLS — defense-in-depth lives
at the handler level, which already enforces
`mailbox_id == claims.sub OR claims.is_admin OR delegated can_admin` for
every grant / list / revoke entry point. The new policies are still present
so any future call that does opt into RLS via
`services::db_session::acquire_with_rls` will be scoped correctly.

The migration is idempotent (DROP POLICY IF EXISTS for both the legacy
names and the new name before each CREATE).

**Verification:** the new spec creates two BYOK users, grants
`read+write+admin` from owner → grantee via `POST /api/shared-mailboxes/{owner}/acl`,
and asserts that `GET /api/shared-mailboxes` as the grantee returns exactly
that one row with `can_admin === true`. The screenshot
`shared-mailbox-list-indicator.png` shows the row painted in the
`SharedMailboxManager` with the green "Admin" badge — the
"switch-to-shared-mailbox indicator" the PM description asked for.

---

## 2. Bug 2 — Delegated `can_admin` grantees got 403 on the same endpoints the UI invited them to use

The `SharedMailboxManager` panel only expands a shared-mailbox row and
shows the grant form + ACL list when `mailbox.can_admin === true`:
[`frontend/src/components/settings/SharedMailboxManager.tsx:170–253`](../../frontend/src/components/settings/SharedMailboxManager.tsx).
The UI explicitly promises ACL management to delegated admins — a fellow
admin of a shared mailbox should be able to add and revoke users.

But the three ACL endpoints all gated on
`mailbox_id != current_id && !claims.is_admin → 403 Forbidden`:

- `GET  /api/shared-mailboxes/{mailbox_id}/acl`
- `POST /api/shared-mailboxes/{mailbox_id}/acl`
- `DELETE /api/shared-mailboxes/{mailbox_id}/acl/{user_id}`

The mailbox owner always passes (`mailbox_id == claims.sub` by definition).
System admins always pass. **But a delegated `can_admin` grantee — the
exact user the UI invites to expand the panel — was rejected**, so
`aclEntries = []`, the ACL list panel showed "No ACL entries. Grant access
to allow other users…" forever, and clicking "Grant Access" submitted to a
backend that returned 403 on every POST.

**Fix:** centralise the auth gate in a single helper:
[`backend/src/handlers/shared.rs:22–49`](../../backend/src/handlers/shared.rs).

```rust
async fn assert_can_manage_acl(
    state: &AppState,
    claims: &Claims,
    mailbox_id: Uuid,
) -> Result<Uuid, AppError> {
    let current_id = parse_mailbox_id(claims)?;
    if mailbox_id == current_id || claims.is_admin {
        return Ok(current_id);
    }
    // Delegated admin lookup — must be a can_admin = true row on this mailbox.
    let delegated: Option<(bool,)> = sqlx::query_as(
        "SELECT can_admin FROM shared_mailbox_acl
         WHERE mailbox_id = $1 AND granted_to = $2",
    )
    .bind(mailbox_id)
    .bind(current_id)
    .fetch_optional(&state.db)
    .await?;
    match delegated {
        Some((true,)) => Ok(current_id),
        _ => Err(AppError::Forbidden(
            "Not the mailbox owner or a delegated admin".to_string(),
        )),
    }
}
```

All three handlers now call `assert_can_manage_acl` first, so the
ownership / system-admin / delegated-admin trio is enforced consistently.

**Verification:** the spec grants `can_admin = true` to the grantee,
expands the mailbox row in the manager, and asserts:

- `.acl-entry` containing the grantee's email is visible
  (`shared-mailbox-acl-list.png`).
- The "Grant Access" button opens the form with
  `input[placeholder="User UUID to grant access"]` present
  (`shared-mailbox-grant-form.png`).

Before the fix, the React Query `enabled: expanded && mailbox.can_admin`
fires the GET, the backend returns 403, the React Query result is `[]`, and
the panel paints "No ACL entries" — which is what the first version of the
spec saw and which we caught on the very first local run.

---

## What each spec asserts

### 1) Shared mailboxes — grant → indicator → ACL list → grant form → revoke

- Pre-condition: `GET /api/shared-mailboxes` as the grantee returns `[]`.
- API: `POST /api/shared-mailboxes/{owner}/acl` with `read + write + admin`.
- API cross-check: `GET /api/shared-mailboxes` as the grantee returns one
  row with `mailbox_id == owner` and `can_admin === true`.
- Login as the grantee via the UI → click sidebar **Shared Mailboxes**.
- DOM: `.mailbox-item` row contains the owner's email, "Admin" badge is
  visible (validates the migration-075 + handler fix end-to-end).
- Click the row header → ACL panel expands.
- DOM: `.acl-entry` containing the grantee's email is visible (validates
  Bug 2 fix — without it the panel paints empty).
- Click **Grant Access** → grant form opens with the User-UUID input.
- API: `DELETE /api/shared-mailboxes/{owner}/acl/{grantee}`.
- `page.reload()` to force a clean React Query refetch.
- DOM: empty-state `<p class="empty-state">No shared mailboxes available…</p>` visible.
- API: `GET /api/shared-mailboxes` returns `[]`.

### 2) Shared files — upload → token → copy-link → public DL (incognito) → expired → delete

- Pre-condition: `GET /api/shared-files` baseline.
- Build a small fixture file in `os.tmpdir()` so the multipart upload has a
  real disk path to attach.
- Login as the grantee → click sidebar **Shared Files**.
- DOM: empty-state "No shared files yet" visible.
- `setInputFiles(fixturePath)` + fill `max_downloads = 2` + click **Upload &
  Share**.
- DOM: row visible with the fixture filename and the green "Active" badge.
- API cross-check: `GET /api/shared-files` returns the new row; capture the
  `download_token` (must match `/^[0-9a-f]{64}$/`).
- Click the copy-link button → visual feedback (Copy icon swaps to Link
  icon for 2 s). Firefox blocks clipboard writes from the test profile, so
  we assert the icon swap rather than the clipboard contents.
- **Incognito context:** `browser.newContext({ acceptDownloads: true })`,
  navigate to a tiny in-page `<a href="…/api/dl/{token}" download>` element,
  click it, and capture the `download` event. Assert the suggested filename
  matches the fixture.
- **APIRequestContext (no auth header):** `GET /api/dl/{token}` returns 200
  with `Content-Disposition: attachment; filename="…"` and a body that
  byte-matches the fixture. This is the first of two downloads.
- The incognito click is the second download. Both succeeded, count is
  now 2 of 2.
- **Expired:** third `GET /api/dl/{token}` returns 400 with a body containing
  "expired". A second incognito `page.goto({url})` renders the JSON error
  inline (no Content-Disposition this time) and asserts the body contains
  "expired".
- API: `DELETE /api/shared-files/{id}`.
- `page.reload()` to force a clean React Query refetch.
- DOM: empty-state "No shared files yet" visible again.
- API cross-check: `GET /api/shared-files` does not contain the deleted row.

### 3) Negative path — unknown public token

- Cheap supplementary test, no UI hops.
- A fresh `APIRequestContext` with no auth GETs `/api/dl/{f×64}`.
- Asserts HTTP 404 + a JSON body containing "not found" or "invalid".

---

## Screenshots

| File | What it captures |
|------|------------------|
| `shared-mailbox-list-indicator.png` | Grantee's **Shared Mailboxes** panel showing the owner's row + the green "Admin" badge — the switch-to-shared-mailbox indicator. |
| `shared-mailbox-acl-list.png` | Expanded ACL panel showing the grantee's own entry (proves Bug 2 fix). |
| `shared-mailbox-grant-form.png` | Grant Access form with the User-UUID input + permission checkboxes. |
| `shared-mailbox-empty-after-revoke.png` | Empty-state copy after revoke. |
| `shared-files-empty.png` | Empty-state on first navigation to **Shared Files**. |
| `shared-file-upload-form-filled.png` | Upload form with fixture file attached and `max_downloads = 2`. |
| `shared-files-list-after-upload.png` | File row with green "Active" badge + copy/delete icons. |
| `shared-file-copy-link-feedback.png` | Copy → Link icon swap after clicking the copy-link button. |
| `shared-file-public-download-success.png` | Incognito context after the public DL succeeded. |
| `shared-file-public-download-expired.png` | Incognito context rendering the 400 "expired" error inline. |
| `shared-files-empty-after-revoke.png` | Empty-state on Shared Files after API delete + reload. |

---

## Run instructions

```bash
# 1. Apply the new migration + rebuild the backend release binary.
cd backend
cargo build --release
systemctl --user restart tasmail-backend.service

# 2. Run the spec on Firefox.
cd ../frontend
PLAYWRIGHT_BASE_URL=http://localhost:5273 \
  npx playwright test e2e/shared-mailboxes-files.spec.ts --project=firefox

# Or against the live tunnel (the default base URL):
npx playwright test e2e/shared-mailboxes-files.spec.ts --project=firefox

# 3. (Optional) Open the HTML report.
npx playwright show-report
```

The `afterAll` hook deletes both test mailboxes via `psql` so re-runs start
clean — see [`frontend/e2e/helpers/db-cleanup.ts`](../../frontend/e2e/helpers/db-cleanup.ts).
Make sure `psql` is available and `TASMAIL_DB_URL` is reachable.

---

## Files changed in this commit

| File | Change | Why |
|------|--------|-----|
| `backend/migrations/075_shared_mailbox_acl_rls_align.sql` | NEW — drops `FORCE RLS`, rewrites policies to use `app.current_user_id`. Idempotent. | Bug 1 |
| `backend/src/handlers/shared.rs` | Add `assert_can_manage_acl` helper that also honours delegated `can_admin`; route list / grant / revoke through it. Add two `parse_mailbox_id` unit tests. | Bug 2 + test coverage |
| `frontend/e2e/shared-mailboxes-files.spec.ts` | NEW — the sweep itself, 3 tests. | The deliverable |
| `frontend/e2e/screenshots/shared/*.png` | NEW — 11 screenshots | Audit trail |
| `docs/assessments/e2e-shared-2026-05.md` | NEW — this doc | Discoverability |

---

## Follow-up tickets (suggested)

| # | Suggestion | Why |
|---|------------|-----|
| 1 | **Audit other `app.mailbox_id`-only callers.** `services::auth_service::login` and `models::audit_log` still SET session vars on the pool. Now that nothing depends on them (handlers do explicit `WHERE`, RLS is realigned), those calls can be removed — they're the source of the pool-state leakage that masked Bug 1. | Removes a footgun the next RLS migration will trip over. |
| 2 | **Surface a "Shared *by* me" panel for mailbox owners.** Today the `SharedMailboxManager` only lists mailboxes you've been granted access to — your own mailbox's ACL is unmanageable from the SPA unless you go in as a delegated admin. A second tab in the panel listing rows from `GET /api/shared-mailboxes/{me}/acl` would close the loop. | The grant flow is currently one-way: you can grant, but you can't see your own grants until somebody adds you as a delegated admin on your own mailbox. Awkward. |
| 3 | **Email-/username-based grant form**, with a `GET /api/users/lookup?email=…` helper for non-admins. Today the grant form takes a raw UUID, which nobody can produce without dev tools. | UX gap. Bug 2 unlocked the delegated-admin path; the next step is making it usable. |
| 4 | **Stricter unique-token rate limiting on `/api/dl/{token}`.** The endpoint is public, the token space is 256-bit so brute force is infeasible, but there's no per-IP rate limit on the 404 path. A scraper could fingerprint valid tokens via timing. | Defense-in-depth. The current rate-limit middleware is keyed on auth identity, which is absent on the public download path. |
