-- TMAIL-165: feature_flags table + seed defaults.
-- Powers the admin dashboard's runtime toggles for onboarding paths, signup,
-- billing, OIDC, and other operator-controlled product surfaces.
--
-- Lookups happen on every signup/login render → cached in Redis with TTL 60s
-- (see services/feature_flags.rs).

CREATE TABLE feature_flags (
    key TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT false,
    -- Optional structured payload for non-boolean settings (e.g. allowed signup domains).
    -- Most flags will leave this NULL and rely on `enabled` alone.
    value JSONB,
    -- True when the flag should be visible to unauthenticated callers
    -- (e.g. the SPA's /signup page asking which onboarding paths exist).
    is_public BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by UUID NULL REFERENCES mailboxes(id) ON DELETE SET NULL
);

CREATE INDEX idx_feature_flags_public ON feature_flags(is_public) WHERE is_public = true;

CREATE OR REPLACE FUNCTION feature_flags_set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER feature_flags_updated_at_trigger
BEFORE UPDATE ON feature_flags
FOR EACH ROW EXECUTE FUNCTION feature_flags_set_updated_at();

-- Seed: defaults that match the current product state (BYOK live, DNS-MX disabled).
INSERT INTO feature_flags (key, name, description, enabled, is_public) VALUES
    ('signup_enabled',
     'Public signup',
     'When false, /api/auth/signup returns 403 and the /signup page is hidden in the SPA.',
     true,
     true),
    ('byok_onboarding_enabled',
     'BYOK onboarding (bring your own IMAP/SMTP)',
     'Primary onboarding path. Users connect TASMail to an existing mail server via the wizard.',
     true,
     true),
    ('dns_mx_onboarding_enabled',
     'DNS-MX onboarding (managed mailbox on a TASMail-hosted domain)',
     'Secondary onboarding. Provisions a mailbox on the operator''s Postfix/Dovecot. Requires those services to be installed and DNS MX records configured for the managed domain.',
     false,
     true),
    ('oidc_login_enabled',
     'OIDC / social login',
     'Show third-party login buttons on /login when OIDC providers are configured.',
     false,
     true),
    ('billing_enabled',
     'Billing + subscriptions',
     'Show pricing tiers + checkout. Requires payment_provider_config rows for at least one provider.',
     false,
     true),
    ('public_signup_requires_invite',
     'Invite-only signup',
     'When true, /api/auth/signup must include a valid invite token.',
     false,
     false);
