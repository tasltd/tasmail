-- Added: Retention policies and legal hold tables for TMAIL-109

CREATE TABLE retention_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    retention_days INTEGER NOT NULL CHECK (retention_days > 0),
    folder_pattern TEXT, -- NULL means all folders, or specific like 'Trash', 'Spam'
    apply_to_all BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE legal_holds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    reason TEXT NOT NULL,
    placed_by UUID NOT NULL REFERENCES users(id),
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ
);
CREATE INDEX idx_legal_holds_user ON legal_holds(user_id);
CREATE INDEX idx_legal_holds_active ON legal_holds(user_id) WHERE active;
