-- Added: eDiscovery search tables for compliance and legal investigations (TMAIL-137)

CREATE TYPE ediscovery_status AS ENUM ('pending', 'running', 'completed', 'failed', 'exported');

CREATE TABLE ediscovery_searches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    description TEXT,
    search_query TEXT NOT NULL,
    target_users UUID[],
    date_from TIMESTAMPTZ,
    date_to TIMESTAMPTZ,
    include_attachments BOOLEAN NOT NULL DEFAULT false,
    status ediscovery_status NOT NULL DEFAULT 'pending',
    results_count INTEGER DEFAULT 0,
    export_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX idx_ediscovery_admin ON ediscovery_searches(admin_id);

CREATE TABLE ediscovery_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    search_id UUID NOT NULL REFERENCES ediscovery_searches(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id),
    folder TEXT NOT NULL,
    uid INTEGER NOT NULL,
    subject TEXT,
    from_address TEXT,
    date TIMESTAMPTZ,
    snippet TEXT,
    relevance_score REAL DEFAULT 0
);
CREATE INDEX idx_ediscovery_results_search ON ediscovery_results(search_id);
