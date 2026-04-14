-- Email delegation: send-as and send-on-behalf permissions
CREATE TABLE IF NOT EXISTS email_delegations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The mailbox that grants delegation
    grantor_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    -- The mailbox that receives delegation permission
    delegate_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    -- Delegation type: 'send_as' (appears as grantor) or 'send_on_behalf' (shows both names)
    delegation_type VARCHAR(20) NOT NULL CHECK (delegation_type IN ('send_as', 'send_on_behalf')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Added: prevent duplicate delegations
    UNIQUE (grantor_id, delegate_id, delegation_type)
);

CREATE INDEX idx_email_delegations_delegate ON email_delegations(delegate_id);
CREATE INDEX idx_email_delegations_grantor ON email_delegations(grantor_id);
