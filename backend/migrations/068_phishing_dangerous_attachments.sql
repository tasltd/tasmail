-- Added: TMAIL-124 — store dangerous attachment warnings on phishing reports (Outlook Safe Attachments equivalent)
ALTER TABLE phishing_reports
    ADD COLUMN IF NOT EXISTS dangerous_attachments JSONB NOT NULL DEFAULT '[]'::jsonb;
