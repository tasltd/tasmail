-- Added: WebAuthn/FIDO2 passkey credentials table for TMAIL-83
CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    credential_id TEXT NOT NULL UNIQUE,
    public_key BYTEA NOT NULL,
    sign_count BIGINT NOT NULL DEFAULT 0,
    -- Human-readable name for the credential (e.g., "MacBook fingerprint")
    name VARCHAR(255) NOT NULL DEFAULT 'Security Key',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_webauthn_creds_mailbox ON webauthn_credentials(mailbox_id);

ALTER TABLE webauthn_credentials ENABLE ROW LEVEL SECURITY;

CREATE POLICY webauthn_creds_isolation ON webauthn_credentials
    USING (mailbox_id = current_setting('app.current_user_id')::uuid);
