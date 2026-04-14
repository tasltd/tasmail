-- Added: DANE (DNS-based Authentication of Named Entities) tables for TMAIL-125
-- PURPOSE: Store DANE policies per domain and verification results for outbound SMTP

-- Added: DANE policy configuration per domain
CREATE TABLE IF NOT EXISTS dane_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain TEXT NOT NULL UNIQUE,
    enforce BOOLEAN NOT NULL DEFAULT false,
    last_checked_at TIMESTAMPTZ,
    tlsa_records JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Added: DANE verification results for outbound messages
CREATE TABLE IF NOT EXISTS dane_verifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL,
    recipient_domain TEXT NOT NULL,
    dane_status TEXT NOT NULL CHECK (dane_status IN ('verified', 'failed', 'no_tlsa', 'disabled')),
    checked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Added: Index for efficient user-scoped queries
CREATE INDEX IF NOT EXISTS idx_dane_verifications_user_id ON dane_verifications(user_id);
CREATE INDEX IF NOT EXISTS idx_dane_verifications_checked_at ON dane_verifications(checked_at DESC);
CREATE INDEX IF NOT EXISTS idx_dane_policies_domain ON dane_policies(domain);

-- Added: RLS on dane_verifications so users only see their own verification results
ALTER TABLE dane_verifications ENABLE ROW LEVEL SECURITY;

CREATE POLICY dane_verifications_user_policy ON dane_verifications
    FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
