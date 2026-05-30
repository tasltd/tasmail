-- TMAIL-319: persist reply / forward threading headers on scheduled_emails so
-- the email scheduler can set the outbound message's In-Reply-To and References
-- headers and downstream mail clients can thread the conversation correctly
-- (RFC 5322 §3.6.4). Both columns nullable / default empty — pre-existing rows
-- and brand-new compose sends (no reply context) leave them unset.
ALTER TABLE scheduled_emails
    ADD COLUMN IF NOT EXISTS in_reply_to TEXT,
    ADD COLUMN IF NOT EXISTS reference_ids TEXT[] NOT NULL DEFAULT '{}';
