# Admin Surface Assessment — May 2026

**Ticket:** TMAIL-251 (axis of TMAIL-241 backend modularisation review)
**Scope:** `/api/admin/*` backend handlers + `frontend/src/components/admin/`
shell and manager components. Includes the audit log writer, retention
sweeper, branding cache, bulk user CSV import (TMAIL-202), IP warm-up
admin (TMAIL-203), custom hostnames (TMAIL-112), and the admin sidebar
registry (TMAIL-197).
**Method:** static read of every handler file under `backend/src/handlers/admin/`,
plus the singleton admin-facing handlers (`branding.rs`, `retention.rs`,
`custom_hostnames.rs`, `bulk_import.rs`, `activesync.rs`, `warmup.rs`,
`cache.rs`, `dlp.rs`, `dane.rs`, `archive.rs`, `ollama.rs`, `ediscovery.rs`),
the route registration in `router.rs`, the auth middleware, every component
in `frontend/src/components/admin/`, and a sample of operator-scoped
managers in `frontend/src/components/settings/`.

---

## TL;DR

The admin surface is functionally complete and largely well-structured —
TanStack Query is used uniformly across all 8 admin managers, the sidebar
is registry-driven (TMAIL-197), the branding cache works correctly under
Redis with explicit on-update invalidation, and the bulk-user-import flow
(TMAIL-202) and IP warm-up panels (TMAIL-203) are wired end-to-end.

There are **two P0 gaps** worth fixing soon:

1. **Three ActiveSync admin endpoints discard `claims` and skip the
   `is_admin` gate.** Any authenticated user can list, create, and update
   ActiveSync device policies. The convention everywhere else is
   per-handler `auth_service::require_admin(&claims)?;` — but it relies on
   each handler remembering to call it. ActiveSync forgot.
2. **The retention sweeper does not exist.** The retention policy CRUD UI
   and API are fully wired, but no background task ever enforces them. No
   email is ever actually purged by a policy.

A third, structural finding underpins both: **`/api/admin/*` routes are
flat `.route()` calls inside the main `protected_routes` blob with no
admin sub-router**, so the `is_admin` gate cannot be applied at the router
layer. Extracting an admin sub-router with a shared middleware would catch
future gaps structurally rather than relying on every handler author
remembering to call `require_admin`.

Beyond those, the audit log writer is inline (acceptable today, worth
revisiting at scale), bulk CSV import has no upload size limit, and the
operator-scoped settings managers (Branding, Retention, Hostnames,
LDAP/SAML/OIDC, eDiscovery, DLP) sit in `components/settings/` rather
than `components/admin/`, gated by the backend 403 only — a UX gap
rather than a security one but architecturally inconsistent.

---

## What was checked

| Axis | Result |
|---|---|
| `claims.is_admin` enforced on every `/api/admin/*` route | ⚠️ 3 endpoints escape (ActiveSync) |
| Audit log writes: inline vs async queue | ⚠️ Inline (MEDIUM risk at scale) |
| Retention sweeper: scheduled + batched | ❌ Does not exist |
| Branding cache: tenant-keyed + invalidated on update | ✅ Redis, single key, explicit invalidate |
| Bulk user CSV import: streaming vs load-all | ⚠️ Load-all, no upload limit |
| Custom hostnames SNI lookup O(1) | ➖ Not yet implemented at web layer |
| Admin sidebar registry-driven (TMAIL-197) | ✅ NAV array in `AdminShell.tsx` |
| RequireAdmin frontend gate | ⚠️ No token expiry check |
| Admin managers use TanStack Query | ✅ All 8 |
| Pagination on list views | ⚠️ Mixed — users/domains/warmup load all |
| Mutation invalidation on create/update/delete | ✅ Every manager |
| Component size <250 lines | ✅ 8/9 (QuoteRequestsManager 255) |

---

## Backend findings

### 1. Admin auth gate (`claims.is_admin`) — PARTIAL

The protected route blob receives a single `auth_middleware` layer at
`backend/src/router.rs:1019-1022`:

```rust
.layer(axum_middleware::from_fn_with_state(
    state.clone(),
    auth_middleware,
));
```

