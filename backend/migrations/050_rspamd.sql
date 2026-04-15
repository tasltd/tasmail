-- Added: Rspamd spam filter configuration tables (TMAIL-15)
CREATE TYPE spam_action AS ENUM ('reject', 'greylist', 'add_header', 'no_action');

CREATE TABLE spam_settings (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  domain_id UUID REFERENCES domains(id),
  threshold_reject DECIMAL(5,2) DEFAULT 15.0,
  threshold_greylist DECIMAL(5,2) DEFAULT 4.0,
  threshold_add_header DECIMAL(5,2) DEFAULT 6.0,
  learn_spam_enabled BOOLEAN DEFAULT true,
  learn_ham_enabled BOOLEAN DEFAULT true,
  dkim_signing_enabled BOOLEAN DEFAULT true,
  arc_signing_enabled BOOLEAN DEFAULT false,
  autolearn_enabled BOOLEAN DEFAULT true,
  custom_rules JSONB DEFAULT '[]',
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE spam_quarantine (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  message_id VARCHAR(500) NOT NULL,
  sender VARCHAR(254),
  subject VARCHAR(998),
  score DECIMAL(8,2) NOT NULL,
  action spam_action NOT NULL,
  symbols JSONB DEFAULT '[]',
  quarantined_at TIMESTAMPTZ DEFAULT now(),
  released BOOLEAN DEFAULT false,
  released_at TIMESTAMPTZ
);
ALTER TABLE spam_quarantine ENABLE ROW LEVEL SECURITY;
CREATE POLICY quarantine_owner ON spam_quarantine FOR ALL USING (user_id = current_setting('app.current_user_id')::uuid);
