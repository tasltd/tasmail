-- Added: Contact groups and membership for organizing contacts (TMAIL-119)

-- Contact groups table: label/group contacts by user
CREATE TABLE contact_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_contact_groups_user_id ON contact_groups(user_id);

-- Junction table: many-to-many between contact_groups and contacts
CREATE TABLE contact_group_members (
    contact_group_id UUID NOT NULL REFERENCES contact_groups(id) ON DELETE CASCADE,
    contact_id UUID NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    PRIMARY KEY (contact_group_id, contact_id)
);

CREATE INDEX idx_contact_group_members_contact ON contact_group_members(contact_id);

-- RLS on contact_groups: users can only see their own groups
ALTER TABLE contact_groups ENABLE ROW LEVEL SECURITY;

CREATE POLICY contact_groups_user_isolation ON contact_groups
    USING (user_id = current_setting('app.current_user_id')::uuid);
