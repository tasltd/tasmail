-- Added: Email comments table for TMAIL-128 (internal comments on emails)
CREATE TABLE IF NOT EXISTS email_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    message_uid INT NOT NULL,
    folder VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    -- NOTE: Author info denormalized for display without extra joins
    author_name VARCHAR(255) NOT NULL,
    author_email VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Added: Index for fast lookup of comments on a specific message
CREATE INDEX idx_email_comments_message ON email_comments(mailbox_id, folder, message_uid);

-- Added: RLS policy to isolate comments per mailbox (same pattern as other tables)
ALTER TABLE email_comments ENABLE ROW LEVEL SECURITY;
CREATE POLICY email_comments_isolation ON email_comments
    USING (mailbox_id = current_setting('app.current_user_id')::uuid);
