-- Added: Billing tables for Paystack/MoMo payment integration (TMAIL-46)
-- PURPOSE: Supports subscription billing plans, user subscriptions, and payment tracking
-- NOTE: Ghana market — prices in GHS (Ghana Cedis), providers are Paystack and MTN MoMo

CREATE TYPE billing_plan_interval AS ENUM ('monthly', 'yearly');
CREATE TYPE payment_status AS ENUM ('pending', 'success', 'failed', 'refunded');
CREATE TYPE payment_provider AS ENUM ('paystack', 'mtn_momo');

CREATE TABLE billing_plans (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(100) NOT NULL,
  description TEXT,
  price_cedis DECIMAL(10,2) NOT NULL,
  interval billing_plan_interval NOT NULL DEFAULT 'monthly',
  max_mailboxes INT NOT NULL DEFAULT 1,
  storage_gb INT NOT NULL DEFAULT 5,
  features JSONB DEFAULT '{}',
  active BOOLEAN DEFAULT true,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE subscriptions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  plan_id UUID NOT NULL REFERENCES billing_plans(id),
  provider payment_provider NOT NULL,
  provider_subscription_id VARCHAR(255),
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  current_period_start TIMESTAMPTZ,
  current_period_end TIMESTAMPTZ,
  cancelled_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);
ALTER TABLE subscriptions ENABLE ROW LEVEL SECURITY;
CREATE POLICY subscription_owner ON subscriptions FOR ALL USING (user_id = current_setting('app.current_user_id')::uuid);

CREATE TABLE payments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  subscription_id UUID REFERENCES subscriptions(id),
  provider payment_provider NOT NULL,
  provider_ref VARCHAR(255) NOT NULL,
  amount_cedis DECIMAL(10,2) NOT NULL,
  currency VARCHAR(3) DEFAULT 'GHS',
  status payment_status NOT NULL DEFAULT 'pending',
  metadata JSONB DEFAULT '{}',
  created_at TIMESTAMPTZ DEFAULT now()
);
ALTER TABLE payments ENABLE ROW LEVEL SECURITY;
CREATE POLICY payment_owner ON payments FOR ALL USING (user_id = current_setting('app.current_user_id')::uuid);
