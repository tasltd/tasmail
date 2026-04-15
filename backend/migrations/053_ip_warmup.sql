-- Added: IP warm-up tracking for new sending IPs (TMAIL-17)
-- PURPOSE: Tracks warm-up progress per sending IP to enforce daily send limits
-- CONSTRAINTS: RLS not needed — admin-only access enforced at handler level

CREATE TABLE ip_warmup_tracking (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ip_address VARCHAR(45) NOT NULL UNIQUE,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    current_day INTEGER NOT NULL DEFAULT 1,
    emails_sent_today INTEGER NOT NULL DEFAULT 0,
    total_emails_sent BIGINT NOT NULL DEFAULT 0,
    last_reset_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    paused BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now()
);
