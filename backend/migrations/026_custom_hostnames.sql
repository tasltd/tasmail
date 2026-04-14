-- Added: Custom hostname configuration per domain for SNI routing (TMAIL-112)
CREATE TABLE custom_hostnames (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id),
    smtp_hostname TEXT NOT NULL,
    imap_hostname TEXT NOT NULL,
    webmail_hostname TEXT,
    autodiscover_hostname TEXT,
    tls_cert_path TEXT,
    tls_key_path TEXT,
    verified BOOLEAN NOT NULL DEFAULT false,
    verified_at TIMESTAMPTZ,
    dns_verification_token TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_custom_hostnames_domain ON custom_hostnames(domain_id);
CREATE UNIQUE INDEX idx_custom_hostnames_smtp ON custom_hostnames(smtp_hostname);
CREATE UNIQUE INDEX idx_custom_hostnames_imap ON custom_hostnames(imap_hostname);
