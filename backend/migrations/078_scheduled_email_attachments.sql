-- TMAIL-321: persist attachment links on scheduled_emails so the modern UI's
-- ComposeModal can upload files to /api/attachments first and then send them
-- with the message body via /api/messages/schedule. Without this junction
-- the scheduler had no way to know which attachments belonged to which
-- scheduled row, and the Paperclip button in the Modern UI was a dead-end.
--
-- Schema notes:
--   * Composite PK on (scheduled_email_id, attachment_id) gives natural
--     dedup if the same attachment is added twice.
--   * `position` keeps the order the user added them so the outbound MIME
--     parts come out in a stable order (helps deterministic tests).
--   * Both FKs are CASCADE: cancelling a scheduled send tidies its links,
--     deleting an attachment removes any stale links instead of erroring.
--   * Index on attachment_id is the reverse-lookup the scheduler uses when
--     building the multipart payload.
CREATE TABLE IF NOT EXISTS scheduled_email_attachments (
    scheduled_email_id UUID NOT NULL
        REFERENCES scheduled_emails(id) ON DELETE CASCADE,
    attachment_id      UUID NOT NULL
        REFERENCES attachments(id) ON DELETE CASCADE,
    position           INTEGER NOT NULL DEFAULT 0,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (scheduled_email_id, attachment_id)
);

CREATE INDEX IF NOT EXISTS idx_scheduled_email_attachments_attachment
    ON scheduled_email_attachments (attachment_id);

CREATE INDEX IF NOT EXISTS idx_scheduled_email_attachments_email_position
    ON scheduled_email_attachments (scheduled_email_id, position);
