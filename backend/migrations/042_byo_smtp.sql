-- Added: BYO-SMTP configuration table for TMAIL-48
-- PURPOSE: Allows users to configure their own external SMTP servers for sending emails

CREATE TABLE smtp_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 587,
    username TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    encryption TEXT NOT NULL DEFAULT 'starttls' CHECK (encryption IN ('none', 'ssl', 'starttls')),
    from_address TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    verified BOOLEAN NOT NULL DEFAULT false,
    last_tested_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (user_id, name)
);

-- Added: Index for fast lookups of user's default SMTP config
CREATE INDEX idx_smtp_configurations_user_default ON smtp_configurations (user_id, is_default);

-- Added: RLS to restrict access to own SMTP configurations only
ALTER TABLE smtp_configurations ENABLE ROW LEVEL SECURITY;

CREATE POLICY smtp_configurations_user_policy ON smtp_configurations
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
