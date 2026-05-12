-- TMAIL-176: usage-based billing schema (replaces flat per-user pricing).
--
-- Three tables:
--   usage_samples       — daily snapshot of each mailbox's used_bytes (rollup feeds this)
--   billing_periods     — one row per (mailbox_id, calendar month) with avg + peak storage
--   billing_invoices    — closed-out monthly invoice with computed amount in GHS
--
-- The existing billing_plans / subscriptions / payments tables stay in place for
-- historical / grandfathered rows; new BYOK accounts use this pipeline.

-- ----------------------------------------------------------------------
-- usage_samples — append-only daily snapshot
-- ----------------------------------------------------------------------
CREATE TABLE usage_samples (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    sampled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    used_bytes BIGINT NOT NULL,
    -- Optional message count snapshot, for trend reporting in /app/billing.
    message_count INTEGER,
    UNIQUE (mailbox_id, sampled_at)
);

CREATE INDEX idx_usage_samples_mailbox_time ON usage_samples(mailbox_id, sampled_at DESC);

-- ----------------------------------------------------------------------
-- billing_periods — running monthly accumulator, one row per (mailbox, month)
-- ----------------------------------------------------------------------
-- status flow:
--   open      – the period is the current calendar month, rollup keeps updating it
--   closed    – the month rolled over, rollup wrote the invoice and stopped touching it
CREATE TABLE billing_periods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    period_start DATE NOT NULL,    -- always the 1st of the month
    period_end DATE NOT NULL,      -- last day of the month
    -- Averaged across all usage_samples that landed in this period — what the
    -- invoice math uses. Stored as bytes; convert to GB at invoice time.
    avg_storage_bytes BIGINT NOT NULL DEFAULT 0,
    -- Highest snapshot observed in the period — exposed in the in-app dashboard
    -- so users can see "this month you peaked at X".
    peak_storage_bytes BIGINT NOT NULL DEFAULT 0,
    -- Number of samples folded into avg_storage_bytes (so partial months don't
    -- look artificially small).
    sample_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at TIMESTAMPTZ,
    UNIQUE (mailbox_id, period_start)
);

CREATE INDEX idx_billing_periods_status ON billing_periods(status, period_start);
CREATE INDEX idx_billing_periods_mailbox ON billing_periods(mailbox_id, period_start DESC);

-- ----------------------------------------------------------------------
-- billing_invoices — one row per closed period, immutable amounts
-- ----------------------------------------------------------------------
-- status flow:
--   pending   – computed but not yet paid; auto-charge worker will try the user's default provider
--   paid      – payment provider returned success
--   failed    – provider rejected; keeps queueing retries until manual intervention
--   waived    – admin marked off; no auto-charge attempted
CREATE TABLE billing_invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    period_id UUID NOT NULL REFERENCES billing_periods(id) ON DELETE CASCADE,
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    avg_storage_bytes BIGINT NOT NULL,
    -- Inputs to the invoice math, frozen at close time so a rate change later
    -- doesn't retroactively rewrite history.
    ghs_per_gb NUMERIC(10, 4) NOT NULL,
    ghs_monthly_min NUMERIC(10, 2) NOT NULL,
    -- Output: max(monthly_min, ceil(avg_storage_gb) * ghs_per_gb)
    amount_ghs NUMERIC(10, 2) NOT NULL,
    minimum_applied BOOLEAN NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'paid', 'failed', 'waived')),
    -- Provider-specific reference once the auto-charge worker fires.
    provider TEXT,
    provider_reference TEXT,
    last_attempt_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    paid_at TIMESTAMPTZ,
    UNIQUE (mailbox_id, period_id)
);

CREATE INDEX idx_billing_invoices_status ON billing_invoices(status, period_end);
CREATE INDEX idx_billing_invoices_mailbox ON billing_invoices(mailbox_id, period_end DESC);

-- ----------------------------------------------------------------------
-- Triggers + RLS
-- ----------------------------------------------------------------------
CREATE OR REPLACE FUNCTION billing_period_set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER bp_updated_at_trigger
BEFORE UPDATE ON billing_periods
FOR EACH ROW EXECUTE FUNCTION billing_period_set_updated_at();

CREATE TRIGGER bi_updated_at_trigger
BEFORE UPDATE ON billing_invoices
FOR EACH ROW EXECUTE FUNCTION billing_period_set_updated_at();

ALTER TABLE usage_samples     ENABLE ROW LEVEL SECURITY;
ALTER TABLE billing_periods   ENABLE ROW LEVEL SECURITY;
ALTER TABLE billing_invoices  ENABLE ROW LEVEL SECURITY;

CREATE POLICY usage_samples_owner ON usage_samples
    USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid);
CREATE POLICY billing_periods_owner ON billing_periods
    USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid);
CREATE POLICY billing_invoices_owner ON billing_invoices
    USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid);
