-- Added: CalDAV/CardDAV configuration table for TMAIL-117
-- PURPOSE: Stores per-user DAV server configs for calendar and contact sync

CREATE TABLE IF NOT EXISTS dav_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    server_url TEXT NOT NULL,
    username TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    dav_type TEXT NOT NULL CHECK (dav_type IN ('caldav', 'carddav', 'both')),
    sync_interval_minutes INT NOT NULL DEFAULT 60,
    last_sync_at TIMESTAMPTZ,
    sync_status TEXT DEFAULT 'idle' CHECK (sync_status IN ('idle', 'syncing', 'error')),
    sync_error TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Added: Index for fast user-scoped lookups
CREATE INDEX IF NOT EXISTS idx_dav_configurations_user_id ON dav_configurations(user_id);

-- Added: Row-Level Security so users can only access their own configs
ALTER TABLE dav_configurations ENABLE ROW LEVEL SECURITY;

CREATE POLICY dav_configurations_user_policy ON dav_configurations
    USING (user_id = current_setting('app.current_user_id')::uuid);
