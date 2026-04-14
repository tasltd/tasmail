-- Added: LDAP/Active Directory configuration and sync log tables for TMAIL-100
CREATE TABLE ldap_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    server_url TEXT NOT NULL,
    bind_dn TEXT NOT NULL,
    bind_password_encrypted TEXT NOT NULL,
    search_base TEXT NOT NULL,
    search_filter TEXT NOT NULL DEFAULT '(objectClass=person)',
    email_attribute TEXT NOT NULL DEFAULT 'mail',
    name_attribute TEXT NOT NULL DEFAULT 'displayName',
    group_filter TEXT,
    sync_interval_minutes INTEGER NOT NULL DEFAULT 60,
    active BOOLEAN NOT NULL DEFAULT true,
    last_sync_at TIMESTAMPTZ,
    last_sync_status TEXT,
    users_synced INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE ldap_sync_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_id UUID NOT NULL REFERENCES ldap_configurations(id) ON DELETE CASCADE,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    users_created INTEGER NOT NULL DEFAULT 0,
    users_updated INTEGER NOT NULL DEFAULT 0,
    users_disabled INTEGER NOT NULL DEFAULT 0,
    errors JSONB DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'running'
);
CREATE INDEX idx_ldap_sync_logs_config ON ldap_sync_logs(config_id);
