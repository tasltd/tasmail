-- Added: Phishing report table for TMAIL-124 — stores per-message phishing scan results
CREATE TABLE IF NOT EXISTS phishing_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    message_uid INT NOT NULL,
    folder VARCHAR(255) NOT NULL,
    -- Detected suspicious indicators
    suspicious_links JSONB NOT NULL DEFAULT '[]'::jsonb,
    suspicious_sender BOOLEAN NOT NULL DEFAULT false,
    spoofed_display_name BOOLEAN NOT NULL DEFAULT false,
    risk_score INT NOT NULL DEFAULT 0 CHECK (risk_score BETWEEN 0 AND 100),
    -- User action: none, dismissed, reported, confirmed_safe
    user_action VARCHAR(20) NOT NULL DEFAULT 'none',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_phishing_reports_mailbox ON phishing_reports(mailbox_id, folder, message_uid);

ALTER TABLE phishing_reports ENABLE ROW LEVEL SECURITY;
CREATE POLICY phishing_reports_isolation ON phishing_reports
    USING (mailbox_id = current_setting('app.current_user_id')::uuid);
