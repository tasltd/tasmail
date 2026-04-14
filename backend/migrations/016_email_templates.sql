-- Email templates with merge fields
CREATE TABLE IF NOT EXISTS email_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    subject VARCHAR(500) NOT NULL DEFAULT '',
    body_html TEXT NOT NULL DEFAULT '',
    body_text TEXT NOT NULL DEFAULT '',
    -- Merge fields available in this template (stored as JSONB array of field names)
    merge_fields JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- Category for organizing templates
    category VARCHAR(100),
    is_shared BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_email_templates_mailbox ON email_templates(mailbox_id);

-- Added: RLS policy
ALTER TABLE email_templates ENABLE ROW LEVEL SECURITY;

CREATE POLICY email_templates_isolation ON email_templates
    USING (mailbox_id = current_setting('app.current_user_id')::uuid OR is_shared = true);
