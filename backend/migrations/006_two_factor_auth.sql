-- Two-factor authentication support
ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS totp_secret VARCHAR(64);
ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS totp_enabled BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS totp_verified_at TIMESTAMPTZ;

-- Backup recovery codes for 2FA
CREATE TABLE backup_codes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    code_hash VARCHAR(128) NOT NULL,
    used BOOLEAN NOT NULL DEFAULT false,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_backup_codes_mailbox ON backup_codes(mailbox_id);
