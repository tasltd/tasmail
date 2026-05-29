-- TMAIL-287: convert event_attendees.rsvp ENUM → TEXT + CHECK.
--
-- Same family of bug as migrations 061/063/065. The Rust model
-- (models/calendar_event.rs::EventAttendee) decodes `rsvp` as `String`,
-- but migration 031 created the column as the Postgres ENUM `rsvp_status`.
-- sqlx refuses the type mismatch and every endpoint that reads or
-- writes event_attendees 500s with:
--     mismatched types; Rust type 'alloc::string::String'
--     (as SQL type 'TEXT') is not compatible with SQL type 'rsvp_status'
--
-- This broke ALL of the following in production:
--   • POST /api/calendar/events with attendees       (response serialization)
--   • POST /api/calendar/events/{id}/rsvp            (RETURNING * decode)
--   • POST /api/calendar/imip/accept                 (attendee upsert)
--   • POST /api/calendar/public/{token}/rsvp         (public booking RSVP)
--
-- Fix: widen to TEXT + CHECK so the value-set guarantee is preserved
-- without sqlx mapping gymnastics. API contract is unchanged
-- ('pending' | 'accepted' | 'declined' | 'maybe'). The Rust struct keeps
-- decoding into String.

ALTER TABLE event_attendees
  ALTER COLUMN rsvp DROP DEFAULT;

ALTER TABLE event_attendees
  ALTER COLUMN rsvp TYPE TEXT USING rsvp::text;

ALTER TABLE event_attendees
  ALTER COLUMN rsvp SET DEFAULT 'pending';

ALTER TABLE event_attendees
  ADD CONSTRAINT event_attendees_rsvp_check
  CHECK (rsvp IN ('pending', 'accepted', 'declined', 'maybe'));

DROP TYPE IF EXISTS rsvp_status;
