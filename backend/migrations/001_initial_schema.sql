-- TASMail initial database schema
-- Manages domains, mailboxes (users), sessions, and aliases

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Domains table: each domain served by the mail system
CREATE TABLE domains (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL UNIQUE,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_domains_name ON domains(name);

-- Mailboxes table: email accounts (users)
CREATE TABLE mailboxes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    username VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    display_name VARCHAR(255),
    quota_bytes BIGINT NOT NULL DEFAULT 1073741824,  -- 1 GB default
    active BOOLEAN NOT NULL DEFAULT true,
    is_admin BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mailboxes_username ON mailboxes(username);
CREATE INDEX idx_mailboxes_domain_id ON mailboxes(domain_id);

-- Sessions table: refresh token storage for JWT auth
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    refresh_token_hash VARCHAR(64) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ip_address VARCHAR(45),
    user_agent TEXT
);

CREATE INDEX idx_sessions_token_hash ON sessions(refresh_token_hash);
CREATE INDEX idx_sessions_mailbox_id ON sessions(mailbox_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-- Aliases table: email forwarding aliases
CREATE TABLE aliases (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    source_address VARCHAR(255) NOT NULL,
    destination_address VARCHAR(255) NOT NULL,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_aliases_source ON aliases(source_address);
CREATE INDEX idx_aliases_domain_id ON aliases(domain_id);
CREATE UNIQUE INDEX idx_aliases_unique ON aliases(source_address, destination_address);

-- Settings table: per-user key-value settings
CREATE TABLE settings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    key VARCHAR(100) NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_settings_mailbox_key ON settings(mailbox_id, key);
