# TMAIL-286 — E2E sweep: contacts, groups, signatures, templates, Sieve filters

- **Issue:** TMAIL-286 (sibling of the TMAIL-281 / 282 / 283 / 284 / 285 settings sweeps)
- **Date:** 2026-05-29
- **Spec:** [`frontend/e2e/contacts-templates-filters.spec.ts`](../../frontend/e2e/contacts-templates-filters.spec.ts)
- **Screenshots:** [`frontend/e2e/screenshots/contacts-templates/`](../../frontend/e2e/screenshots/contacts-templates/) — 17 PNGs covering editor / list / preview / sandbox states for every surface.
- **Target:** Live `https://mail.techatscale.io` (workstation backend on `127.0.0.1:3300` reverse-tunnelled through `140.82.32.141:9601`).
- **Browser:** Firefox (per the E2E HARD RULE).
- **Workers:** 1 (`mode: 'serial'` — the mutation specs share one BYOK signup).

---

## TL;DR

All 5 tests pass on a clean run after the **four bug fixes** this commit ships.
The sweep proves the five core settings managers — Signatures, Contacts,
Groups, Templates, Sieve Filters — round-trip through the SPA, backend, and
the database when navigated through the sidebar.

| # | Surface | Outcome | Bug fix |
|---|---------|---------|---------|
| 1 | Signatures: create default → list badge → API GET → delete | ✅ Pass | — |
| 2 | Contacts: create → list → search filter → edit → delete | ✅ Pass | — |
| 3 | Groups: create (no `domain_id`) → expand → add member → API GET | ⚠️ → ✅ | **Bug 1** (below) |
| 4 | Templates: menu nav → editor → merge fields → render preview | ⚠️ → ✅ | **Bug 2** + **Bug 3** |
| 5 | Filters: create → "Active" badge → match-test sandbox → API verdict | ⚠️ → ✅ | **Bug 4** + new endpoint |

---

## 1. Bug 1 — `frontend/src/api/filters.ts` was hitting `/api/api/filters`

The API client at `frontend/src/api/client.ts:1,27` prepends
`API_BASE_URL` (`"/api"`, see `frontend/src/utils/constants.ts:1`) to every
request path. Every other API module in `frontend/src/api/` uses a *relative*
path (`/contacts`, `/signatures`, `/groups`, `/templates`, …). The filters
module, however, hardcoded `/api/filters` as the base, so the actual fetch
URL became `/api/api/filters` — a 404 on every CRUD call. Result: **the
entire Filters surface was non-functional in production**, but our unit tests
mocked `apiClient` so the bug never surfaced.

**Fix:** strip the redundant `/api` prefix.
[`frontend/src/api/filters.ts:48–65`](../../frontend/src/api/filters.ts).

**Verification:** the new `filters: create rule, Active badge, match-test
verdict — happy path` spec lists rules via the live API, creates one through
the UI, and asserts that the GET round-trip returns the new rule with
`enabled=true`. The screenshot `filter-active-badge.png` shows the saved
rule rendered with the new green "Active" badge.

---

## 2. Bug 2 — `TemplateManager` was completely unreachable in the SPA

`frontend/src/components/settings/TemplateManager.tsx` has existed since
TMAIL-94, but three pieces of wiring were missing:

| File | Missing wiring |
|------|----------------|
| `frontend/src/stores/mailStore.ts:34` | `'templates'` not in the `ViewMode` union → `setViewMode('templates')` was a TypeScript error |
| `frontend/src/components/layout/Sidebar.tsx` | No menu button for Templates → no way to navigate there |
| `frontend/src/components/layout/AppShell.tsx` | No `viewMode === 'templates'` branch in the Suspense block → nothing to render even if you could set the mode |

Net effect: **the entire Email Templates feature was orphaned code** in the
production SPA. Users could only reach it by setting `viewMode` from the
React DevTools.

**Fix:** wire all three pieces — extend the `ViewMode` union, add a `FileText`
sidebar entry next to Filters, add a lazy import + branch in AppShell.
[`frontend/src/stores/mailStore.ts:34`](../../frontend/src/stores/mailStore.ts),
[`frontend/src/components/layout/Sidebar.tsx:30,113`](../../frontend/src/components/layout/Sidebar.tsx),
[`frontend/src/components/layout/AppShell.tsx:32,125`](../../frontend/src/components/layout/AppShell.tsx).

**Verification:** the `templates` spec uses our standard `navigateToSettings`
helper which clicks the sidebar item by label. If the sidebar entry isn't
there, the test fails at the navigation step. The screenshot
`template-preview-rendered.png` shows the merge-field render: input
`{name: "Ama", company: "TASMail"}` → output `Hi Ama, welcome to TASMail.`.

---

## 3. Bug 3 — `GroupManager` was submitting an invalid `domain_id`

`GroupManager.handleCreate` was sending `domain_id: ''` because the SPA had
no way to look up the user's domain — `/api/admin/domains` is admin-only and
non-admin BYOK users couldn't enumerate domains they belonged to. The backend
deserialiser then rejected the empty string as an invalid Uuid before the
handler even ran, so **every group create attempt from the UI failed**.

