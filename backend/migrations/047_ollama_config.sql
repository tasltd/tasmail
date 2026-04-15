-- Added: Ollama local LLM inference server configuration tables (TMAIL-102)
-- PURPOSE: Admin-level Ollama server settings and cached model metadata

-- NOTE: Single-row config table for the Ollama server connection
CREATE TABLE IF NOT EXISTS ollama_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    base_url TEXT NOT NULL DEFAULT 'http://localhost:11434',
    enabled BOOLEAN NOT NULL DEFAULT false,
    default_model TEXT DEFAULT 'llama3.2',
    max_context_length INT DEFAULT 4096,
    gpu_layers INT DEFAULT -1,
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Added: Insert a default row so GET always returns a config
INSERT INTO ollama_config (base_url, enabled, default_model, max_context_length, gpu_layers)
VALUES ('http://localhost:11434', false, 'llama3.2', 4096, -1)
ON CONFLICT DO NOTHING;

-- NOTE: Cached model metadata from Ollama /api/tags response
CREATE TABLE IF NOT EXISTS ollama_model_cache (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_name TEXT NOT NULL UNIQUE,
    size_bytes BIGINT,
    parameter_count TEXT,
    quantization TEXT,
    last_pulled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT now()
);
