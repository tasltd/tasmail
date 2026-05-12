-- Added: PST import tracking table for TMAIL-115 (Outlook PST file import)
CREATE TYPE pst_import_status AS ENUM ('pending', 'processing', 'completed', 'failed');

CREATE TABLE pst_imports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id),
    filename TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    status pst_import_status NOT NULL DEFAULT 'pending',
    target_folder TEXT NOT NULL DEFAULT 'INBOX',
    messages_found INTEGER,
    messages_imported INTEGER DEFAULT 0,
    error_message TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_pst_imports_user ON pst_imports(user_id);

ALTER TABLE pst_imports ENABLE ROW LEVEL SECURITY;

CREATE POLICY pst_imports_user_policy ON pst_imports
    USING (user_id = current_setting('app.current_user_id')::uuid);
