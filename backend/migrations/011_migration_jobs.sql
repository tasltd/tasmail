-- Migration jobs for IMAP-to-IMAP and MBOX import operations.
-- Tracks the status and progress of each migration task.

CREATE TABLE migration_jobs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    job_type VARCHAR(20) NOT NULL CHECK (job_type IN ('imap', 'mbox')),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    -- IMAP source credentials (encrypted at rest in production)
    source_host VARCHAR(255),
    source_port INTEGER DEFAULT 993,
    source_user VARCHAR(255),
    source_password_encrypted TEXT,
    source_use_ssl BOOLEAN DEFAULT true,
    -- MBOX file path (for mbox imports)
    mbox_file_path TEXT,
    -- Progress tracking
    folders_total INTEGER DEFAULT 0,
    folders_done INTEGER DEFAULT 0,
    messages_total INTEGER DEFAULT 0,
    messages_done INTEGER DEFAULT 0,
    bytes_transferred BIGINT DEFAULT 0,
    error_message TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_migration_jobs_mailbox ON migration_jobs(mailbox_id);
CREATE INDEX idx_migration_jobs_status ON migration_jobs(status);

-- RLS
ALTER TABLE migration_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE migration_jobs FORCE ROW LEVEL SECURITY;

CREATE POLICY migration_jobs_isolation ON migration_jobs
    USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid);
CREATE POLICY migration_jobs_admin ON migration_jobs
    USING (current_setting('app.is_admin', true) = 'true');
