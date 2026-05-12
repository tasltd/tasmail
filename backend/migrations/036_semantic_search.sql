-- Added: Semantic search with pgvector for TMAIL-106
-- PURPOSE: Enable vector similarity search on email embeddings using cosine distance
--
-- Fix: Wrap in DO block so the migration succeeds even when pgvector isn't installed.
-- When pgvector is missing the embeddings table simply isn't created and the semantic-search
-- handler returns 503 at runtime — TASMail keeps booting on hosts without the extension.
DO $migration$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_available_extensions WHERE name = 'vector') THEN
        CREATE EXTENSION IF NOT EXISTS vector;

        CREATE TABLE email_embeddings (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id UUID NOT NULL REFERENCES mailboxes(id),
            folder TEXT NOT NULL,
            uid INTEGER NOT NULL,
            subject TEXT,
            embedding vector(1536),
            model_used TEXT NOT NULL DEFAULT 'text-embedding-3-small',
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            UNIQUE(user_id, folder, uid)
        );

        CREATE INDEX idx_email_embeddings_user ON email_embeddings(user_id);
        CREATE INDEX idx_email_embeddings_vector ON email_embeddings USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

        ALTER TABLE email_embeddings ENABLE ROW LEVEL SECURITY;

        CREATE POLICY email_embeddings_user_policy ON email_embeddings
            USING (user_id = current_setting('app.current_user_id')::uuid);
    ELSE
        RAISE NOTICE 'pgvector extension not available — skipping email_embeddings table. Install postgresql-16-pgvector to enable semantic search.';
    END IF;
END $migration$;
