-- Added: NLP search history table for TMAIL-135
-- PURPOSE: Stores natural language search queries and their AI-parsed parameters for history/reuse
-- CONSTRAINTS: RLS enforced on user_id column

CREATE TABLE nlp_search_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    query_text TEXT NOT NULL,
    parsed_params JSONB NOT NULL DEFAULT '{}',
    result_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Added: Index for efficient user history lookups ordered by recency
CREATE INDEX idx_nlp_search_history_user_created
    ON nlp_search_history (user_id, created_at DESC);

-- Added: Enable RLS on nlp_search_history
ALTER TABLE nlp_search_history ENABLE ROW LEVEL SECURITY;

-- Added: RLS policy — users can only access their own search history
CREATE POLICY nlp_search_history_user_policy ON nlp_search_history
    USING (user_id = current_setting('app.current_user_id')::uuid);
