-- Added (TMAIL-386): Per-user, per-sender allowlist for remote `<img>`
-- rendering in the Classic UI message view.
--
-- WHY THIS EXISTS
-- ---------------
-- The HTML sanitiser (`services::html_sanitizer`, P0 #10 / TMAIL-364) blocks
-- every remote `<img src="http(s)://...">` by default — rewrites the URL to a
-- 1×1 transparent placeholder so tracking-pixel beacons are dead on arrival.
-- The Classic UI message read view (TMAIL-363 / TMAIL-386) surfaces two opt-
-- ins above the body when remote images were blocked:
--
--   1. "Show images" — one-shot. POSTs and re-renders THIS view only. No DB
--      write. Implemented as a `?show_images=1` query param honoured by the
--      `get_message` handler.
--   2. "Always show images from this sender" — persistent. INSERTs a row in
--      this table keyed on the lower-cased sender email address. On every
--      subsequent render of any message FROM that address, the handler
--      consults the allowlist and asks the sanitiser to surface the real
--      remote URL.
--
-- SCHEMA NOTES
-- ------------
-- * `mailbox_id` — owning user. ON DELETE CASCADE so a dropped mailbox
--   sweeps its allowlist (RLS + cleanliness).
-- * `sender_address` — lower-cased, trimmed email address. Handlers MUST
--   normalise on insert + on lookup (`.to_ascii_lowercase().trim()`) so the
--   `(mailbox_id, sender_address)` UNIQUE constraint actually matches case-
--   variant senders. TEXT — addresses are short, no need to bound length
--   here beyond Postgres' default ~1 GB TEXT cap; a malformed jumbo address
--   will fail the application-side parser long before it reaches this column.
-- * `created_at` — for the future "manage allowlist" settings page (P2). Not
--   used by the render path.
--
-- The handler that writes this row also performs application-side validation
-- (parser must produce a non-empty `local@domain.tld`). The DB CHECK below
-- is a thin safety net — `LIKE '%@%'` + non-empty so a coding bug can't
-- persist a junk row that breaks lookups.
--
-- RLS ALIGNMENT
-- -------------
-- Standard tenant-scoped policy: a user reads / writes / deletes only their
-- own allowlist rows. The auth middleware sets `app.current_user_id` per
-- request, the SELECT/INSERT/DELETE policies below compare `mailbox_id` to
-- that session GUC, matching the pattern used by `signatures`, `contacts`,
-- and every other per-user table in this schema.

CREATE TABLE remote_image_allowlist (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    -- Normalised email address: lower-cased, trimmed. Application code must
    -- match this contract — see `models::remote_image_allowlist::normalise`.
    sender_address TEXT NOT NULL
        CHECK (length(sender_address) > 0 AND sender_address LIKE '%@%'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- A user can only allow a given sender once. Upserts (TMAIL-386's
    -- "Always show images" form) use ON CONFLICT DO NOTHING against this
    -- unique constraint so re-clicking the button is a safe no-op.
    UNIQUE (mailbox_id, sender_address)
);

-- Hot path: the read view consults `(mailbox_id, sender_address)` on every
-- message render. The UNIQUE above already covers this exact lookup, so no
-- additional composite index is needed.

-- Covering index for the future settings page that lists every allowed
-- sender for a user, sorted newest-first. Cheap on this small table.
CREATE INDEX idx_remote_image_allowlist_mailbox_created_at
    ON remote_image_allowlist(mailbox_id, created_at DESC);

ALTER TABLE remote_image_allowlist ENABLE ROW LEVEL SECURITY;

-- Owner read.
CREATE POLICY remote_image_allowlist_owner_select ON remote_image_allowlist
    FOR SELECT
    USING (mailbox_id::text = current_setting('app.current_user_id', true));

-- Owner write (insert) — the "Always show images" handler runs after the
-- auth middleware sets `app.current_user_id`, so the inserted row's
-- mailbox_id MUST match the session GUC.
CREATE POLICY remote_image_allowlist_owner_insert ON remote_image_allowlist
    FOR INSERT
    WITH CHECK (mailbox_id::text = current_setting('app.current_user_id', true));

-- Owner delete — for the future settings page's per-row "Remove" button.
CREATE POLICY remote_image_allowlist_owner_delete ON remote_image_allowlist
    FOR DELETE
    USING (mailbox_id::text = current_setting('app.current_user_id', true));

-- Admins can see every allowlist row for any tenant-wide audit / support
-- view. Matches the admin pattern on `signatures`, `contacts`, etc.
CREATE POLICY remote_image_allowlist_admin_all ON remote_image_allowlist
    FOR ALL
    USING (current_setting('app.is_admin', true) = 'true');