That layer validates the JWT and injects `Claims` — it does **not** check
`is_admin`. Every admin handler must call `auth_service::require_admin(&claims)?;`
itself. Almost every handler does. Three do not:

| Endpoint | Handler | Evidence |
|---|---|---|
| `GET /api/admin/activesync/policies` | `activesync::list_policies` | `activesync.rs:124-129` |
| `POST /api/admin/activesync/policies` | `activesync::create_policy` | `activesync.rs:134-157` |
| `PUT /api/admin/activesync/policies/{id}` | `activesync::update_policy` | `activesync.rs:161-207` |

All three use `axum::Extension(_claims): axum::Extension<Claims>` —
underscore-prefixed, claims discarded — with no `require_admin` call.

**Confirmed-gated handlers** (sampled): `domains.rs`, `users.rs`,
`audit.rs`, `feature_flags.rs`, `payment_providers.rs`, `quote_requests.rs`,
`queue.rs`, `branding.rs`, `retention.rs`, `custom_hostnames.rs`,
`bulk_import.rs`, `warmup.rs`, `cache.rs`, `dlp.rs`, `dane.rs`,
`archive.rs`, `ollama.rs`. `ediscovery.rs` uses `require_compliance`
(admits `is_admin || is_compliance_officer`) — intentional but undocumented.

### 2. Audit log writes — INLINE

`models/audit_log.rs:39-54`:

```rust
pub async fn record(pool: &PgPool, ...) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT set_config('app.is_admin', 'true', false)")
        .execute(&mut *conn).await?;
    sqlx::query("INSERT INTO audit_log ...")
        .execute(&mut *conn).await?;
    Ok(())
}
```

Two sequential DB round-trips (SET + INSERT) per audited request,
synchronously inside the handler. Call sites use `let _ = AuditLog::record(...).await;`
which silently swallows the error but still blocks until the INSERT
completes. No background task, channel, or `tokio::spawn` exists for
audit writes.

**Risk:** MEDIUM. Negligible at current load. Becomes problematic if a
high-frequency code path (bulk import row-by-row, state-change sweep)
starts auditing every step.

### 3. Retention sweeper — DOES NOT EXIST

`main.rs` spawns exactly three background services:

```rust
services::email_scheduler::EmailScheduler        // main.rs:71
services::queue_processor::QueueProcessor        // main.rs:89
services::billing_rollup::BillingRollup          // main.rs:105
```

`models/retention_policy.rs` contains only CRUD methods (`find_all`,
`find_by_id`, `create`, `update`, `delete`) — no enforcement loop.
`LegalHold::find_active_for_user` exists but is never called from any
scheduled task.

**Risk:** HIGH. The retention policy UI and API
(`GET/POST/PUT/DELETE /api/admin/retention`) are fully wired, but the
policies are pure configuration — they never delete anything.

### 4. Branding cache — OK

Redis-backed via `services/cache_service.rs`. Single global key
`tasmail:branding` with configurable TTL (default 300 s).

`handlers/branding.rs:17-19` (read path):

```rust
if let Some(cached) = state.cache.get_branding::<Branding>().await {
    return Ok(Json(cached));
}
```

`handlers/branding.rs:43` (write path explicitly invalidates):

```rust
state.cache.invalidate_branding().await;
```

`POST /api/admin/branding/reset` also invalidates. Redis-unavailable
degrades to a DB hit (correct).

**Caveat:** the cache is **not per-tenant** — a single key for the entire
instance. Acceptable for the current single-tenant deployment; will need
a `tasmail:branding:{tenant_id}` key shape if per-domain branding ever
ships.

### 5. Bulk user CSV import — LOAD-ALL, NO LIMIT

`handlers/bulk_import.rs:58-61`:

```rust
let data = field
    .bytes()
    .await
    .map_err(...)?;
file_data = Some(data.to_vec());
```

`field.bytes()` buffers the entire multipart body into a single `Bytes`
allocation, then parses the full string in one pass via
`csv_processor::parse_csv(&csv_content)`. No streaming, no
`ContentLengthLimit`, no `DefaultBodyLimit`, no early size check.

**Risk:** MEDIUM. Admin-only, so the attack surface is the admin token
itself. A misconfigured or compromised admin could OOM the process with
a multi-GB upload. A 50 MB cap covers ~500 k users with headroom.

