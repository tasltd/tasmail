-- Added: Attachment storage table with ClamAV scan tracking for TMAIL-59
CREATE TABLE IF NOT EXISTS attachments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    message_uid INT,
    folder VARCHAR(255),
    filename VARCHAR(500) NOT NULL,
    content_type VARCHAR(255) NOT NULL DEFAULT 'application/octet-stream',
    size_bytes BIGINT NOT NULL,
    -- Storage location (local filesystem path or object storage key)
    storage_path TEXT NOT NULL,
    -- SHA-256 hash for deduplication and integrity
    checksum VARCHAR(64) NOT NULL,
    -- Virus scan status: pending, clean, infected, error
    scan_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    scan_result TEXT,
    scanned_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_attachments_mailbox ON attachments(mailbox_id);
CREATE INDEX idx_attachments_checksum ON attachments(checksum);
CREATE INDEX idx_attachments_scan ON attachments(scan_status) WHERE scan_status = 'pending';

ALTER TABLE attachments ENABLE ROW LEVEL SECURITY;
CREATE POLICY attachments_isolation ON attachments
    USING (mailbox_id = current_setting('app.current_user_id')::uuid);
