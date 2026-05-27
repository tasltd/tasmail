-- Added: Quiet hours + badge count + thread grouping for push notifications (TMAIL-50)
-- PURPOSE: Lets users mute notifications during quiet hours, sync the unread badge
-- count back to devices, and group notifications by sender/thread.
-- NOTE: Quiet hours live on push_devices (not a separate table) so a user can have
-- different windows per device — e.g. work phone silent at night, personal phone
-- always on. The web SPA writes them on register; the mobile app reads OS-level
-- Do Not Disturb and mirrors it here.

ALTER TABLE push_devices
  ADD COLUMN IF NOT EXISTS quiet_hours_start TIME,
  ADD COLUMN IF NOT EXISTS quiet_hours_end TIME,
  -- IANA timezone name, e.g. 'Africa/Accra'. NULL = use UTC for the window check.
  ADD COLUMN IF NOT EXISTS quiet_hours_timezone TEXT,
  -- Last known unread count synced from the client. Pushed back as APNs badge /
  -- FCM data.badge so the system tray icon stays in sync without a fresh IMAP poll.
  ADD COLUMN IF NOT EXISTS badge_count INTEGER NOT NULL DEFAULT 0;

-- NOTE: Allow the badge_count >= 0 invariant at the DB layer
ALTER TABLE push_devices
  ADD CONSTRAINT push_devices_badge_count_nonneg CHECK (badge_count >= 0);