### 6. Custom hostnames SNI — NOT WIRED AT WEB LAYER

`models/custom_hostname.rs:9` (header comment):

> `EXTERNAL: Actual SNI routing is handled by Postfix/Dovecot — this is the management layer`

`find_all` does `SELECT * FROM custom_hostnames ORDER BY created_at DESC`
— no indexed `find_by_hostname(name)`, no HashMap in `AppState`, no
Rustls `ResolvesServerCert` integration in `main.rs` or `router.rs`.

**Risk:** LOW now (feature stub). MEDIUM the day the web-layer SNI
resolver ships, **if** it reuses the `find_all` + linear scan pattern —
every TLS handshake would do a full table scan. The right shape is an
in-memory `Arc<RwLock<HashMap<String, CertConfig>>>` in `AppState`
rebuilt on every `PUT/POST /api/admin/hostnames`.

### 7. Admin route registration — SCATTERED

All admin routes are flat `.route()` calls interleaved with user routes
inside a single `protected_routes` Router. Example `router.rs:168-176`:

```rust
.route(
    "/api/admin/domains",
    get(handlers::admin::domains::list_domains).post(...),
)
.route(
    "/api/admin/domains/{id}",
    delete(handlers::admin::domains::delete_domain),
)
```

There is no `Router::new()` admin sub-router, no `admin_only_middleware`
layer that wraps only `/api/admin/*`. The final `auth_middleware` layer
wraps the whole `protected_routes` blob, not the admin subset.

**Consequence:** the `is_admin` check is convention rather than
structure. Every new admin endpoint needs the author to remember
`require_admin`. The ActiveSync gap in finding 1 is the direct result.

### 8. Other backend findings

- **`GET /api/admin/users` — unbounded list, no pagination**
  (`users.rs:24`): `SELECT * FROM mailboxes ORDER BY username` with
  `fetch_all`. `export_users_csv` (`bulk_import.rs:320`) has the same
  pattern — intentional for CSV, but document it.
- **ActiveSync `update_policy` — O(n) fetch to get one record**
  (`activesync.rs:168-172`): fetches all policies, then Rust-side
  `.find(|p| p.id == id)`. Should be `SELECT ... WHERE id = $1`.
- **Audit log: duplicate `build_query` and `build_filtered_query`**
  (`audit_log.rs:65-91` and `128-145`). The `build_query` variant
  (used only in tests) does not support the prefix-match `LIKE` filter
  from `build_filtered_query`. Divergence risk.
- **Audit log: no `created_at` index pressure guard** — `GET /api/admin/audit-log`
  without a mailbox_id filter runs `SELECT * FROM audit_log WHERE 1=1
  ORDER BY created_at DESC LIMIT 500`. Confirm migrations include
  `CREATE INDEX ... ON audit_log (created_at DESC)`.
- **eDiscovery uses `require_compliance`** — intentional but undocumented.
  Compliance officers (non-admin) can access `/api/admin/ediscovery/*`.

---

## Frontend findings

### 1. Sidebar nav (TMAIL-197) — REGISTRY-DRIVEN

`frontend/src/components/admin/AdminShell.tsx:26-35`:

```ts
const NAV: NavEntry[] = [
  { to: '/admin/feature-flags', label: 'Feature flags', icon: <ToggleRight size={18} /> },
  { to: '/admin/quote-requests', label: 'Quote requests', icon: <Inbox size={18} /> },
  { to: '/admin/audit-log', label: 'Audit log', icon: <ScrollText size={18} /> },
  { to: '/admin/cache', label: 'Cache', icon: <Database size={18} /> },
  { to: '/admin/domains', label: 'Domains', icon: <Globe size={18} /> },
  { to: '/admin/payment-providers', label: 'Payment providers', icon: <CreditCard size={18} /> },
  { to: '/admin/users', label: 'Users', icon: <Users size={18} /> },
  { to: '/admin/warmup', label: 'IP warm-up', icon: <Activity size={18} /> },
];
```

Rendered with one `.map()` at line 48. Adding a new section = one array
entry + one `<Route>` in `App.tsx:144-153`. Correct pattern.

**Minor:** icons are pre-instantiated JSX elements rather than component
references, which allocates elements at module parse time. Cosmetic.

