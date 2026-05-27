-- Added (TMAIL-127): Scope calendar_events.ics_uid uniqueness per organizer.
--
-- The original migration 031 declared `ics_uid TEXT NOT NULL UNIQUE` which is a
-- global constraint. That works for a single-tenant install but breaks the
-- inbound iMIP accept flow: when two different mailboxes receive the same
-- VEVENT (organizer + CC attendee on the same TASMail instance), only the
-- first user to click Accept can persist the event — the second hits the
-- global unique violation.
--
-- RFC 5545 §3.8.4.7 only requires UID to be globally unique *per organizer*,
-- so the correct constraint is composite (organizer_id, ics_uid). That also
-- lets the imip_parser upsert flow use a proper ON CONFLICT target.

ALTER TABLE calendar_events DROP CONSTRAINT IF EXISTS calendar_events_ics_uid_key;
ALTER TABLE calendar_events
    ADD CONSTRAINT calendar_events_organizer_ics_uid_key UNIQUE (organizer_id, ics_uid);
