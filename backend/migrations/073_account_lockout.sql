-- Added (TMAIL-273): Per-account brute-force lockout.
--
-- Per-IP rate-limiting on /api/auth/login already exists, but a distributed
-- attacker rotating IPs can keep trying passwords against a single account
-- indefinitely. This migration adds the per-account counter + timestamp the
-- auth_service uses to enforce a 5-failure-in-15-min lockout.
--
-- Columns:
--   failed_login_attempts — rolling counter; reset on success OR when the
--     current attempt falls outside the configured window.
--   last_failed_login_at  — timestamp of the most recent failed attempt;
--     used to decide whether the counter has rolled over.
--   locked_until          — when set in the future, /api/auth/login returns
--     423 without checking the password. NULL means not locked.
--
-- Index choice: partial index on locked_until so the unlock sweep / admin
-- queries that look for currently-locked accounts don't scan the full table.
-- The vast majority of rows will have locked_until = NULL, so the partial
-- index stays tiny.

ALTER TABLE mailboxes
    ADD COLUMN IF NOT EXISTS failed_login_attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_failed_login_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS locked_until TIMESTAMPTZ NULL;

CREATE INDEX IF NOT EXISTS idx_mailboxes_locked_until
    ON mailboxes(locked_until)
    WHERE locked_until IS NOT NULL;