### 2. RequireAdmin gate — UX-ONLY, MISSING EXP CHECK

`frontend/src/components/admin/RequireAdmin.tsx:17-30`:

```ts
function decodeClaims(token: string): JwtClaims | null {
  try {
    const payload = token.split('.')[1];
    return JSON.parse(atob(payload)) as JwtClaims;
  } catch { return null; }
}
// ...
const token = localStorage.getItem('access_token');
if (!token) return <Navigate to="/login" replace />;
const claims = decodeClaims(token);
if (!claims?.is_admin) { /* render blocked UI */ }
```

The header comment documents intent: "the backend re-verifies on every
request, so this is purely a UX gate." No `/api/auth/me` call, no
signature check (correct — the backend is authoritative). The gap is
**no `exp` check** — an expired admin token will pass this gate, render
the shell, then 401 on the first API call.

### 3. Data fetching pattern per manager

All 8 admin managers use TanStack Query consistently. No raw
`useEffect+fetch` anywhere in the admin surface.

| Manager | Pattern | Pagination | Mutation invalidation |
|---|---|---|---|
| DomainsManager | `useQuery` | ❌ load all | ✅ `['admin-domains']` |
| UsersManager | `useQuery` | ❌ load all | ✅ `['admin-users']` |
| AuditLogManager | `useQuery` | Server-side `limit` only (25/50/100/250/500) | N/A (read-only) |
| FeatureFlagsManager | `useQuery` + optimistic | ❌ load all | ✅ rollback + `invalidateQueries` in `onSettled` |
| PaymentProvidersManager | `useQuery` | ❌ load all, client-side filter on `!archived` | ✅ `['admin-payment-providers']` |
| QuoteRequestsManager | `useQuery` with hardcoded `limit: 50, offset: 0` | Partial — no "load more" | ✅ `['admin', 'quote-requests']` |
| CacheManager | `useQuery` for status/stats | N/A | ✅ `['admin-cache-stats']` |
| WarmupManager | `useQuery` | ❌ load all tracked IPs | ✅ `['admin-warmup-status']` |

### 4. Component sizes (Modularize rule)

| File | Lines | Status |
|---|---|---|
| RequireAdmin.tsx | 45 | ✅ OK |
| AdminShell.tsx | 89 | ✅ OK |
| FeatureFlagsManager.tsx | 107 | ✅ OK |
| DomainsManager.tsx | 140 | ✅ OK |
| AuditLogManager.tsx | 145 | ✅ OK |
| WarmupManager.tsx | 152 | ✅ OK |
| CacheManager.tsx | 182 | ✅ OK |
| UsersManager.tsx | 185 | ✅ OK |
| PaymentProvidersManager.tsx | 229 | ✅ OK |
| QuoteRequestsManager.tsx | 255 | ⚠️ WARN — embeds `DetailPanel` (lines 177-255) that should be its own file |

### 5. Audit log viewer — server-limit only, no pagination

`AuditLogManager.tsx:26-34` exposes a 25/50/100/250/500 limit selector.
At 500 rows the result set is rendered into a flat DOM table with no
virtualisation (~3000 nodes for 6 columns × 500 rows). Acceptable now,
becomes a P1 once daily audit volume crosses a few thousand.

### 6. Bulk user CSV import UI (TMAIL-202) — WIRED

`UsersManager.tsx:80-93` exposes a file input that posts to
`/api/admin/users/bulk-import` via `adminUsersApi.bulkImport(file)`
(`api/admin-users.ts:48-52`). The result banner shows aggregate
`success_count` / `error_count` / `total_rows` / `status`. **Per-row
error detail is not surfaced** — only the totals. This is the main
usability gap.

### 7. IP warm-up UI (TMAIL-203) — WIRED, READ + START ONLY

`WarmupManager.tsx` exposes three panels: tracked-IP table with current
day/week, daily limit, progress bar, total sent, started date, and state;
a start-tracking form; and the 8-week schedule reference table.

`WarmupStatus` has a `paused` boolean but no UI to toggle it. No
pause/resume/remove actions — only start.

### 8. CacheManager.tsx — operational dashboard

