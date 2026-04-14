-- Email snooze: temporarily hide messages until a specified time
CREATE TABLE IF NOT EXISTS snoozed_emails (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    folder VARCHAR(255) NOT NULL,
    message_uid INTEGER NOT NULL,
    snooze_until TIMESTAMPTZ NOT NULL,
    -- Original folder to move back to when snooze expires
    original_folder VARCHAR(255) NOT NULL DEFAULT 'INBOX',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Added: index for efficient polling of expired snoozes
CREATE INDEX idx_snoozed_emails_until ON snoozed_emails(snooze_until) WHERE snooze_until > NOW();
CREATE INDEX idx_snoozed_emails_mailbox ON snoozed_emails(mailbox_id);

-- Added: RLS policy
ALTER TABLE snoozed_emails ENABLE ROW LEVEL SECURITY;

CREATE POLICY snoozed_emails_isolation ON snoozed_emails
    USING (mailbox_id = current_setting('app.current_user_id')::uuid);
