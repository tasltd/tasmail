-- TMAIL-289: realign shared_mailbox_acl RLS with the post-TMAIL-161 convention.
--
-- Migration 010 created the table with `FORCE ROW LEVEL SECURITY` and policies
-- referencing `current_setting('app.mailbox_id', true)`. After TMAIL-161 the
-- auth middleware no longer SETs `app.mailbox_id` on the pool connection (the
-- SET landed on the wrong connection anyway), and every other table that grew
-- RLS after migration 010 standardised on `app.current_user_id` instead
-- (017 webauthn, 019 attachments, 020 phishing, 028 shared_files, etc).
--
-- The practical result of leaving migration 010 unchanged was that every
-- query against shared_mailbox_acl from the app's connection (alleina, no
-- BYPASSRLS) would evaluate the policy with an unset session var, which
-- silently dropped every row. Live behaviour only "worked" because of
-- connection-pool state leakage from the audit_log path setting
-- `app.is_admin = 'true'` and the unrelated `set_config('app.mailbox_id', ...)`
-- still called from auth_service::login. Fragile and incorrect.
--
-- Fix: drop FORCE so the app role (table owner) bypasses RLS, replace the
-- broken policies with ones keyed on `app.current_user_id` (matching every
-- table created since), and keep the admin bypass on `app.is_admin`. Handler-
-- level authz in handlers/shared.rs already enforces "must be the mailbox
-- owner or admin" for grant/list/revoke, so handler-level checks remain the
-- canonical access gate.

ALTER TABLE shared_mailbox_acl NO FORCE ROW LEVEL SECURITY;

-- Idempotent re-apply: drop legacy and new policies before recreating so
-- running this file twice in a row never errors.
DROP POLICY IF EXISTS shared_acl_own ON shared_mailbox_acl;
DROP POLICY IF EXISTS shared_acl_admin ON shared_mailbox_acl;
DROP POLICY IF EXISTS shared_acl_visibility ON shared_mailbox_acl;

CREATE POLICY shared_acl_visibility ON shared_mailbox_acl
    USING (
        granted_to = current_setting('app.current_user_id', true)::uuid
        OR mailbox_id = current_setting('app.current_user_id', true)::uuid
    );

CREATE POLICY shared_acl_admin ON shared_mailbox_acl
    USING (current_setting('app.is_admin', true) = 'true');
