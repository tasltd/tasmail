-- Email signatures and contacts tables

-- Signatures table: per-user email signatures
CREATE TABLE signatures (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    html_body TEXT NOT NULL DEFAULT '',
    text_body TEXT NOT NULL DEFAULT '',
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_signatures_mailbox_id ON signatures(mailbox_id);

-- Contacts table: address book entries per user
CREATE TABLE contacts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    display_name VARCHAR(255),
    company VARCHAR(255),
    phone VARCHAR(50),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_contacts_mailbox_id ON contacts(mailbox_id);
CREATE INDEX idx_contacts_email ON contacts(email);
CREATE UNIQUE INDEX idx_contacts_mailbox_email ON contacts(mailbox_id, email);
