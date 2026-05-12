-- Added: Per-user IMAP server configuration (TMAIL pivot to BYO webmail).
-- TASMail is a webmail UI on top of any IMAP/SMTP server. Users supply their own
-- IMAP credentials (Gmail, Outlook, Yahoo, FastMail, ProtonMail Bridge, corporate Exchange,
-- self-hosted Dovecot, etc.) and TASMail proxies their mailbox in the browser.
--
-- Mirrors the smtp_configurations table layout (migration 042) so the two-step
-- onboarding (IMAP + SMTP) shares conventions: encrypted password at rest,
-- one-default-per-user, RLS-isolated.

CREATE TABLE imap_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 993,
    username TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    -- 'ssl' = implicit TLS on connect (993), 'starttls' = STARTTLS upgrade (143), 'none' = plaintext (test only)
    encryption TEXT NOT NULL DEFAULT 'ssl' CHECK (encryption IN ('none', 'ssl', 'starttls')),
    -- Folder name conventions vary across providers — let users override the defaults
    sent_folder TEXT,
    drafts_folder TEXT,
    trash_folder TEXT,
    spam_folder TEXT,
    archive_folder TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    verified BOOLEAN NOT NULL DEFAULT false,
    last_tested_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, name)
);

CREATE INDEX idx_imap_configurations_user_default ON imap_configurations (user_id, is_default);

ALTER TABLE imap_configurations ENABLE ROW LEVEL SECURITY;

CREATE POLICY imap_configurations_user_policy ON imap_configurations
    USING (user_id = current_setting('app.current_user_id', true)::uuid);

-- Auto-update updated_at on row change
CREATE OR REPLACE FUNCTION imap_cfg_set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER imap_cfg_updated_at_trigger
BEFORE UPDATE ON imap_configurations
FOR EACH ROW EXECUTE FUNCTION imap_cfg_set_updated_at();
