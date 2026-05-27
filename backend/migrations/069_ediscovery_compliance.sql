-- Added: TMAIL-137 — Complete eDiscovery: dedicated compliance officer RBAC,
-- legal hold scoping, and selectable export format (MBOX/EML/PDF).

-- Compliance officer role for eDiscovery access. Admins implicitly have it too;
-- this column lets us delegate compliance access without granting full admin.
ALTER TABLE mailboxes
    ADD COLUMN IF NOT EXISTS is_compliance_officer BOOLEAN NOT NULL DEFAULT false;

-- When true, the search auto-scopes to users with an active legal hold; useful
-- when investigators must not touch mailboxes outside an existing hold.
ALTER TABLE ediscovery_searches
    ADD COLUMN IF NOT EXISTS legal_hold_only BOOLEAN NOT NULL DEFAULT false;

-- Export format selection. TEXT + CHECK (not ENUM) per project convention so
-- sqlx can decode as String and new formats can be added without a type alter.
ALTER TABLE ediscovery_searches
    ADD COLUMN IF NOT EXISTS export_format TEXT NOT NULL DEFAULT 'mbox';

ALTER TABLE ediscovery_searches
    DROP CONSTRAINT IF EXISTS ediscovery_export_format_check;
ALTER TABLE ediscovery_searches
    ADD CONSTRAINT ediscovery_export_format_check
    CHECK (export_format IN ('mbox', 'eml', 'pdf'));
