-- TMAIL-204: push_notification_log.device_id should not block device delete.
--
-- Migration 051 made device_id `REFERENCES push_devices(id)` with no ON
-- DELETE action, so unregistering a device that has ever received a test
-- notification 500s with:
--     update or delete on table "push_devices" violates foreign key
--     constraint "push_notification_log_device_id_fkey"
--
-- Setting the FK to ON DELETE SET NULL preserves the delivery history
-- (the log row stays, with device_id nulled out) while letting the user
-- unregister at any time. CASCADE would also work but loses audit data.

ALTER TABLE push_notification_log
  DROP CONSTRAINT IF EXISTS push_notification_log_device_id_fkey;

ALTER TABLE push_notification_log
  ADD CONSTRAINT push_notification_log_device_id_fkey
  FOREIGN KEY (device_id) REFERENCES push_devices(id) ON DELETE SET NULL;
