# Backend build repair — 26 Sep 2026

## Summary

The TASMail backend had not compiled since about 31 Aug 2026. The live
service could not start, and it filled the disk with build errors. This
report records what was broken, what was fixed and what is still open.

After the fix, `cargo build --release` succeeds and `cargo test` passes:
5,122 tests pass, 0 fail and 3 are ignored. The frontend builds, its 1,468
unit tests pass and `trace-check` is clean.

## The build loop

The `tasmail-backend` user service ran `cargo run --release` on every
start and restarted every 10 seconds. Because the working tree did not
compile, each restart was a failed build. The service had restarted 55,711
times, and its log (`~/.claude/logs/tasmail-backend.log`) grew by about
1.6 GB a day.

The unit file lives outside the repo at
`~/.config/systemd/user/tasmail-backend.service`. It was changed in three ways:

| Setting | Before | After |
|---|---|---|
| `ExecStart` | `cargo run --release --quiet` | `backend/target/release/tasmail` |
| `RestartSec` | 10 | 30 |
| Start limit | none | 5 starts per 10 minutes |

The service now runs the last good build. A broken working tree can no
longer take the service down or loop. Deploying is unchanged:
`cargo build --release`, then `systemctl --user restart tasmail-backend`.

## What was broken in the code

Commit `497d34d` (23 Aug, "restructure router") and later uncommitted work
left the backend half-refactored. Each problem and its fix:

| Problem | Effect | Fix |
|---|---|---|
| `router.rs` was cut down to 22 of 263 routes. The new `src/router/*.rs` files were never wired in and changed many paths. | Most of the API would have returned 404. | Restored the full router from before `497d34d`. Its paths match what the SPA calls. Removed the unused `src/router/` files. |
| `handlers/mobile.rs` lost 1,218 lines and ended with a "rest of the file would be here" marker. | The mobile API would not compile. | Restored the file from the last commit. No other file was truncated. |
| The new `Mailbox` extractor read `AppState` from request extensions, which nothing fills. | Every route using it would return 500. | It now reads the router state through axum's `FromRef`. |
| `lib.rs` made every module private. | The `dlp_milter` binary and the integration tests could not build. | Modules are `pub` again. `extractors` was added to both crate roots. |
| The LDAP sync scheduler (TMAIL-322) called functions that do not exist. | It could not compile. | Rewrote it on the real `LdapService` API. It follows the same path as `POST /api/admin/ldap/:id/sync`. A failure in one directory no longer stops the others. |
| Send, draft and search looked up the mailbox before checking input. | Hostile input reached the database before it was rejected, against TMAIL-37. | These three handlers validate first, then call `load_mailbox`. |
| `sessions` and `scheduled_emails` still forced row-level security (migration 008). | Scheduling an email returned 500. Refresh-token lookup, sign-out and revoke matched no rows. | New migration `087` removes FORCE on both. This is the same fix migration `086` made for four other tables. |
| A half-written SAML logout handler (TMAIL-305) sat inside the test module. | The test build failed. | Removed it. It was never routed. |
| `trace-check`'s baseline had been reset to 0 known orphans against the cut-down router. | The check reported 10 false "new" orphans. | Restored the baseline from before `497d34d`. |

## Follow-up: stale IMAP password cache

A security review of the push found that the in-memory cache of decrypted
IMAP credentials (TMAIL-435) never dropped an entry. After a user changed or
deleted their IMAP settings, the server kept using the old password until it
restarted.

- **`invalidate_user_credentials`** in `imap_service.rs` now clears both the in-memory entry and the Redis copy. All four places that write IMAP settings call it.
- **Cached entries expire after 5 minutes**, the same as the Redis cache. That covers changes made any other way.
- **Tests:** `tests/imap_credential_cache_test.rs` covers an empty cache, one user among 10,000, and leaving other users untouched. Unit tests cover entry age at zero, at the limit and far past it.

## Tests that were changed

- **15 test files** now set the new `imap_credential_cache` field on `AppState`.
- **`rls_context_test`** runs its isolation queries under a non-owner role. Since migration 086 the app role bypasses row-level security by design, so only a non-owner role proves the policy works. It also clears leftover tenant settings on pooled connections.
- **`classic_settings_sessions_test`** sends a `User-Agent`. The page shows the current request's user agent, so a request without one showed "(unknown)".
- **One doc example** in `db_session.rs` is marked `ignore`, because it is pseudocode.
- **ESLint** now turns off the React hooks and empty-pattern rules for `e2e/`. Playwright fixtures take a callback named `use`, and the hooks rule mistakes it for a React hook.

## Still open

- **Eight more tables still force row-level security:** `settings`, `backup_codes`, `audit_log`, `auto_reply_rules`, `auto_reply_log`, `quota_usage`, `migration_jobs` and `sms_otp_codes`. Any handler that uses the plain pool on these will fail the same way. No test fails today, so they were not changed.
- **SAML single logout (TMAIL-305)** is not built.
- **22 ESLint errors** remain in files this work did not touch. Most are React Compiler rules.
- **The noreply SMTP password is committed** in `frontend/e2e/fixtures/base.ts`. Rotate it and read it from an environment variable.
