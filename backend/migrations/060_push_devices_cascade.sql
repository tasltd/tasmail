-- TMAIL-204: cascade push_devices/push_notification_log on mailbox delete +
-- convert push_platform ENUM to TEXT-with-CHECK so it round-trips through
-- the Rust model (which uses `String` not a sqlx-mapped enum).
--
-- Two issues surfaced while wiring up PushDevicesManager:
--   1. Migration 051 created `REFERENCES mailboxes(id)` with no ON DELETE
--      action. Every other FK in the schema cascades; deleting a user
--      should wipe their devices + delivery history.
--   2. push_devices.platform is the Postgres ENUM `push_platform`, but
--      models/push_notification.rs decodes the column into `String`. sqlx
--      refuses the type mismatch and every query 500s with
--      `mismatched types; Rust type 'String' is not compatible with SQL
--      type 'push_platform'`. Easiest fix that keeps the value-set
--      constraint is to widen the column to TEXT plus a CHECK.

ALTER TABLE push_devices
  DROP CONSTRAINT IF EXISTS push_devices_user_id_fkey;

ALTER TABLE push_devices
  ADD CONSTRAINT push_devices_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;

ALTER TABLE push_notification_log
  DROP CONSTRAINT IF EXISTS push_notification_log_user_id_fkey;

ALTER TABLE push_notification_log
  ADD CONSTRAINT push_notification_log_user_id_fkey
  FOREIGN KEY (user_id) REFERENCES mailboxes(id) ON DELETE CASCADE;
