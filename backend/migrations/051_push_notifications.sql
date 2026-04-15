-- Added: Push notification device registration and history (TMAIL-50)
CREATE TYPE push_platform AS ENUM ('fcm', 'apns', 'web');

CREATE TABLE push_devices (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  platform push_platform NOT NULL,
  device_token TEXT NOT NULL,
  device_name VARCHAR(200),
  app_version VARCHAR(50),
  active BOOLEAN DEFAULT true,
  last_used_at TIMESTAMPTZ DEFAULT now(),
  created_at TIMESTAMPTZ DEFAULT now()
);
ALTER TABLE push_devices ENABLE ROW LEVEL SECURITY;
CREATE POLICY push_device_owner ON push_devices FOR ALL USING (user_id = current_setting('app.current_user_id')::uuid);
CREATE UNIQUE INDEX push_device_token_unique ON push_devices(user_id, device_token);

CREATE TABLE push_notification_log (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  device_id UUID REFERENCES push_devices(id),
  title VARCHAR(200) NOT NULL,
  body TEXT,
  data JSONB DEFAULT '{}',
  sent_at TIMESTAMPTZ DEFAULT now(),
  delivered BOOLEAN DEFAULT false,
  error TEXT
);
