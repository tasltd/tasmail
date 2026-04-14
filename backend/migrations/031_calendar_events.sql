-- Added: Calendar events and attendee tables for meeting scheduling (TMAIL-127)

CREATE TYPE event_status AS ENUM ('tentative', 'confirmed', 'cancelled');
CREATE TYPE rsvp_status AS ENUM ('pending', 'accepted', 'declined', 'maybe');

CREATE TABLE calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organizer_id UUID NOT NULL REFERENCES users(id),
    title TEXT NOT NULL,
    description TEXT,
    location TEXT,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    all_day BOOLEAN NOT NULL DEFAULT false,
    recurrence_rule TEXT,
    status event_status NOT NULL DEFAULT 'tentative',
    linked_message_uid INTEGER,
    linked_folder TEXT,
    ics_uid TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_calendar_events_organizer ON calendar_events(organizer_id);
CREATE INDEX idx_calendar_events_time ON calendar_events(start_time, end_time);

CREATE TABLE event_attendees (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    display_name TEXT,
    rsvp rsvp_status NOT NULL DEFAULT 'pending',
    responded_at TIMESTAMPTZ
);
CREATE INDEX idx_event_attendees_event ON event_attendees(event_id);

ALTER TABLE calendar_events ENABLE ROW LEVEL SECURITY;
CREATE POLICY calendar_events_organizer_policy ON calendar_events
    USING (organizer_id = current_setting('app.current_user_id')::uuid);
