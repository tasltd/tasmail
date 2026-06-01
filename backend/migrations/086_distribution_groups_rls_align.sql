-- TMAIL-411: realign distribution_groups, group_members, signatures, and
-- contacts RLS with the post-TMAIL-161 / TMAIL-289 convention (same fix
-- shape migration 075 applied to shared_mailbox_acl).
--
-- Background:
--   * Migration 008 declared FORCE ROW LEVEL SECURITY on `signatures` and
--     `contacts`. Migration 009 declared it on `distribution_groups` and
--     `group_members`. Migration 010 had done the same for
--     `shared_mailbox_acl` (subsequently relaxed by 075).
--   * After TMAIL-161 the auth middleware was downgraded to a no-op and
--     stopped SETting `app.mailbox_id` on pool connections. TMAIL-309 added
--     a new rls_context_middleware + RlsConn extractor, but no handlers
--     migrated to use it — they all still call `&state.db` directly, which
--     pulls a fresh pool connection per query with no session vars set.
--   * The practical result is that every INSERT against these tables from
--     the app's role (alleina locally, the equivalent in prod) hits the
--     policy with `current_setting('app.mailbox_id', true)` returning an
--     empty string, fails the `::uuid` cast / row check, and the database
--     returns "new row violates row-level security policy for table …" →
--     the handler returns 500. SELECT silently returns empty.
--   * The contacts-templates-filters E2E sweep (TMAIL-411) reproduces this
--     for the `signatures` and `distribution_groups` paths. The same bug
--     latently affects the `contacts` POST path (CSV import already trips
--     it, regular API create does too).
--
-- Fix shape (mirrors 075_shared_mailbox_acl_rls_align.sql):
--   * Drop FORCE so the app role — which is the table owner in every
--     deployment, see `\\d signatures` / `\\d distribution_groups` — bypasses
--     RLS the same way it already does for the other ~30 ENABLE-only tables
--     in this schema (email_templates, sieve_rules, contact_groups, …).
--   * Leave ENABLE intact + leave the policies in place so a future
--     non-owner role (the still-aspirational `tasmail_app` mentioned in
--     008's comments) still inherits tenant isolation when the broader
--     RlsConn handler migration eventually lands.
--   * Handler-level explicit `WHERE mailbox_id = $N` + ownership checks in
--     handlers/signatures.rs, handlers/contacts.rs, handlers/groups.rs
--     remain the canonical access gate — same posture the rest of the
--     codebase already runs with.
--
-- Safety:
--   * `NO FORCE ROW LEVEL SECURITY` is idempotent — re-running the
--     statement against a table that already has FORCE removed is a no-op.
--   * No data changes; no schema changes; only the row-security mode is
--     flipped.

ALTER TABLE signatures NO FORCE ROW LEVEL SECURITY;
ALTER TABLE contacts NO FORCE ROW LEVEL SECURITY;
ALTER TABLE distribution_groups NO FORCE ROW LEVEL SECURITY;
ALTER TABLE group_members NO FORCE ROW LEVEL SECURITY;
