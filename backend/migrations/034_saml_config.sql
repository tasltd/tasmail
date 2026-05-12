-- Added: SAML 2.0 SSO configuration tables for TMAIL-101
CREATE TABLE saml_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    sso_url TEXT NOT NULL,
    slo_url TEXT,
    certificate TEXT NOT NULL,
    name_id_format TEXT NOT NULL DEFAULT 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress',
    attribute_mapping JSONB NOT NULL DEFAULT '{"email": "email", "name": "displayName"}',
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE saml_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    saml_config_id UUID NOT NULL REFERENCES saml_configurations(id),
    user_id UUID REFERENCES mailboxes(id),
    session_index TEXT,
    name_id TEXT NOT NULL,
    attributes JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_saml_sessions_user ON saml_sessions(user_id);
