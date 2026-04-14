-- Added: Email tasks/to-do table for TMAIL-126
CREATE TABLE email_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT,
    due_date TIMESTAMPTZ,
    completed BOOLEAN NOT NULL DEFAULT false,
    completed_at TIMESTAMPTZ,
    priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    -- Optional link to an email
    linked_folder TEXT,
    linked_uid INTEGER,
    linked_subject TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_email_tasks_user ON email_tasks(user_id);
CREATE INDEX idx_email_tasks_due ON email_tasks(user_id, due_date) WHERE NOT completed;
ALTER TABLE email_tasks ENABLE ROW LEVEL SECURITY;
CREATE POLICY email_tasks_user_policy ON email_tasks
    USING (user_id = current_setting('app.current_user_id')::uuid);
