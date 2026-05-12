-- Added: Chat integration table for team chat webhook notifications (TMAIL-129)
-- PURPOSE: Stores user-configured webhook URLs for forwarding email notifications
-- to Slack, Teams, Google Chat, Discord, or custom platforms

CREATE TYPE chat_platform AS ENUM ('slack', 'teams', 'google_chat', 'discord', 'custom');

CREATE TABLE chat_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id),
    platform chat_platform NOT NULL,
    webhook_url TEXT NOT NULL,
    channel_name TEXT,
    notify_on_receive BOOLEAN NOT NULL DEFAULT true,
    notify_on_send BOOLEAN NOT NULL DEFAULT false,
    notify_on_mention BOOLEAN NOT NULL DEFAULT true,
    filter_from TEXT,
    filter_subject TEXT,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_chat_integrations_user ON chat_integrations(user_id);
ALTER TABLE chat_integrations ENABLE ROW LEVEL SECURITY;
CREATE POLICY chat_integrations_user_policy ON chat_integrations
    USING (user_id = current_setting('app.current_user_id')::uuid);
