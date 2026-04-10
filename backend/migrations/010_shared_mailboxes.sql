-- Shared mailboxes with Dovecot ACL-based access control.
-- A shared mailbox is a regular mailbox that grants access to other users
-- with configurable permissions (read, write, delete, admin).

CREATE TABLE shared_mailbox_acl (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    granted_to UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    can_read BOOLEAN NOT NULL DEFAULT true,
    can_write BOOLEAN NOT NULL DEFAULT false,
    can_delete BOOLEAN NOT NULL DEFAULT false,
    can_admin BOOLEAN NOT NULL DEFAULT false,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    granted_by UUID REFERENCES mailboxes(id) ON DELETE SET NULL,
    CONSTRAINT unique_acl_pair UNIQUE (mailbox_id, granted_to)
);

CREATE INDEX idx_shared_acl_mailbox ON shared_mailbox_acl(mailbox_id);
CREATE INDEX idx_shared_acl_granted ON shared_mailbox_acl(granted_to);

-- Mark mailboxes as shared via a flag
ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS is_shared BOOLEAN NOT NULL DEFAULT false;

-- RLS: Users can see ACL entries where they are the mailbox owner, grantee, or admin
ALTER TABLE shared_mailbox_acl ENABLE ROW LEVEL SECURITY;
ALTER TABLE shared_mailbox_acl FORCE ROW LEVEL SECURITY;

CREATE POLICY shared_acl_own ON shared_mailbox_acl
    USING (
        granted_to = current_setting('app.mailbox_id', true)::uuid
        OR mailbox_id IN (
            SELECT id FROM mailboxes WHERE id = current_setting('app.mailbox_id', true)::uuid
        )
    );
CREATE POLICY shared_acl_admin ON shared_mailbox_acl
    USING (current_setting('app.is_admin', true) = 'true');
