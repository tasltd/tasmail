-- Added (TMAIL-361): One-shot 2FA-challenge tokens for the /classic no-JS surface.
--
-- WHY THIS EXISTS
-- ---------------
-- When a Classic-UI login resolves to a user with TOTP enrolled, we must NOT
-- create the full `classic_sessions` row yet — that would defeat the 2FA gate.
-- Instead we issue a short-lived "pending 2FA" handle (this table) that says
-- "this browser has proved knowledge of the password for user X; let them
-- type a 6-digit TOTP code within 5 minutes". Only after a successful
-- `verify_totp` does the login handler create the real session.
--
-- SCHEMA NOTES
-- ------------
-- * `id` is the cookie value's secret half — random UUIDv4 wrapped in an
--   HMAC signature in the `tasmail_classic_pending_2fa` cookie by the
--   handler (same shape as `tasmail_classic_sid` so a leaked DB row alone
--   can't be forged into a cookie).
-- * `user_id` is the mailbox that just passed the password check. FK cascades
--   on mailbox delete so we never leave orphan pending tokens dangling.
-- * `csrf_token` is the form-field token for the 2FA challenge page. The
--   short-lived form's CSRF defence rides on the same OWASP double-submit
--   pattern that login uses (cookie ↔ hidden _csrf field, constant-time
--   compare).
-- * `failed_attempts` increments on every wrong 6-digit code. The handler
--   invalidates the pending token (DELETE) when this hits the configured
--   max, forcing the user back to /classic/login.
-- * `expires_at` is FIXED (NOT sliding) at 5 minutes after creation — the
--   challenge MUST complete within one window so a stolen post-password
--   cookie can't be replayed for hours.
-- * `last_seen_ip` + `last_seen_ua` mirror the `classic_sessions` audit
--   columns so admin "active 2FA challenges" views (P1) can show pending
--   gates without a second table.
--
-- RLS ALIGNMENT
-- -------------
-- Same as `classic_sessions`: the handler that reads this table runs BEFORE
-- rls_context_middleware (it needs the row to know who the user is), so the
-- lookup bypasses RLS via the raw pool. The owner-scoped SELECT policy is
-- belt-and-braces so a future admin "list pending 2FA challenges" view can
-- be wired safely.

CREATE TABLE pending_2fa_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    csrf_token TEXT NOT NULL,
    failed_attempts INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- FIXED 5-min window. The handler does NOT bump this on each request.
    expires_at TIMESTAMPTZ NOT NULL,
    last_seen_ip TEXT NULL,
    last_seen_ua TEXT NULL
);

-- Cleanup sweep + admin view both filter on expiry; partial index keeps the
-- prune query off a seq-scan as the table grows.
CREATE INDEX idx_pending_2fa_tokens_expires_at
    ON pending_2fa_tokens(expires_at)
    WHERE expires_at < '2099-01-01';

-- Covering index for the "list pending challenges per user" admin view and
-- for the DELETE-by-user path the handler uses to clear stale gates when
-- the user re-logs in.
CREATE INDEX idx_pending_2fa_tokens_user_id ON pending_2fa_tokens(user_id);

ALTER TABLE pending_2fa_tokens ENABLE ROW LEVEL SECURITY;

-- Owner can read their own pending tokens. The handler itself bypasses RLS
-- via the raw pool (it has to — RLS context isn't set yet at the 2FA gate).
CREATE POLICY pending_2fa_tokens_owner_select ON pending_2fa_tokens
    FOR SELECT
    USING (user_id::text = current_setting('app.current_user_id', true));
