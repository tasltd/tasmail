-- TMAIL-181: enterprise quote-request inbox.
-- Public form on the landing page POSTs into this table; admins triage from
-- /admin/quote-requests; status walks new → contacted → quoted → won|lost.

CREATE TABLE enterprise_quote_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    contact_name TEXT NOT NULL,
    contact_email TEXT NOT NULL,
    company TEXT,
    estimated_users INT,
    message TEXT NOT NULL,

    status TEXT NOT NULL DEFAULT 'new'
        CHECK (status IN ('new', 'contacted', 'quoted', 'won', 'lost')),

    -- Internal notes appended by sales as they progress the deal.
    internal_notes TEXT,

    -- Spam/abuse forensics — captured at submission time.
    source_ip INET,
    user_agent TEXT,

    -- Optional sales-rep assignment (admin mailbox id).
    assigned_to UUID NULL REFERENCES mailboxes(id) ON DELETE SET NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    contacted_at TIMESTAMPTZ,
    quoted_at TIMESTAMPTZ,
    closed_at TIMESTAMPTZ
);

CREATE INDEX idx_eqr_status ON enterprise_quote_requests(status);
CREATE INDEX idx_eqr_created_at ON enterprise_quote_requests(created_at DESC);
CREATE INDEX idx_eqr_email ON enterprise_quote_requests(lower(contact_email));

CREATE OR REPLACE FUNCTION eqr_set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER eqr_updated_at_trigger
BEFORE UPDATE ON enterprise_quote_requests
FOR EACH ROW EXECUTE FUNCTION eqr_set_updated_at();
