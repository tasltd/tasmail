-- Added: TMAIL-103 — cache AI-generated email and thread summaries to avoid
-- re-paying provider tokens for the same content.
-- PURPOSE: Stores per-user summaries keyed by folder + uid + body hash for
-- single-email summaries, or by sorted uid list hash for thread summaries.
-- CONSTRAINTS: body_hash is a SHA-256 of the source text, so the cache
-- invalidates automatically when the message body changes (e.g. user edits
-- a draft and re-summarizes). RLS enforced via app.current_user_id.

CREATE TABLE email_summary_cache (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    -- 'single' for one email, 'thread' for conversation summaries
    kind TEXT NOT NULL CHECK (kind IN ('single', 'thread')),
    folder TEXT NOT NULL,
    -- For 'single': the message UID. For 'thread': the lowest UID in the set
    -- (the full uid list is captured in body_hash so collisions don't return
    -- wrong results).
    uid BIGINT NOT NULL,
    -- SHA-256 hex digest of the source content (email body for 'single',
    -- sorted "uid1,uid2,..." for 'thread'). Hex so it's diffable in pg dumps.
    body_hash TEXT NOT NULL,
    summary TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    -- For thread summaries: how many messages were included.
    message_count INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, kind, folder, uid, body_hash)
);

-- Lookups: (user, kind, folder, uid, body_hash) is the natural read path.
-- Already covered by the UNIQUE index above.
CREATE INDEX idx_email_summary_cache_user_created
    ON email_summary_cache(user_id, created_at DESC);

ALTER TABLE email_summary_cache ENABLE ROW LEVEL SECURITY;
CREATE POLICY email_summary_cache_user_policy ON email_summary_cache
    USING (user_id = current_setting('app.current_user_id')::uuid);
