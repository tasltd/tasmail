-- Added: DLP (Data Loss Prevention) rules and violations tracking for TMAIL-108

CREATE TYPE dlp_action AS ENUM ('block', 'quarantine', 'warn', 'log');
CREATE TYPE dlp_severity AS ENUM ('low', 'medium', 'high', 'critical');

CREATE TABLE dlp_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    pattern TEXT NOT NULL,
    pattern_type TEXT NOT NULL DEFAULT 'regex' CHECK (pattern_type IN ('regex', 'keyword', 'dictionary')),
    action dlp_action NOT NULL DEFAULT 'warn',
    severity dlp_severity NOT NULL DEFAULT 'medium',
    apply_to_subject BOOLEAN NOT NULL DEFAULT true,
    apply_to_body BOOLEAN NOT NULL DEFAULT true,
    apply_to_attachments BOOLEAN NOT NULL DEFAULT false,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE dlp_violations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID NOT NULL REFERENCES dlp_rules(id),
    user_id UUID NOT NULL REFERENCES users(id),
    action_taken dlp_action NOT NULL,
    matched_pattern TEXT NOT NULL,
    matched_text TEXT,
    message_subject TEXT,
    recipient TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_dlp_violations_user ON dlp_violations(user_id);
CREATE INDEX idx_dlp_violations_rule ON dlp_violations(rule_id);
