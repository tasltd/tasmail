-- Added: Email archive tables for Piler integration (TMAIL-107)

-- PURPOSE: Stores admin-defined archiving policies that determine which emails get archived
CREATE TABLE archive_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    match_criteria JSONB NOT NULL DEFAULT '{}',
    archive_after_days INT NOT NULL DEFAULT 90,
    delete_original BOOLEAN NOT NULL DEFAULT false,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- PURPOSE: Tracks user archive search history for audit and quick re-search
CREATE TABLE archive_searches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    query TEXT NOT NULL,
    filters JSONB DEFAULT '{}',
    result_count INT,
    searched_at TIMESTAMPTZ DEFAULT now()
);

-- PURPOSE: Stores global Piler archive server configuration (singleton-ish, one row)
CREATE TABLE archive_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    piler_url TEXT,
    piler_api_key_encrypted TEXT,
    retention_years INT NOT NULL DEFAULT 7,
    enabled BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Added: RLS on archive_searches so users can only see their own search history
ALTER TABLE archive_searches ENABLE ROW LEVEL SECURITY;

CREATE POLICY archive_searches_user_policy ON archive_searches
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
