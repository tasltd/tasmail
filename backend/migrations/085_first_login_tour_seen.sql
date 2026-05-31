-- TMAIL-401: per-mailbox flag for the first-login product tour.
-- Default false so freshly-signed-up users see the tour on first AppShell
-- render. The PATCH handler flips it to true on dismiss and the flag stays
-- true forever after that.

ALTER TABLE mailboxes
    ADD COLUMN IF NOT EXISTS first_login_tour_seen BOOLEAN NOT NULL DEFAULT false;
