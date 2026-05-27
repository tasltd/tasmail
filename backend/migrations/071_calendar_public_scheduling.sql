-- TMAIL-269 / TMAIL-127: public scheduling link for external participants.
--
-- Owners can flip `public_enabled = true` on a calendar event to publish a
-- booking page at /book/{public_token}. External users hit two public routes:
--
--   GET  /api/calendar/public/{token}           -> event summary
--   POST /api/calendar/public/{token}/rsvp      -> email + name + status
--
-- The token is an unguessable UUIDv4 generated automatically per row. A row
-- starts with `public_enabled = false`, so existing events stay private until
-- the owner explicitly opts in via the CalendarManager UI.

ALTER TABLE calendar_events
  ADD COLUMN public_token   UUID    NOT NULL DEFAULT gen_random_uuid(),
  ADD COLUMN public_enabled BOOLEAN NOT NULL DEFAULT false;

-- The public lookup path is GET /api/calendar/public/{token}, so token must be
-- unique and indexed. UNIQUE constraint also auto-creates the index.
ALTER TABLE calendar_events
  ADD CONSTRAINT calendar_events_public_token_key UNIQUE (public_token);

-- Defense-in-depth: extend the RLS policy so that public-enabled rows are
-- visible even when no app.current_user_id session var is set. The handler
-- already filters by token, but if RLS is ever turned on for the connection
-- user this keeps the public booking page reachable.
DROP POLICY IF EXISTS calendar_events_organizer_policy ON calendar_events;
CREATE POLICY calendar_events_organizer_policy ON calendar_events
    USING (
        organizer_id = current_setting('app.current_user_id', true)::uuid
        OR public_enabled = true
    );
