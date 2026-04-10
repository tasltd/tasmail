-- Track mailbox quota usage with periodic snapshots
CREATE TABLE quota_usage (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    used_bytes BIGINT NOT NULL DEFAULT 0,
    message_count INTEGER NOT NULL DEFAULT 0,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(mailbox_id)
);

CREATE INDEX idx_quota_usage_mailbox_id ON quota_usage(mailbox_id);

-- Add quota warning threshold to mailboxes
ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS quota_warn_percent INTEGER NOT NULL DEFAULT 80;
