-- Added: POP3 configuration table for TMAIL-133
-- PURPOSE: Stores per-user POP3 access settings for Dovecot mailbox access

CREATE TABLE IF NOT EXISTS pop3_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT false,
    delete_after_download BOOLEAN NOT NULL DEFAULT false,
    retention_days INTEGER DEFAULT NULL,
    last_pop3_login TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Added: RLS policy for pop3_configurations
ALTER TABLE pop3_configurations ENABLE ROW LEVEL SECURITY;

CREATE POLICY pop3_configurations_user_policy ON pop3_configurations
    USING (user_id = current_setting('app.current_user_id')::uuid);
