-- Added: Payment provider credential storage (mirrors PayPro's PaymentProviderConfig domain).
-- TASMail uses the SAME four providers as PayPro: PAYSTACK / MASTERCARD / CYBERSOURCE / BANK_TRANSFER.
-- Sensitive fields (secret_key, api_password, etc.) are stored encrypted at rest and only decrypted in-memory at request time.
--
-- Priority order at runtime (matches PayPro):
--   1. Tenant-scoped row (tenant_id IS NOT NULL AND enabled = true) wins for that tenant
--   2. Global row (tenant_id IS NULL AND enabled = true) is the fallback
--   3. If neither row exists, the provider is disabled

CREATE TABLE payment_provider_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Provider type. Whitelisted to PayPro's four supported providers.
    provider TEXT NOT NULL CHECK (provider IN ('PAYSTACK', 'MASTERCARD', 'CYBERSOURCE', 'BANK_TRANSFER')),
    -- NULL tenant_id = global config; non-null = tenant-scoped override
    tenant_id UUID NULL,

    -- Friendly identifiers
    name TEXT,
    description TEXT,

    -- ===== Encrypted credential fields (AES-256-GCM, ciphertext stored as text) =====
    secret_key TEXT,         -- Paystack secret_key, generic API secret
    public_key TEXT,         -- Paystack public_key
    webhook_secret TEXT,     -- HMAC secret for webhook verification
    merchant_id TEXT,        -- Mastercard / Cybersource merchant ID
    api_password TEXT,       -- Mastercard MPGS API password
    key_id TEXT,             -- Cybersource HTTP-Signature key id
    shared_secret_key TEXT,  -- Cybersource HMAC shared secret (base64)
    key_file_path TEXT,      -- Cybersource P12 key file path (filesystem ref)

    -- ===== Non-encrypted configuration =====
    base_url TEXT,
    callback_url TEXT,
    currency TEXT DEFAULT 'GHS',
    environment TEXT DEFAULT 'sandbox' CHECK (environment IN ('sandbox', 'production')),
    enabled BOOLEAN NOT NULL DEFAULT true,
    archived BOOLEAN NOT NULL DEFAULT false,

    -- Bank-transfer-only details, stored as JSON (account name, number, bank, branch, swift, ref prefix)
    bank_details JSONB,

    -- Paystack split code (e.g., SPL_xxx) for revenue splits
    split_code TEXT,

    notes TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_ppc_provider          ON payment_provider_config(provider);
CREATE INDEX idx_ppc_tenant            ON payment_provider_config(tenant_id);
CREATE INDEX idx_ppc_provider_tenant   ON payment_provider_config(provider, tenant_id) WHERE enabled AND NOT archived;

-- Auto-update updated_at on row change
CREATE OR REPLACE FUNCTION ppc_set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ppc_updated_at_trigger
BEFORE UPDATE ON payment_provider_config
FOR EACH ROW EXECUTE FUNCTION ppc_set_updated_at();
