-- Added: TMAIL-58 — priority queue (urgent first), bounced status, spec-matched retry config
-- NOTE: keeps status as TEXT (no enum) so sqlx decodes to String; new values: pending|sending|sent|failed|dead_letter|bounced

-- Added: priority column (higher value = higher priority, default 0 = normal)
ALTER TABLE email_queue
    ADD COLUMN IF NOT EXISTS priority INT NOT NULL DEFAULT 0;

-- Changed: align default max_retries with spec (3 retries: 5s, 30s, 5m)
ALTER TABLE email_queue
    ALTER COLUMN max_retries SET DEFAULT 3;

-- Added: composite index for priority-ordered polling of ready items
-- Workers ORDER BY priority DESC, next_retry_at ASC so urgent items drain first
DROP INDEX IF EXISTS idx_email_queue_pending;
CREATE INDEX IF NOT EXISTS idx_email_queue_ready
    ON email_queue(priority DESC, next_retry_at ASC)
    WHERE status IN ('pending', 'failed');

-- Added: status CHECK constraint documents the allowed values (incl. new 'bounced')
ALTER TABLE email_queue
    DROP CONSTRAINT IF EXISTS email_queue_status_check;
ALTER TABLE email_queue
    ADD CONSTRAINT email_queue_status_check
    CHECK (status IN ('pending', 'sending', 'sent', 'failed', 'dead_letter', 'bounced'));
