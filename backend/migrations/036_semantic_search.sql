-- Added: Semantic search with pgvector for TMAIL-106
-- PURPOSE: Enable vector similarity search on email embeddings using cosine distance

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE email_embeddings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
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
