-- Added: Email queue table with retry logic for TMAIL-58
CREATE TABLE IF NOT EXISTS email_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    to_addresses TEXT[] NOT NULL,
    cc_addresses TEXT[] NOT NULL DEFAULT '{}',
    bcc_addresses TEXT[] NOT NULL DEFAULT '{}',
    subject VARCHAR(1000) NOT NULL DEFAULT '',
    body_html TEXT NOT NULL DEFAULT '',
    body_text TEXT NOT NULL DEFAULT '',
    -- Queue status: pending, sending, sent, failed, dead_letter
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    -- Retry tracking
    retry_count INT NOT NULL DEFAULT 0,
    max_retries INT NOT NULL DEFAULT 5,
    -- Next retry time (exponential backoff: base_delay * 2^retry_count)
    next_retry_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error TEXT,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ
);

-- Added: Partial index for efficient polling of ready-to-send items
CREATE INDEX idx_email_queue_pending ON email_queue(status, next_retry_at) WHERE status IN ('pending', 'failed');
CREATE INDEX idx_email_queue_mailbox ON email_queue(mailbox_id);
