-- Auto-reply / vacation responder rules
CREATE TABLE auto_reply_rules (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT false,
    subject TEXT NOT NULL DEFAULT 'Out of Office',
    body_text TEXT NOT NULL DEFAULT '',
    body_html TEXT,
    start_date TIMESTAMPTZ,
    end_date TIMESTAMPTZ,
    reply_to_all BOOLEAN NOT NULL DEFAULT false,
    exclude_lists BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(mailbox_id)
);

-- Track who we've already replied to (prevent loops)
CREATE TABLE auto_reply_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    sender_address VARCHAR(255) NOT NULL,
    replied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_auto_reply_log_lookup ON auto_reply_log(mailbox_id, sender_address);
CREATE INDEX idx_auto_reply_rules_active ON auto_reply_rules(mailbox_id, enabled);
