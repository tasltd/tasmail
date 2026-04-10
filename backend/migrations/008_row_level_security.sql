-- Row-Level Security policies for multi-tenant data isolation.
-- Uses session variable app.mailbox_id set by the application before each query.

-- Enable RLS on all mailbox-owned tables
ALTER TABLE settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE signatures ENABLE ROW LEVEL SECURITY;
ALTER TABLE contacts ENABLE ROW LEVEL SECURITY;
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE quota_usage ENABLE ROW LEVEL SECURITY;
ALTER TABLE scheduled_emails ENABLE ROW LEVEL SECURITY;
ALTER TABLE backup_codes ENABLE ROW LEVEL SECURITY;
ALTER TABLE auto_reply_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE auto_reply_log ENABLE ROW LEVEL SECURITY;

-- RLS policies: allow access only when app.mailbox_id matches the row's mailbox_id.
-- The application sets this via SET LOCAL app.mailbox_id before each request.

-- settings
CREATE POLICY settings_isolation ON settings
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- signatures
CREATE POLICY signatures_isolation ON signatures
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- contacts
CREATE POLICY contacts_isolation ON contacts
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- sessions
CREATE POLICY sessions_isolation ON sessions
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- audit_log
CREATE POLICY audit_log_isolation ON audit_log
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- quota_usage
CREATE POLICY quota_usage_isolation ON quota_usage
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- scheduled_emails
CREATE POLICY scheduled_emails_isolation ON scheduled_emails
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- backup_codes
CREATE POLICY backup_codes_isolation ON backup_codes
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- auto_reply_rules
CREATE POLICY auto_reply_rules_isolation ON auto_reply_rules
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- auto_reply_log
CREATE POLICY auto_reply_log_isolation ON auto_reply_log
  USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid)
  WITH CHECK (mailbox_id = current_setting('app.mailbox_id', true)::uuid);

-- Admin bypass: allow admin users full access to all rows.
-- Admin access is indicated by setting app.is_admin = 'true'.
CREATE POLICY settings_admin ON settings USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY signatures_admin ON signatures USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY contacts_admin ON contacts USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY sessions_admin ON sessions USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY audit_log_admin ON audit_log USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY quota_usage_admin ON quota_usage USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY scheduled_emails_admin ON scheduled_emails USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY backup_codes_admin ON backup_codes USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY auto_reply_rules_admin ON auto_reply_rules USING (current_setting('app.is_admin', true) = 'true');
CREATE POLICY auto_reply_log_admin ON auto_reply_log USING (current_setting('app.is_admin', true) = 'true');

-- NOTE: RLS requires the connection user to NOT be a superuser or table owner.
-- For production, create a restricted role:
--   CREATE ROLE tasmail_app LOGIN PASSWORD 'xxx';
--   GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO tasmail_app;
-- Then set FORCE ROW LEVEL SECURITY on tables where the owner also needs RLS:
ALTER TABLE settings FORCE ROW LEVEL SECURITY;
ALTER TABLE signatures FORCE ROW LEVEL SECURITY;
ALTER TABLE contacts FORCE ROW LEVEL SECURITY;
ALTER TABLE sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE audit_log FORCE ROW LEVEL SECURITY;
ALTER TABLE quota_usage FORCE ROW LEVEL SECURITY;
ALTER TABLE scheduled_emails FORCE ROW LEVEL SECURITY;
ALTER TABLE backup_codes FORCE ROW LEVEL SECURITY;
ALTER TABLE auto_reply_rules FORCE ROW LEVEL SECURITY;
ALTER TABLE auto_reply_log FORCE ROW LEVEL SECURITY;
