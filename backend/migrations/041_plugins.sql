-- Added: Plugin/extension architecture tables for TMAIL-132
-- PURPOSE: Allows users to register plugins that fire on email events (webhook, script, filter)

CREATE TABLE plugins (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users(id) ON DELETE CASCADE,  -- NOTE: null = system-wide plugin
    name        TEXT NOT NULL,
    description TEXT,
    plugin_type TEXT NOT NULL CHECK (plugin_type IN ('webhook', 'script', 'filter')),
    config      JSONB NOT NULL DEFAULT '{}',
    hooks       TEXT[] NOT NULL DEFAULT '{}',
    enabled     BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ DEFAULT now(),
    updated_at  TIMESTAMPTZ DEFAULT now()
);

-- Added: Index for efficient lookup of enabled plugins per user
CREATE INDEX idx_plugins_user_enabled ON plugins(user_id, enabled);

CREATE TABLE plugin_executions (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_id     UUID NOT NULL REFERENCES plugins(id) ON DELETE CASCADE,
    event         TEXT NOT NULL,
    status        TEXT NOT NULL CHECK (status IN ('success', 'error', 'timeout')),
    duration_ms   INT,
    error_message TEXT,
    executed_at   TIMESTAMPTZ DEFAULT now()
);

-- Added: RLS policies for plugins table
ALTER TABLE plugins ENABLE ROW LEVEL SECURITY;

-- NOTE: Users can see their own plugins and system-wide plugins (user_id IS NULL)
CREATE POLICY plugins_select ON plugins FOR SELECT
    USING (
        user_id::text = current_setting('app.current_user_id', true)
        OR user_id IS NULL
    );

CREATE POLICY plugins_insert ON plugins FOR INSERT
    WITH CHECK (
        user_id::text = current_setting('app.current_user_id', true)
    );

CREATE POLICY plugins_update ON plugins FOR UPDATE
    USING (
        user_id::text = current_setting('app.current_user_id', true)
    );

CREATE POLICY plugins_delete ON plugins FOR DELETE
    USING (
        user_id::text = current_setting('app.current_user_id', true)
    );

-- Added: RLS policies for plugin_executions table
ALTER TABLE plugin_executions ENABLE ROW LEVEL SECURITY;

-- NOTE: Users can see executions for their own plugins and system plugins
CREATE POLICY plugin_executions_select ON plugin_executions FOR SELECT
    USING (
        plugin_id IN (
            SELECT id FROM plugins
            WHERE user_id::text = current_setting('app.current_user_id', true)
               OR user_id IS NULL
        )
    );
