-- SMS OTP support for two-factor authentication.
-- Stores phone numbers and OTP delivery preferences per mailbox.

ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS phone_number VARCHAR(20);
ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS sms_otp_enabled BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE mailboxes ADD COLUMN IF NOT EXISTS sms_provider VARCHAR(20) DEFAULT 'hubtel'
    CHECK (sms_provider IN ('hubtel', 'africastalking'));

-- OTP codes with short TTL (5 minutes) and single-use
CREATE TABLE sms_otp_codes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    code VARCHAR(6) NOT NULL,
    phone_number VARCHAR(20) NOT NULL,
    used BOOLEAN NOT NULL DEFAULT false,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sms_otp_mailbox ON sms_otp_codes(mailbox_id);
CREATE INDEX idx_sms_otp_expires ON sms_otp_codes(expires_at);

-- RLS
ALTER TABLE sms_otp_codes ENABLE ROW LEVEL SECURITY;
ALTER TABLE sms_otp_codes FORCE ROW LEVEL SECURITY;

CREATE POLICY sms_otp_isolation ON sms_otp_codes
    USING (mailbox_id = current_setting('app.mailbox_id', true)::uuid);
CREATE POLICY sms_otp_admin ON sms_otp_codes
    USING (current_setting('app.is_admin', true) = 'true');