`GET /api/admin/cache/status` (connection flag + TTL config),
`GET /api/admin/cache/stats` (raw `redis INFO`), `POST /api/admin/cache/flush`
(two-step confirm). Flush is disabled when Redis is unreachable
(`CacheManager.tsx:127`).

### 9. "30+ managers" — clarification

The CLAUDE.md "30+ manager components" count is correct, but most of them
do **not** live in `components/admin/`. They are in
`components/settings/` and are surfaced via the user-facing AppShell
`viewMode` switcher (`AppShell.tsx:107-151`), not via the `/admin/*`
route tree. The `components/admin/` folder contains exactly 9 files (8
managers + AdminShell + RequireAdmin).

| Operator-scoped manager | Location | Frontend gate |
|---|---|---|
| BrandingManager | `settings/` | ❌ none — backend 403 only |
| RetentionManager | `settings/` | ❌ none — backend 403 only |
| HostnameManager | `settings/` | ❌ none — backend 403 only |
| LdapManager | `settings/` | ❌ none — backend 403 only |
| SamlManager | `settings/` | ❌ none — backend 403 only |
| OidcManager | `settings/` | ❌ none — backend 403 only |
| EdiscoveryManager | `settings/` | ❌ none — backend 403 only |
| DlpManager | `settings/` | ❌ none — backend 403 only |
| BulkImportManager (settings) | `settings/` | ❌ none — but this is the *email/IMAP* bulk importer, distinct from UsersManager's CSV user-import |

This is the architectural inconsistency: two parallel admin surfaces with
different gating. Non-admin users can navigate into Branding/Retention/
Hostname/LDAP/SAML/OIDC/eDiscovery/DLP forms in the user shell and only
see the failure on submit (backend 403). The backend gate holds, so it's
a UX/discoverability issue rather than a security one — but it shouldn't
stay split.

---

## Recommendations

### P0 — Security / correctness

1. **Fix ActiveSync admin gate** (`activesync.rs:124, 134, 161`). Replace
   `axum::Extension(_claims)` with `axum::Extension(claims)` and add
   `auth_service::require_admin(&claims)?;` as the first line in
   `list_policies`, `create_policy`, and `update_policy`.

2. **Implement the retention sweeper**. Add
   `services/retention_sweeper.rs` as a `tokio::time::interval` task in
   `main.rs`, sibling to `EmailScheduler` / `QueueProcessor` /
   `BillingRollup`. The loop should query `retention_policies`, honour
   `legal_holds`, and issue batched IMAP deletes via `imap_service`
   (`LIMIT 500` per policy per tick). Without this, every retention
   policy is a no-op.

3. **Add an admin sub-router with shared `is_admin` middleware**
   (`router.rs`). Extract `/api/admin/*` into its own `Router::new()`,
   layer an `admin_only_middleware` on top, then merge into
   `protected_routes`. Removes the per-handler `require_admin` call as
   the gate of last resort and prevents future ActiveSync-style gaps
   structurally.

4. **Add `exp` check to RequireAdmin** (`RequireAdmin.tsx:29`). One line
   before the `is_admin` check:

   ```ts
   if (claims.exp * 1000 < Date.now()) return <Navigate to="/login" replace />;
   ```

5. **Either move operator-scoped managers under `AdminShell` or gate
   them in AppShell.** `BrandingManager`, `RetentionManager`,
   `HostnameManager`, `LdapManager`, `SamlManager`, `OidcManager`,
   `EdiscoveryManager`, `DlpManager` currently render for any
   authenticated user. Pick one canonical home — recommend moving them
   to the admin route tree, removing the `viewMode` branches from
   AppShell — so the frontend gate is consistent.

### P1 — Data / scalability

6. **Add upload size limit for bulk CSV** (`router.rs` + `bulk_import.rs`).
   `axum::extract::DefaultBodyLimit::max(52_428_800)` (50 MB) on the
   bulk-import route. Prevents an OOM via a multi-GB upload from a
   compromised admin token.

7. **Fix ActiveSync `update_policy` N+1** (`activesync.rs:168`). Add
   `ActiveSyncPolicy::find_by_id(&state.db, id)` (`SELECT ... WHERE id = $1`)
   instead of fetching all policies and finding in Rust.

8. **Paginate `GET /api/admin/users`** (`users.rs:24`). Add `limit` /
   `offset` query params (default 100, max 1000) matching the pattern in
   `quote_requests.rs`.

