-- Added (TMAIL-375): Password reset tokens for the /classic no-JS surface.
--
-- WHY THIS EXISTS
-- ---------------
-- The Classic UI cannot use the SPA's "forgot password" flow because that flow
-- doesn't exist (the SPA itself is missing it — see gap-analysis modern-ui.md).
-- Classic users on a stuck-on-2G phone or screen reader still need to recover
-- a forgotten password. The flow is:
--
--   1. GET  /classic/password-reset/request — email field.
--   2. POST /classic/password-reset/request — server resolves the email; if a
--      mailbox exists, generates a 32-byte URL-safe random token, stores
--      SHA-256(token) in this table with a 1-hour TTL, and emails the user a
--      link to `/classic/password-reset/confirm?token=<raw>`. ALWAYS renders a
--      generic "if that address is registered you'll receive an email" page
--      regardless of whether the email exists — prevents account enumeration.
--   3. GET  /classic/password-reset/confirm?token=… — server hashes the
--      incoming token, looks up an UNUSED + UNEXPIRED row; if invalid, renders
--      an "invalid or expired" page; if valid, renders a new+confirm password
--      form.
--   4. POST /classic/password-reset/confirm — validates the token again
--      (single-use, defence-in-depth), updates `mailboxes.password_hash` via
--      `auth_service::hash_password` + `Mailbox::update_password`, marks the
--      row `used_at = now()`, and revokes EVERY existing session for the user
--      (`classic_sessions` + the SPA's `sessions` refresh-token rows) so a
--      stolen-cookie attacker is immediately locked out.
--
-- SCHEMA NOTES
-- ------------
-- * `token_hash` is SHA-256(raw_token) hex-encoded (64 chars). Storing the
--   hash means a DB leak alone can't be turned into a reset link — the
--   attacker also needs the raw 32-byte value, which only ever appears in
--   the outbound email and the user's clipboard.
-- * `user_id` is the mailbox FK with CASCADE so deleting a mailbox auto-
--   cleans pending reset tokens — same shape as `classic_sessions` and
--   `pending_2fa_tokens`.
-- * `expires_at` is FIXED at 1 hour after `created_at` (NOT sliding). One
--   hour is the spec acceptance criterion; longer windows expand the
--   attack surface if the email is intercepted, shorter windows hurt
--   accessibility (e.g. a user on a slow inbox sync).
-- * `used_at` enforces single-use semantics. The lookup query filters
--   `used_at IS NULL` so a replay of a consumed link returns the same
--   "invalid or expired" page as an unknown token — no enumeration of
--   whether the token was ever valid.
-- * `request_ip` + `request_ua` mirror the audit columns on
--   `classic_sessions` / `pending_2fa_tokens` so admin "list pending
--   password resets" views can show who initiated each request.
--
-- RLS ALIGNMENT
-- -------------
-- The handler runs BEFORE rls_context_middleware (the user has no session
-- when they click the reset link), so the lookup query uses the raw pool
-- and bypasses RLS. The owner-scoped policy is belt-and-braces so any
-- future admin / introspection endpoint sees only what it should.

CREATE TABLE password_reset_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    -- SHA-256(raw_token), hex-encoded (64 chars). UNIQUE so a duplicate-token
    -- generation (statistically impossible at 256 bits but defence-in-depth)
    -- fails the INSERT rather than silently overwriting a live row.
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- FIXED 1-hour window per spec.
    expires_at TIMESTAMPTZ NOT NULL,
    -- Single-use marker. NULL while pending; set to now() on first successful
    -- confirm POST.
    used_at TIMESTAMPTZ NULL,
    -- Audit columns — populated from the request that triggered the issue.
    request_ip TEXT NULL,
    request_ua TEXT NULL
);

-- The "list pending resets for this user" admin view + the "invalidate all
-- pending for this user on a fresh request" sweep both filter on user_id.
CREATE INDEX idx_password_reset_tokens_user_id ON password_reset_tokens(user_id);

-- Cleanup sweep (DELETE WHERE expires_at < now() OR used_at IS NOT NULL) hits
-- this partial index instead of a seq-scan.
CREATE INDEX idx_password_reset_tokens_expires_at
    ON password_reset_tokens(expires_at)
    WHERE used_at IS NULL AND expires_at < '2099-01-01';

ALTER TABLE password_reset_tokens ENABLE ROW LEVEL SECURITY;

-- Owner can read their own pending reset rows. The handler itself bypasses
-- RLS via the raw pool (it has to — RLS context isn't set on the reset
-- link's HTTP request because the user has no session yet).
CREATE POLICY password_reset_tokens_owner_select ON password_reset_tokens
    FOR SELECT
    USING (user_id::text = current_setting('app.current_user_id', true));

-- Admins can see / clean all (audit + "force-expire" support workflow).
CREATE POLICY password_reset_tokens_admin_all ON password_reset_tokens
    FOR ALL
    USING (current_setting('app.is_admin', true) = 'true');
