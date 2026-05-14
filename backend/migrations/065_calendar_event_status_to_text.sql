-- TMAIL-240: convert calendar_events.status ENUM → TEXT + CHECK.
--
-- The Rust model (models/calendar_event.rs::CalendarEvent) decodes the
-- column as `String`, but migration 026 created it as the Postgres ENUM
-- `event_status`. sqlx refuses the type mismatch and every read or write
-- of calendar_events 500s with:
--     mismatched types; Rust type 'alloc::string::String'
--     (as SQL type 'TEXT') is not compatible with SQL type 'event_status'
--
-- Same fix pattern as migrations 061 (push_platform) and 063 (bulk_import
-- _status): widen to TEXT + CHECK so the value-set guarantee is preserved
-- without sqlx mapping gymnastics. API contract is unchanged
-- ('tentative' | 'confirmed' | 'cancelled'). The Rust struct keeps
-- decoding into String.

ALTER TABLE calendar_events
  ALTER COLUMN status DROP DEFAULT;

ALTER TABLE calendar_events
  ALTER COLUMN status TYPE TEXT USING status::text;

ALTER TABLE calendar_events
  ALTER COLUMN status SET DEFAULT 'tentative';

ALTER TABLE calendar_events
  ADD CONSTRAINT calendar_events_status_check
  CHECK (status IN ('tentative', 'confirmed', 'cancelled'));

DROP TYPE IF EXISTS event_status;
