# TASMail Cross-Feature Assessment Index

> **Parent epic:** [TMAIL-241 — Modularisation, static-types, performance & scalability assessment (cross-feature)](https://cim.techatscale.io/projects/TMAIL/issues/241)
> **Cycle:** May 2026 (`-2026-05.md` suffix)
> **Status:** in progress — 19 of 22 child assessments landed; 3 still queued.

This index points at every per-feature assessment produced under TMAIL-241. Each
report is a point-in-time audit at HEAD around late-May 2026 against four axes:

1. **Modularisation** — split files >250 LOC with >2 responsibilities, lift
   hard-coded lists/enums into registries, separate data layer from presentation.
2. **Static types** — Rust: kill `unwrap`/`as` casts in hot paths, prefer
   newtypes, narrow `serde_json::Value`. TypeScript: tighten `any`/`unknown`,
   add discriminated unions, ensure API response types match Rust models 1:1.
3. **Performance** — index FKs + filter columns, paginate list endpoints, kill
   N+1, stream large responses, cache hot reads with explicit invalidation,
   replace blocking I/O in handlers with async.
4. **Scalability** — registry-driven config over hardcoded constants,
   queue-backed async work over inline, bounded queues, connection-pool hygiene,
   data models that survive 10×–100× growth without destructive migration.

Findings whose fix doesn't fit in the report's accompanying commit are filed as
follow-up child tasks (recorded in the per-report **Follow-up tasks** section).

---

## Backend feature areas

| # | Ticket | Area | Report | Status |
|---|--------|------|--------|--------|
| 1 | TMAIL-242 | Auth & Identity (login, 2FA, SAML, OIDC, LDAP, WebAuthn) | [auth-2026-05.md](auth-2026-05.md) | In Review |
| 2 | TMAIL-243 | Folders & Messages (IMAP read/list/move/flag/delete) | [folders-messages-2026-05.md](folders-messages-2026-05.md) | In Review |
| 3 | TMAIL-244 | Compose, Drafts, Send & Scheduled Send | [compose-send-2026-05.md](compose-send-2026-05.md) | In Review |
| 4 | TMAIL-245 | Search, Filters & Templates | _pending_ | Backlog |
| 5 | TMAIL-246 | Attachments, Shared Files & Quota | [attachments-storage-2026-05.md](attachments-storage-2026-05.md) | In Review |
| 6 | TMAIL-247 | Calendar & Contacts | _pending_ | Backlog |
| 7 | TMAIL-248 | Phishing, DLP & Anti-Spam scanners | [security-scanners-2026-05.md](security-scanners-2026-05.md) | In Review |
| 8 | TMAIL-249 | AI subsystem (Ollama, embeddings, BYOK AI, smart reply, summarization) | [ai-subsystem-2026-05.md](ai-subsystem-2026-05.md) | In Review |
| 9 | TMAIL-250 | Billing & Payment Providers (Paystack / Mastercard / Cybersource / Bank Transfer) | [billing-2026-05.md](billing-2026-05.md) | In Review |
| 10 | TMAIL-251 | Admin surface (domains, users, audit, branding, retention, custom hostnames) | [admin-surface-2026-05.md](admin-surface-2026-05.md) | In Review |
| 11 | TMAIL-252 | Migration imports (IMAP, MBOX, PST) | [migration-imports-2026-05.md](migration-imports-2026-05.md) | In Review |
| 12 | TMAIL-253 | Mobile API, offline sync & push notifications | [mobile-sync-push-2026-05.md](mobile-sync-push-2026-05.md) | In Review |

## Frontend feature areas

| # | Ticket | Area | Report | Status |
|---|--------|------|--------|--------|
| 13 | TMAIL-254 | SPA frontend (api client, hooks, stores, components, settings managers) | [spa-frontend-2026-05.md](spa-frontend-2026-05.md) | In Review |
| 14 | TMAIL-255 | Alt-UI shadcn theme at `/modern/` | [alt-ui-2026-05.md](alt-ui-2026-05.md) | In Review |
| 15 | TMAIL-256 | Migration imports (IMAP, MBOX, PST) — _duplicate of TMAIL-252_ | [migration-imports-2026-05.md](migration-imports-2026-05.md) | In Review |
| 16 | TMAIL-257 | Frontend types & API contract parity (Rust ↔ TS ↔ Dart) — _duplicate of TMAIL-262_ | [frontend-types-parity-2026-05.md](frontend-types-parity-2026-05.md) | In Review |
| 17 | TMAIL-258 | Frontend state management (Zustand stores, TanStack Query, derived state) | [frontend-state-2026-05.md](frontend-state-2026-05.md) | In Review |
| 18 | TMAIL-259 | Frontend bundle size, code splitting & lazy loading | [frontend-bundle-2026-05.md](frontend-bundle-2026-05.md) | In Review |
| 19 | TMAIL-260 | Frontend accessibility (WCAG 2.1 AA, keyboard nav, ARIA, focus management) | [frontend-a11y-2026-05.md](frontend-a11y-2026-05.md) | In Review |
| 20 | TMAIL-261 | Frontend PWA, offline cache & service worker | [frontend-pwa-offline-2026-05.md](frontend-pwa-offline-2026-05.md) | In Review |
| 21 | TMAIL-262 | Frontend types & API contract parity (Rust ↔ TS ↔ Dart) | [frontend-types-parity-2026-05.md](frontend-types-parity-2026-05.md) (also linked from pointer stub [types-contract-parity-2026-05.md](types-contract-parity-2026-05.md) under the filename quoted in the ticket body) | In Review |
| 22 | TMAIL-263 | Frontend rendering performance (virtualisation, memo, profiler) | [frontend-render-perf-2026-05.md](frontend-render-perf-2026-05.md) | In Review |

---

## Coverage gaps & duplicates

The first sweep raised the children one-by-one as the auto-fix queue drained;
two pairs of duplicate tickets need parent-level consolidation:

- **Migration imports** — TMAIL-252 (backend axis) and TMAIL-256 (frontend axis)
  share a single landed report at `migration-imports-2026-05.md`. Close one as a
  duplicate of the other when triaging.
- **Frontend types & API contract parity** — TMAIL-257 and TMAIL-262. Single
  report [frontend-types-parity-2026-05.md](frontend-types-parity-2026-05.md)
  now landed; close one of the two tickets as a duplicate of the other.

The three genuinely missing backend reports are queued (priority=low) and will be
picked up by the auto-fix queue in order. TMAIL-244 landed under the
ticket-named filename `compose-send-2026-05.md` rather than the earlier
placeholder `compose-drafts-2026-05.md`; the assessment surfaced a P0
production-breaking send-path bug (`EmailScheduler` uses a hardcoded
`"placeholder"` SMTP password — every SPA-initiated send silently fails after
the 10s undo window), see §1 of that report.

- TMAIL-244 → [compose-send-2026-05.md](compose-send-2026-05.md) ✅
- TMAIL-245 → `search-filters-templates-2026-05.md`
- TMAIL-246 → [attachments-storage-2026-05.md](attachments-storage-2026-05.md) ✅ (landed
  under the ticket-named filename `attachments-storage-2026-05.md` rather than the earlier
  placeholder `attachments-quota-2026-05.md`)
- TMAIL-247 → `calendar-contacts-2026-05.md`

---

## How to read each report

Every assessment follows roughly the same shape:

1. **Header** — ticket, date, scope (files), method.
2. **TL;DR** — biggest wins ranked by ROI.
3. **Findings by axis** — modularisation, static types, performance, scalability.
4. **Severity** — `P0` (ship-blocker), `P1` (load-bearing pre-GA), `P2` (cleanup).
5. **Follow-up tasks** — child tickets raised for refactors that didn't fit in
   the accompanying commit. Cross-reference these against the PM module.

The accompanying commit(s) for each report carry the `TMAIL-NNN` identifier in
their subject — `git log --grep TMAIL-242` for the auth audit fixes, etc.

---

## Acceptance for the parent epic

TMAIL-241 transitions to Done when:

- [x] Every feature area has a child ticket raised.
- [x] This index exists and points at every report (placeholder rows for the
      five still-pending areas, which the queue workers will fill in).
- [ ] All 22 children are Done (18 in review, 3 still pending — see above).
- [ ] All follow-up child tickets raised by the per-area reports are at least
      triaged (Done or explicitly deferred with a target cycle).

Until the remaining backlog items land their reports, the epic stays in Backlog
per its priority=low ordering.
