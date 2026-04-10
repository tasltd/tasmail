-- Distribution groups (email lists) backed by Postfix virtual aliases.
-- A group has an address (e.g., team@example.com) that expands to multiple members.

CREATE TABLE distribution_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    address VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    owner_mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    allow_external BOOLEAN NOT NULL DEFAULT false,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dist_groups_domain ON distribution_groups(domain_id);
CREATE INDEX idx_dist_groups_owner ON distribution_groups(owner_mailbox_id);
CREATE INDEX idx_dist_groups_address ON distribution_groups(address);

-- Group membership: maps group to member email addresses.
-- Members can be local mailboxes or external addresses.
CREATE TABLE group_members (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    group_id UUID NOT NULL REFERENCES distribution_groups(id) ON DELETE CASCADE,
    member_address VARCHAR(255) NOT NULL,
    mailbox_id UUID REFERENCES mailboxes(id) ON DELETE SET NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_group_members_unique ON group_members(group_id, member_address);
CREATE INDEX idx_group_members_group ON group_members(group_id);
CREATE INDEX idx_group_members_mailbox ON group_members(mailbox_id);

-- RLS for distribution_groups and group_members
ALTER TABLE distribution_groups ENABLE ROW LEVEL SECURITY;
ALTER TABLE distribution_groups FORCE ROW LEVEL SECURITY;
ALTER TABLE group_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE group_members FORCE ROW LEVEL SECURITY;

-- Group owners and members can see their groups
CREATE POLICY dist_groups_owner ON distribution_groups
    USING (owner_mailbox_id = current_setting('app.mailbox_id', true)::uuid);
CREATE POLICY dist_groups_admin ON distribution_groups
    USING (current_setting('app.is_admin', true) = 'true');

-- Members can see their group memberships
CREATE POLICY group_members_access ON group_members
    USING (
        mailbox_id = current_setting('app.mailbox_id', true)::uuid
        OR group_id IN (
            SELECT id FROM distribution_groups
            WHERE owner_mailbox_id = current_setting('app.mailbox_id', true)::uuid
        )
    );
CREATE POLICY group_members_admin ON group_members
    USING (current_setting('app.is_admin', true) = 'true');
