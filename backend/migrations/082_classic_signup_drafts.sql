-- Added (TMAIL-374): Server-side draft state for the /classic no-JS signup
-- wizard.
--
-- WHY THIS EXISTS
-- ---------------
-- The Classic UI cannot stash multi-step wizard state in localStorage / sessionStorage
-- (no JavaScript). Carrying it in URL params is rejected by the gap analysis
-- because credentials and password material would leak into history / referrer.
--
-- So the three-step signup flow (Step 1 account → Step 2 IMAP/SMTP → Step 3 done)
-- keeps its in-progress state in this server-side `signup_drafts` table, keyed by
-- an opaque UUID stored in a signed cookie (`tasmail_classic_signup_draft`). Each
-- POST on the wizard reads the cookie, looks up the row, mutates it, and lets the
-- next GET render from the persisted row.
--
-- SCHEMA NOTES
-- ------------
-- * `id` — random UUIDv4. The cookie's secret half (the public half is an HMAC
--   signature over the id, computed by the handler with the JWT_SECRET-derived
--   key — same shape as `classic_sessions`).
-- * `mailbox_id` — set by Step 1 after the Mailbox row has been created. NULL
--   between page render and the first POST; non-NULL from Step 2 onward.
-- * `current_step` — `"account" | "servers" | "done"` driven by the handler.
--   TEXT with a CHECK constraint per the migrations 061/063/065 pattern (no
--   Postgres ENUMs — sqlx decodes ENUMs as `String` only after the
--   ENUM→TEXT-with-CHECK conversion, so we never add new ones).
-- * `csrf_token` — 32 random bytes, base64url-no-pad (43 chars). Re-used on
--   every step of the wizard since the cookie + form pair is constant across
--   the flow. Rotated after each successful step is overkill for a 30-min
--   draft and complicates resumption.
-- * `expires_at` — FIXED at creation + 30 minutes. NOT sliding. A user who
--   walks away from a half-finished wizard for an hour can just start over
--   (no credentials have been written to the mailboxes table until Step 1
--   succeeds; even then the orphaned Mailbox row stays — that's a separate
--   cleanup concern + acceptable for now, see the gap analysis P2 list).
-- * `last_seen_ip` + `last_seen_ua` — same audit fields as `classic_sessions`
--   for the same reasons.
--
-- RLS ALIGNMENT
-- -------------
-- The handlers that read this table run on the PUBLIC sub-router (no user
-- session yet) — they bypass RLS by using the raw `state.db` pool. The
-- belt-and-braces policy below scopes ordinary reads to the linked mailbox
-- so any future admin "in-progress signups" view can resolve safely.

CREATE TABLE classic_signup_drafts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- NULL until Step 1 completes. Set to the freshly-created Mailbox.id then.
    -- ON DELETE CASCADE so a dropped mailbox sweeps its in-progress draft.
    mailbox_id UUID NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    current_step TEXT NOT NULL DEFAULT 'account'
        CHECK (current_step IN ('account', 'servers', 'done')),
    csrf_token TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- FIXED 30-min window — not sliding. A user who walks away starts over.
    expires_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_ip TEXT NULL,
    last_seen_ua TEXT NULL
);

-- Cleanup sweep filters on expiry; partial index keeps the prune off a
-- seq-scan as the table grows. Same shape as `classic_sessions`.
CREATE INDEX idx_classic_signup_drafts_expires_at
    ON classic_signup_drafts(expires_at)
    WHERE expires_at < '2099-01-01';

-- Covering index for the rare admin "in-progress signups for user X" view
-- and for the unique-draft-per-mailbox check the handler does in Step 2/3.
CREATE INDEX idx_classic_signup_drafts_mailbox_id
    ON classic_signup_drafts(mailbox_id)
    WHERE mailbox_id IS NOT NULL;

ALTER TABLE classic_signup_drafts ENABLE ROW LEVEL SECURITY;

-- Owner can read their own draft (for the rare admin "resume signup" flow).
-- The handler itself bypasses RLS via the raw pool — no user session exists
-- yet when the draft is being mutated.
CREATE POLICY classic_signup_drafts_owner_select ON classic_signup_drafts
    FOR SELECT
    USING (
        mailbox_id IS NOT NULL
        AND mailbox_id::text = current_setting('app.current_user_id', true)
    );

-- Admins can read every draft (admin "active signups" view).
CREATE POLICY classic_signup_drafts_admin_all ON classic_signup_drafts
    FOR ALL
    USING (current_setting('app.is_admin', true) = 'true');