The fix has two parts:

1. **Backend** — `CreateGroupRequest.domain_id` becomes
   `Option<Uuid>`. When omitted, the handler resolves the owner's
   mailbox.domain_id and uses that. This matches single-domain BYOK
   reality (the only domain a non-admin user can create groups in) and
   leaves admins on multi-domain deployments free to pin one explicitly.
   [`backend/src/models/distribution_group.rs:33–40`](../../backend/src/models/distribution_group.rs),
   [`backend/src/handlers/groups.rs:36–66`](../../backend/src/handlers/groups.rs).
2. **Frontend** — stop sending the empty string.
   [`frontend/src/components/settings/GroupManager.tsx:33–46`](../../frontend/src/components/settings/GroupManager.tsx),
   [`frontend/src/types/groups.ts:22–30`](../../frontend/src/types/groups.ts).

Backend unit tests cover three corners of the deserialiser:
`test_create_group_request_deserialization` (domain pinned),
`test_create_group_request_without_domain_id` (omitted → None) and
`test_create_group_request_with_empty_string_domain_id_rejected` (so
the SPA can't accidentally regress to sending `''` and silently passing).

**Verification:** the `groups` spec creates a group through the UI without
ever specifying a domain. The screenshot `group-list-after-create.png`
shows the row, `group-with-member.png` shows an added member, and the API
cross-check confirms both records via `/api/groups` + `/api/groups/{id}/members`.

---

## 4. Bug 4 / new endpoint — Sieve match-test sandbox

The PM description for TMAIL-286 explicitly asked for a "match-test result"
screenshot, but no `POST /api/filters/{id}/test`-style endpoint existed —
users had to enable a rule against real mail to see whether it would actually
match.

**Backend (new):** added
`POST /api/filters/{id}/test`. The handler loads the rule, runs the new
`SieveRule::evaluate_sample(&SampleMessage)` method, and returns a per-condition
breakdown with the final `matched` verdict + the `match_mode` used. The
evaluator re-uses the existing `evaluate_condition` helper, so the sandbox
and the production matcher cannot drift apart.
[`backend/src/models/sieve_rule.rs:64–92,231–280`](../../backend/src/models/sieve_rule.rs),
[`backend/src/handlers/sieve.rs:118–135`](../../backend/src/handlers/sieve.rs),
[`backend/src/router.rs:342`](../../backend/src/router.rs).

**Frontend (new):** added a flask-icon button on every rule row and an
inline sandbox panel above the list. The sandbox takes `from / subject / body`
inputs, calls the new endpoint, and renders a verdict with a per-condition
breakdown.
[`frontend/src/api/filters.ts:67–105`](../../frontend/src/api/filters.ts),
[`frontend/src/components/settings/FilterManager.tsx:241–306,442–522`](../../frontend/src/components/settings/FilterManager.tsx).

Six new backend unit tests cover the evaluator (`test_evaluate_sample_*` in
`sieve_rule.rs`): ALL mode all-match, ALL mode partial-fail, ANY mode
one-match, body field, missing field treated as empty, empty conditions
never match. All six pass.

**Verification:** the `filters` spec runs the positive path
(`from = newsletter@store.com` against a `from contains newsletter` rule →
"Would match") and the negative path (`from = friend@ok.com` → "Would not
match") through the UI, AND cross-checks both verdicts via a direct API hit
to `/api/filters/{id}/test`. Screenshots `filter-test-result-match.png` and
`filter-test-result-nomatch.png` document the two outcomes.

---

## What each spec asserts

### 1) Signatures
- Pre-condition API GET of `/api/signatures`.
- Click sidebar **Signatures** → click **New Signature** → fill name + HTML
  + text + `Default` checkbox → click **Save**.
- DOM assertions: row visible, "Default" badge visible.
- API GET cross-check: new signature exists, `is_default === true`,
  `text_body` contains "Best regards".
- API DELETE + final GET to confirm deletion.

### 2) Contacts
- Pre-condition API GET of `/api/contacts`.
- Click sidebar **Contacts** → **Add Contact** → fill email/name/company →
  **Save**.
- DOM: row visible.
- Search filter: typing `RUN_TAG` narrows the visible list to just the new row.
- API GET: contacts list grew by exactly 1; new row matches submitted values.
- API DELETE + final GET to confirm.

### 3) Groups (validates Bug 3 fix end-to-end)
- Pre-condition API GET of `/api/groups`.
- Click sidebar **Groups** → **New Group** → fill name + address +
  description → **Create Group** (no `domain_id` field exists in the form
  by design).
- DOM: group row visible.
- API GET: group exists; backend resolved a real `domain_id` (FK didn't fail).
- Expand the group → fill **Add member email…** → submit.
- API GET `/api/groups/{id}/members` confirms the member is persisted.
- API DELETE of the group cascades to members.

### 4) Templates (validates Bug 2 fix end-to-end)
- Pre-condition API GET of `/api/templates`.
- Click sidebar **Templates** (the menu entry must exist!) → **New Template**
  → fill all fields with merge syntax `{{name}}` and `{{company}}` → **Create**.
- DOM: row visible with "2 merge fields" hint.
- API GET cross-check: merge_fields is `['company', 'name']` (sorted).
- Open preview (Eye icon) → fill the merge inputs `name=Ama, company=TASMail`
  → **Render Preview**.
- DOM: `[data-testid="preview-output"]` contains the rendered text body
  `"Hi Ama, welcome to TASMail."`.
- API DELETE.

### 5) Filters (validates Bug 1 + Bug 4 fixes end-to-end)
- Pre-condition API GET of `/api/filters`.
- Click sidebar **Filters** → **New Filter** → fill rule name + condition
  (`from contains newsletter`) + action (`move → Newsletters`) → **Create Rule**.
- DOM: row visible, "Active" badge visible.
- API GET cross-check: rule exists, `enabled === true`,
  `conditions[0].value === "newsletter"`.
- Click flask icon → sandbox opens.
- Positive sample (`from=newsletter@store.com`) → click **Test Match** →
  DOM verdict reads "Would match"; direct API POST to
  `/api/filters/{id}/test` returns `matched: true`.
- Negative sample (`from=friend@ok.com`) → DOM verdict reads "Would not match";
  API POST returns `matched: false`.
- API DELETE.

---

## Run instructions

```bash
# 1. Build the backend release binary + restart the systemd unit:
cd backend && cargo build --release
systemctl --user restart tasmail-backend.service

# 2. Run the spec on Firefox against the live tunnel (default baseURL).
#    Uses 1 worker because the suite shares a single BYOK signup.
cd frontend && npx playwright test contacts-templates-filters.spec.ts --project=firefox

# 3. (Optional) Open the HTML report.
npx playwright show-report
```

Override the backend URL for local-only runs:
```bash
PLAYWRIGHT_BASE_URL=http://localhost:5273 npx playwright test contacts-templates-filters.spec.ts --project=firefox
```

The `afterAll` hook deletes the test mailbox via `psql` so re-runs start
clean — see `frontend/e2e/helpers/db-cleanup.ts`. Make sure `psql` is
available and `TASMAIL_DB_URL` is reachable.

---

## Files changed in this commit

| File | Change | Why |
|------|--------|-----|
| `frontend/src/api/filters.ts` | Strip redundant `/api` prefix; add `testFilter()` + types | Bug 1 + new endpoint client |
| `frontend/src/stores/mailStore.ts` | Add `'templates'` to `ViewMode` union | Bug 2 |
| `frontend/src/components/layout/Sidebar.tsx` | Add Templates entry with FileText icon | Bug 2 |
| `frontend/src/components/layout/AppShell.tsx` | Add lazy import + Suspense branch for TemplateManager | Bug 2 |
| `frontend/src/components/settings/FilterManager.tsx` | Add "Active" badge, flask test button, sandbox panel | Match-test UX + Bug 1 verification |
| `frontend/src/components/settings/GroupManager.tsx` | Stop sending `domain_id: ''` | Bug 3 |
| `frontend/src/types/groups.ts` | Make `domain_id` optional on the request type | Bug 3 |
| `backend/src/models/distribution_group.rs` | `domain_id: Option<Uuid>`; threading through `create()` | Bug 3 |
| `backend/src/handlers/groups.rs` | Resolve fallback `domain_id` from owner's mailbox | Bug 3 |
| `backend/src/models/sieve_rule.rs` | New `SampleMessage` / `RuleMatchBreakdown` / `evaluate_sample` | Match-test endpoint |
| `backend/src/handlers/sieve.rs` | New `test_rule` handler | Match-test endpoint |
| `backend/src/router.rs` | Register `POST /api/filters/{id}/test` | Match-test endpoint |
| `frontend/e2e/contacts-templates-filters.spec.ts` | NEW — the sweep itself | The deliverable |
| `frontend/e2e/screenshots/contacts-templates/*.png` | NEW — 17 screenshots | Audit trail |
| `docs/assessments/e2e-contacts-templates-2026-05.md` | NEW — this doc | Discoverability |

---

## Follow-up tickets (suggested)

| # | Suggestion | Why |
|---|------------|-----|
| 1 | **Add a `body` field to `matches_email`** — currently the production matcher's `field == "body"` branch returns `""` because the IMAP pipeline doesn't hand the body bytes to `evaluate_condition`. The sandbox evaluator does honour body. Production needs to catch up so the two paths cannot diverge. | The PM description for TMAIL-286 expected a body-aware match-test result. We deliver it for the sandbox but the live IMAP pipeline still ignores body conditions. |
| 2 | **Surface `/api/admin/domains` as a non-admin "my domains" endpoint** so admins on multi-domain deployments can still pin a `domain_id` from the GroupManager UI. Today the fallback works, but it forces the group into the user's primary domain even when they belong to multiple. | Bug 3 fix only addresses the single-domain BYOK case. Multi-tenant admins should get a domain picker. |
| 3 | **Add a `priority` reorder E2E** — `reorderFilters` is exposed in the API client and there are arrow-up/down buttons in `FilterManager`, but no spec exercises the reorder. With Bug 1 fixed, reorder is now reachable; coverage should follow. | Coverage gap, low risk but easy to add. |