9. **Paginate audit log viewer** (`AuditLogManager.tsx`). Replace the
   25/50/100/250/500 dropdown with `?limit=100&offset=N` prev/next
   pagination. Optionally add `@tanstack/react-virtual` for the table
   rows if a single page must show > 100 rows.

10. **Surface per-row errors from bulk CSV import** (`UsersManager.tsx`,
    `BulkImportResult` type). Backend already collects per-row errors —
    echo them in the result payload and render an expandable list under
    the result banner.

### P2 — Hygiene / future-proofing

11. **Switch audit log writes to fire-and-forget background task**.
    `tokio::spawn(async move { let _ = AuditLog::record(...).await; });`
    so audit writes never block the response path. Acceptable to lose a
    tiny window of audit entries on hard crash; if not, use a bounded
    channel + dedicated writer task.

12. **Confirm `audit_log.created_at` index exists in migrations**.
    Without it `GET /api/admin/audit-log` does a full sequential scan as
    the table grows.

13. **Remove duplicate `build_query` function** (`audit_log.rs:128-145`).
    Tests should use `build_filtered_query`. Delete the duplicate to
    prevent divergence.

14. **Pre-load custom hostnames into an `Arc<RwLock<HashMap>>` in
    `AppState`** when the web-layer SNI resolver ships. Do not query the
    DB per TLS handshake.

15. **Split `DetailPanel` out of `QuoteRequestsManager.tsx`** (lines
    177-255) into `QuoteRequestDetailPanel.tsx`. Brings the parent to
    ~175 lines.

16. **Add load-more or proper pagination to
    `QuoteRequestsManager`** — currently hardcoded `limit: 50, offset: 0`,
    so the 51st quote in any status is invisible.

17. **Document the `ediscovery` dual-role gate** (`require_compliance`).
    Add a comment in `router.rs` next to the `/api/admin/ediscovery/*`
    block stating the role policy, and reconcile with `docs/SECURITY.md`.

18. **WarmupManager: add pause / resume / remove actions**. Backend has
    the `paused` flag; UI only exposes start.

19. **Per-tenant branding cache key** (`services/cache_service.rs`).
    Switch `tasmail:branding` to `tasmail:branding:{tenant_id}` ahead of
    multi-tenant branding so the migration is single-line later.

---

## Action item summary

| # | Priority | Area | One-line fix |
|---|---|---|---|
| 1 | P0 | backend/security | Gate ActiveSync admin endpoints with `require_admin` |
| 2 | P0 | backend/correctness | Implement `services/retention_sweeper.rs` background task |
| 3 | P0 | backend/structure | Extract `/api/admin/*` into admin sub-router with shared middleware |
| 4 | P0 | frontend/UX | Add `exp` check to `RequireAdmin` |
| 5 | P0 | frontend/architecture | Move operator-scoped managers under `AdminShell` or gate in AppShell |
| 6 | P1 | backend/scale | 50 MB upload cap on `POST /api/admin/users/bulk-import` |
| 7 | P1 | backend/perf | Fix N+1 in `activesync::update_policy` |
| 8 | P1 | backend/scale | Paginate `GET /api/admin/users` |
| 9 | P1 | frontend/perf | Paginate audit log viewer |
| 10 | P1 | frontend/UX | Surface per-row errors from bulk CSV import |
| 11 | P2 | backend/perf | Move audit log writes to `tokio::spawn` |
| 12 | P2 | backend/perf | Confirm `audit_log.created_at` index |
| 13 | P2 | backend/hygiene | Remove duplicate `build_query` |
| 14 | P2 | backend/perf | Pre-load custom hostnames into `HashMap` (when SNI ships) |
| 15 | P2 | frontend/modularity | Split `DetailPanel` out of `QuoteRequestsManager` |
| 16 | P2 | frontend/UX | Pagination on `QuoteRequestsManager` |
| 17 | P2 | docs | Document `ediscovery` dual-role gate |
| 18 | P2 | frontend/UX | Warmup pause/resume/remove actions |
| 19 | P2 | backend/multi-tenant | Per-tenant branding cache key |

These should be filed as scoped TMAIL tasks against this assessment.
