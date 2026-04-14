-- Sieve filter rules for server-side email filtering
CREATE TABLE IF NOT EXISTS sieve_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    -- Conditions (stored as JSONB for flexible matching)
    conditions JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- Match mode: 'all' (AND) or 'any' (OR)
    match_mode VARCHAR(10) NOT NULL DEFAULT 'all' CHECK (match_mode IN ('all', 'any')),
    -- Actions (stored as JSONB for flexible actions)
    actions JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- Stop processing further rules after this one matches
    stop_processing BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Added: index for fast lookup by mailbox, ordered by priority
CREATE INDEX idx_sieve_rules_mailbox_priority ON sieve_rules(mailbox_id, priority);

-- Added: RLS policy for sieve rules
ALTER TABLE sieve_rules ENABLE ROW LEVEL SECURITY;

CREATE POLICY sieve_rules_isolation ON sieve_rules
    USING (mailbox_id = current_setting('app.current_user_id')::uuid);
