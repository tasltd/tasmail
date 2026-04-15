-- Added: ActiveSync device management tables for TMAIL-130
-- PURPOSE: Stores device registrations and sync policies for ActiveSync proxy management
-- NOTE: TASMail manages device metadata; actual ActiveSync protocol handled by Z-Push or similar proxy

-- ActiveSync policies define security requirements for mobile devices
CREATE TABLE IF NOT EXISTS activesync_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    require_encryption BOOLEAN NOT NULL DEFAULT true,
    max_inactivity_lock_mins INT DEFAULT 5,
    min_password_length INT DEFAULT 4,
    allow_simple_password BOOLEAN NOT NULL DEFAULT false,
    max_failed_password_attempts INT DEFAULT 10,
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- ActiveSync device registrations per user
CREATE TABLE IF NOT EXISTS activesync_devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_type TEXT NOT NULL,
    device_name TEXT,
    device_os TEXT,
    last_sync_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'allowed',
    policy_key TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (user_id, device_id)
);

-- Added: RLS on activesync_devices so users can only see their own devices
ALTER TABLE activesync_devices ENABLE ROW LEVEL SECURITY;

CREATE POLICY activesync_devices_user_policy ON activesync_devices
    USING (user_id = current_setting('app.current_user_id', true)::UUID);
