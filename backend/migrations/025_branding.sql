-- Added: White-label branding configuration table for TMAIL-111
CREATE TABLE branding (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_name TEXT NOT NULL DEFAULT 'TASMail',
    logo_url TEXT,
    favicon_url TEXT,
    primary_color TEXT NOT NULL DEFAULT '#2563eb',
    secondary_color TEXT NOT NULL DEFAULT '#1e40af',
    accent_color TEXT NOT NULL DEFAULT '#3b82f6',
    login_background_url TEXT,
    custom_css TEXT,
    footer_text TEXT,
    support_email TEXT,
    support_url TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Insert default branding row
INSERT INTO branding (id) VALUES (gen_random_uuid());
