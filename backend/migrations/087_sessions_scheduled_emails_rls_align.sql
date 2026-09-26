-- Realign `sessions` and `scheduled_emails` RLS with the convention that
-- migrations 075 and 086 applied to shared_mailbox_acl, signatures, contacts,
-- distribution_groups and group_members.
--
-- Background:
--   * Migration 008 declared FORCE ROW LEVEL SECURITY on both tables.
--   * The model code for both runs on the plain pool with no `app.mailbox_id`
--     session var: models/session.rs (refresh-token lookup, revoke, revoke-all,
--     expired cleanup) and models/scheduled_email.rs (create, called from
--     POST /api/scheduled in handlers/scheduled.rs).
--   * Under FORCE the table owner (the app role) is subject to the policies, so
--     INSERT fails with "new row violates row-level security policy" (the
--     schedule-send endpoint returns 500) and SELECT/DELETE silently match zero
--     rows (a refresh token never validates, a revoke never revokes).
--   * Reproduced by tests/scheduled_email_attachments_test.rs and
--     tests/classic_settings_{password,sessions}_test.rs.
--
-- Fix shape (mirrors 086):
--   * Drop FORCE so the owner role bypasses RLS like every ENABLE-only table.
--   * Keep ENABLE and the policies so a future non-owner role still inherits
--     tenant isolation. Handler-level `WHERE mailbox_id = $N` checks remain the
--     access gate.
--
-- Safety: no data or schema change; idempotent.

ALTER TABLE sessions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE scheduled_emails NO FORCE ROW LEVEL SECURITY;
