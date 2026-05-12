-- Added: AI provider configuration for BYOK AI features (TMAIL-105)
-- PURPOSE: Stores user-configured AI API keys (encrypted) for email summarization and smart replies
-- CONSTRAINTS: One config per user+provider pair, RLS enforced

CREATE TYPE ai_provider AS ENUM ('openai', 'anthropic', 'google', 'ollama', 'custom');

CREATE TABLE ai_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id),
    provider ai_provider NOT NULL,
    api_key_encrypted TEXT NOT NULL,
    model_name TEXT NOT NULL,
    base_url TEXT,
    max_tokens INTEGER NOT NULL DEFAULT 500,
    temperature REAL NOT NULL DEFAULT 0.7,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, provider)
);
CREATE INDEX idx_ai_configs_user ON ai_configurations(user_id);
ALTER TABLE ai_configurations ENABLE ROW LEVEL SECURITY;
CREATE POLICY ai_configs_user_policy ON ai_configurations
    USING (user_id = current_setting('app.current_user_id')::uuid);
