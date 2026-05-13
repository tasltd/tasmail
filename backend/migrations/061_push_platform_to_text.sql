-- TMAIL-204: convert push_devices.platform ENUM → TEXT + CHECK.
--
-- The Rust model (models/push_notification.rs::PushDevice) decodes the
-- column as `String`, but migration 051 created it as the Postgres ENUM
-- `push_platform`. sqlx refuses the type mismatch and every read of
-- push_devices 500s with:
--     mismatched types; Rust type 'alloc::string::String'
--     (as SQL type 'TEXT') is not compatible with SQL type 'push_platform'
--
-- Easiest fix that preserves the value-set guarantee is to widen the
-- column to TEXT plus a CHECK constraint. The API contract is unchanged
-- (still 'fcm' | 'apns' | 'web') and the Rust struct keeps decoding into
-- String without sqlx mapping gymnastics.

ALTER TABLE push_devices
  ALTER COLUMN platform TYPE TEXT USING platform::text;

ALTER TABLE push_devices
  ADD CONSTRAINT push_devices_platform_check
  CHECK (platform IN ('fcm', 'apns', 'web'));

DROP TYPE IF EXISTS push_platform;
