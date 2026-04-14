-- Added: OIDC provider configuration and user linking tables for TMAIL-99
-- PURPOSE: Enable "Sign in with Google/Microsoft" via OpenID Connect

CREATE TABLE oidc_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    issuer_url TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT NOT NULL,
    scopes TEXT NOT NULL DEFAULT 'openid email profile',
    redirect_uri TEXT NOT NULL,
    auto_create_users BOOLEAN NOT NULL DEFAULT false,
    default_role TEXT NOT NULL DEFAULT 'user',
    active BOOLEAN NOT NULL DEFAULT true,
    icon_url TEXT,
    button_label TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE oidc_user_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    provider_id UUID NOT NULL REFERENCES oidc_providers(id),
    subject TEXT NOT NULL,
    email TEXT NOT NULL,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(provider_id, subject)
);
CREATE INDEX idx_oidc_links_user ON oidc_user_links(user_id);
