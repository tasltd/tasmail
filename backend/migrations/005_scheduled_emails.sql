-- Scheduled and delayed emails for schedule-send and undo-send
CREATE TABLE scheduled_emails (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    to_addresses TEXT[] NOT NULL,
    cc_addresses TEXT[] DEFAULT '{}',
    bcc_addresses TEXT[] DEFAULT '{}',
    subject TEXT NOT NULL DEFAULT '',
    text_body TEXT,
    html_body TEXT,
    scheduled_at TIMESTAMPTZ NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    cancel_token UUID NOT NULL DEFAULT uuid_generate_v4(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ
);

CREATE INDEX idx_scheduled_emails_mailbox ON scheduled_emails(mailbox_id);
CREATE INDEX idx_scheduled_emails_status_time ON scheduled_emails(status, scheduled_at);
CREATE INDEX idx_scheduled_emails_cancel_token ON scheduled_emails(cancel_token);
