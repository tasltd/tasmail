-- TMAIL-202: backfill ON DELETE actions on every mailbox FK that was
-- created with the implicit NO ACTION default.
--
-- Each one of these blocks mailbox deletion when even a single child row
-- exists. The users-admin delete flow (TMAIL-202) hits at least one
-- (bulk_user_imports), but the same shape will trip every other admin
-- flow that needs to remove a test user. Sweep them all in one go to
-- match the cascade pattern the rest of the schema already uses.
--
-- CASCADE — per-user data with no audit value to preserve.
-- SET NULL — audit / compliance / financial rows that should outlive
--            their referenced mailbox.

-- ── CASCADE
ALTER TABLE ai_configurations DROP CONSTRAINT IF EXISTS ai_configurations_user_id_fkey;
ALTER TABLE ai_configurations ADD CONSTRAINT ai_configurations_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE calendar_events DROP CONSTRAINT IF EXISTS calendar_events_organizer_id_fkey;
ALTER TABLE calendar_events ADD CONSTRAINT calendar_events_organizer_id_fkey
  FOREIGN KEY (organizer_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE chat_integrations DROP CONSTRAINT IF EXISTS chat_integrations_user_id_fkey;
ALTER TABLE chat_integrations ADD CONSTRAINT chat_integrations_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE email_tasks DROP CONSTRAINT IF EXISTS email_tasks_user_id_fkey;
ALTER TABLE email_tasks ADD CONSTRAINT email_tasks_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE oidc_user_links DROP CONSTRAINT IF EXISTS oidc_user_links_user_id_fkey;
ALTER TABLE oidc_user_links ADD CONSTRAINT oidc_user_links_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE pst_imports DROP CONSTRAINT IF EXISTS pst_imports_user_id_fkey;
ALTER TABLE pst_imports ADD CONSTRAINT pst_imports_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE saml_sessions DROP CONSTRAINT IF EXISTS saml_sessions_user_id_fkey;
ALTER TABLE saml_sessions ADD CONSTRAINT saml_sessions_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE shared_files DROP CONSTRAINT IF EXISTS shared_files_user_id_fkey;
ALTER TABLE shared_files ADD CONSTRAINT shared_files_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE spam_quarantine DROP CONSTRAINT IF EXISTS spam_quarantine_user_id_fkey;
ALTER TABLE spam_quarantine ADD CONSTRAINT spam_quarantine_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE sync_checkpoints DROP CONSTRAINT IF EXISTS sync_checkpoints_user_id_fkey;
ALTER TABLE sync_checkpoints ADD CONSTRAINT sync_checkpoints_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE webhooks DROP CONSTRAINT IF EXISTS webhooks_user_id_fkey;
ALTER TABLE webhooks ADD CONSTRAINT webhooks_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

-- ── SET NULL (audit / compliance / financial)
ALTER TABLE bulk_user_imports DROP CONSTRAINT IF EXISTS bulk_user_imports_admin_id_fkey;
ALTER TABLE bulk_user_imports ALTER COLUMN admin_id DROP NOT NULL;
ALTER TABLE bulk_user_imports ADD CONSTRAINT bulk_user_imports_admin_id_fkey
  FOREIGN KEY (admin_id) REFERENCES mailboxes(id) ON DELETE SET NULL;

ALTER TABLE ediscovery_searches DROP CONSTRAINT IF EXISTS ediscovery_searches_admin_id_fkey;
ALTER TABLE ediscovery_searches ALTER COLUMN admin_id DROP NOT NULL;
ALTER TABLE ediscovery_searches ADD CONSTRAINT ediscovery_searches_admin_id_fkey
  FOREIGN KEY (admin_id) REFERENCES mailboxes(id) ON DELETE SET NULL;

ALTER TABLE ediscovery_results DROP CONSTRAINT IF EXISTS ediscovery_results_user_id_fkey;
ALTER TABLE ediscovery_results ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE ediscovery_results ADD CONSTRAINT ediscovery_results_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE SET NULL;

ALTER TABLE dlp_violations DROP CONSTRAINT IF EXISTS dlp_violations_user_id_fkey;
ALTER TABLE dlp_violations ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE dlp_violations ADD CONSTRAINT dlp_violations_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE SET NULL;

ALTER TABLE legal_holds DROP CONSTRAINT IF EXISTS legal_holds_placed_by_fkey;
ALTER TABLE legal_holds ALTER COLUMN placed_by DROP NOT NULL;
ALTER TABLE legal_holds ADD CONSTRAINT legal_holds_placed_by_fkey
  FOREIGN KEY (placed_by) REFERENCES mailboxes(id) ON DELETE SET NULL;

ALTER TABLE legal_holds DROP CONSTRAINT IF EXISTS legal_holds_user_id_fkey;
ALTER TABLE legal_holds ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE legal_holds ADD CONSTRAINT legal_holds_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE SET NULL;

ALTER TABLE payments DROP CONSTRAINT IF EXISTS payments_user_id_fkey;
ALTER TABLE payments ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE payments ADD CONSTRAINT payments_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE SET NULL;

ALTER TABLE subscriptions DROP CONSTRAINT IF EXISTS subscriptions_user_id_fkey;
ALTER TABLE subscriptions ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE subscriptions ADD CONSTRAINT subscriptions_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE SET NULL;
