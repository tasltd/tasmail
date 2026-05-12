-- Added: Sync state tracking for offline-first protocol (TMAIL-51)
-- PURPOSE: Per-user, per-device sync checkpoints to enable efficient delta sync
-- CONSTRAINTS: RLS enforced so users can only see their own checkpoints

CREATE TABLE sync_checkpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id),
    device_id UUID REFERENCES push_devices(id),
    folder_name VARCHAR(255) NOT NULL,
    last_uid BIGINT DEFAULT 0,
    last_modseq BIGINT DEFAULT 0,
    uidvalidity BIGINT DEFAULT 0,
    last_synced_at TIMESTAMPTZ DEFAULT now(),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(user_id, device_id, folder_name)
);

ALTER TABLE sync_checkpoints ENABLE ROW LEVEL SECURITY;
CREATE POLICY sync_checkpoint_owner ON sync_checkpoints
    FOR ALL USING (user_id = current_setting('app.current_user_id')::uuid);
