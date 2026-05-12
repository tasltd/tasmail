-- Added: Shared files table for large file sharing via cloud storage links (TMAIL-138)
CREATE TABLE shared_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id),
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    file_size BIGINT NOT NULL,
    storage_path TEXT NOT NULL,
    download_token TEXT NOT NULL UNIQUE,
    download_count INTEGER NOT NULL DEFAULT 0,
    max_downloads INTEGER,
    expires_at TIMESTAMPTZ,
    password_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_shared_files_user ON shared_files(user_id);
CREATE UNIQUE INDEX idx_shared_files_token ON shared_files(download_token);
ALTER TABLE shared_files ENABLE ROW LEVEL SECURITY;
CREATE POLICY shared_files_user_policy ON shared_files
    USING (user_id = current_setting('app.current_user_id')::uuid);
