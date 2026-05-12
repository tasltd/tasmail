-- Added: Bulk user import tracking table for TMAIL-136
CREATE TYPE bulk_import_status AS ENUM ('pending', 'processing', 'completed', 'failed');

CREATE TABLE bulk_user_imports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_id UUID NOT NULL REFERENCES mailboxes(id),
    filename TEXT NOT NULL,
    total_rows INTEGER NOT NULL DEFAULT 0,
    processed_rows INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    errors JSONB DEFAULT '[]',
    status bulk_import_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX idx_bulk_imports_admin ON bulk_user_imports(admin_id);
