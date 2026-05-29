# Types & Contract Parity (Rust ↔ TS ↔ Dart) — Pointer

> **Tickets:** TMAIL-262 (and its duplicate, TMAIL-257)
> **Parent epic:** TMAIL-241 — modularisation, static-types, performance &
> scalability assessment (cross-feature)
> **Cycle:** 2026-05

## Why this file exists

The TMAIL-262 issue body asks for the deliverable at
`docs/assessments/types-contract-parity-2026-05.md`. The substantive report
landed earlier under the TMAIL-257-named sibling path
[`frontend-types-parity-2026-05.md`](frontend-types-parity-2026-05.md) (commit
[`f2ec6f2`](https://github.com/tasltd/tasmail/commit/f2ec6f2)), because both
tickets describe the same scope and `docs/assessments/INDEX.md` already pairs
them.

This file is a stable redirect so that anyone searching the repo for the
literal filename quoted in the TMAIL-262 ticket finds the deliverable without
having to grep INDEX.md.

## Canonical report

➡️ **[`frontend-types-parity-2026-05.md`](frontend-types-parity-2026-05.md)**

That single ~715-line report closes both tickets and covers every TMAIL-262
specific check:

| TMAIL-262 check | Where in the canonical report |
|---|---|
| Every Axum handler response has a matching TS interface? | §1 surface-area metrics; §2 handler-↔-api/ pairing table (54 of 67 paired) |
| `Promise<any>` / `Promise<unknown>` returns | §1 ("**0** / **0**"), §6 narrowness audit |
| Discriminated unions for state machines vs free-form strings | §5 — narrows `MessageState`, `WebhookEventType`, `DlpAction`, `SpamAction`, `SyncChange` (tagged), notes Rust-wider-than-TS gaps in `sieve_rule` |
| Rust `serde` rename / skip attributes vs TS field names | §8.1 — table of `rename`, `rename_all`, `flatten`, `skip_serializing_if` usage; cross-checks against TS field names |
| Dart models in `mobile/lib/models/` match Rust 1:1 | §3 — field-by-field diff of `auth.dart`, `email.dart`, `attachment_draft.dart`, `sync_checkpoint.dart`. Flags the **P0** `LoginResponse.user` Dart cast that crashes against the real backend, and the **P1** ~6.7 % mobile model coverage (4 of 60 Rust models) |
| Cross-stack table: backend route → TS type → Dart type → drift markers | §2 + §3 (paired tables) |
| Idea — CI hook to break the build when a new handler ships without a TS type | §10 Follow-ups — listed as a follow-up task to file separately |

## TL;DR of findings (from the canonical report)

1. **P0** — Dart `LoginResponse.fromJson` expects `json['user']` (non-nullable),
   but `handlers/auth.rs::login` returns `TokenPair { access_token,
   refresh_token, expires_in }` with **no `user` field**. Dart crashes
   client-side before the inbox renders. The TS `TokenPair` shape is correct
   against the Rust source of truth; Dart is the divergent stack.
2. **P1** — Mobile model surface covers 4 of 60 Rust models (~6.7 %). Anything
   beyond login, inbox-list, message-detail and sync-checkpoint flows through
   the Dart client as `Map<String, dynamic>`. Needs a strategy decision, not a
   per-screen fix.
3. **P1** — 255 inline TS interfaces in `api/*.ts` vs 27 canonical ones in
   `types/` (9.4× ratio). Several entity types (`CalendarEvent`,
   `EmailDelegation`, `SieveRule`) exist only in `api/*.ts`, so non-API
   consumers couple presentation to the API client.
4. Zero `Promise<any>` and zero `Promise<unknown>` return types across all 65
   `frontend/src/api/*.ts` modules — the wire-call narrowness is in good
   shape.

See the canonical report for the full detail, file references, line numbers,
and follow-up task list.
