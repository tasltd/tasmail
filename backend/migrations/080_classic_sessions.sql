-- Added (TMAIL-357): Server-side session table for the /classic no-JS surface.
--
-- WHY THIS EXISTS
-- ---------------
-- The Classic UI cannot manage Bearer tokens — there's no JavaScript to read
-- them from localStorage and attach them to fetch(). Login state therefore has
-- to ride on an HttpOnly cookie. The cookie value is an opaque session id; the
-- mapping from `(session_id) → (user_id, csrf_token, expires_at, ...)` lives
-- in this table so we can:
--   * Invalidate sessions server-side (logout, sign-out-everywhere, password
--     change, admin force-logout) without waiting for a JWT to expire.
--   * Bind a CSRF token to each session, since the Classic surface uses
--     real `<form method="post">` submissions and needs the CSRF defence
--     that the SPA gets for free via Bearer-in-header (cf. TMAIL-358 P0 #4).
--   * Track last-seen IP / UA per session for audit + admin "active sessions"
--     listing (P1 #23 "Sign-out everywhere").
--
-- SCHEMA NOTES
-- ------------
-- * `id` is the cookie value's secret half: a random UUID v4 (128 bits). The
--   cookie itself is a base64url-encoded ed25519-signed wrapper around this
--   id, computed by the middleware (so a stolen DB row alone can't forge a
--   cookie — the attacker also needs the JWT_SECRET-derived signing key).
-- * `user_id` is the mailbox FK. Cascade-deletes on mailbox removal so we
--   never end up with orphan sessions pointing at deleted users.
-- * `csrf_token` is 32 random bytes, base64-standard-encoded (44 chars).
--   Compared in constant time against the `_csrf` form field by the middleware
--   on every state-changing POST.
-- * `expires_at` is the SLIDING expiry — the middleware bumps this forward by
--   24 hours on each authenticated request that lands within the current
--   window. A long-idle session naturally expires; an active one keeps
--   renewing. The cleanup sweep prunes rows past `expires_at`.
-- * `last_seen_ip` + `last_seen_ua` are TEXT (not INET / structured) because
--   they're free-form audit fields, not joinable / filterable. Keep them
--   trim-able to 256 chars to defend against absurd UA strings — enforced
--   by the middleware, not a CHECK constraint (so a legitimate long UA
--   never breaks login).
--
-- RLS ALIGNMENT
-- -------------
-- The middleware that reads this table runs BEFORE rls_context_middleware
-- (it needs the row to know who the user is), so the lookup query has to
-- bypass RLS. We achieve that the same way the existing JWT auth path does:
-- the lookup uses the raw `state.db` pool, not the RLS-primed `RlsConn`, and
-- the policy below scopes "ordinary" reads to the owning user. Maintenance
-- sweeps (cleanup_expired) run as the migration owner role and bypass RLS by
-- default per Postgres semantics.

CREATE TABLE classic_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    csrf_token TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Sliding expiry: middleware bumps this on each request inside the window.
    expires_at TIMESTAMPTZ NOT NULL,
    -- Audit fields — updated on each authenticated hit by the middleware.
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_ip TEXT NULL,
    last_seen_ua TEXT NULL
);

-- One row per cookie; PK lookup is the hot path. Add a covering index on
-- user_id so the "sign out everywhere" + admin "list sessions" queries
-- (P1 #23) don't seq-scan as the table grows.
CREATE INDEX idx_classic_sessions_user_id ON classic_sessions(user_id);

-- Partial index on expires_at lets the cleanup sweep (`DELETE FROM
-- classic_sessions WHERE expires_at < now()`) hit a small targeted index
-- rather than scanning the whole table.
CREATE INDEX idx_classic_sessions_expires_at
    ON classic_sessions(expires_at)
    WHERE expires_at < '2099-01-01';

ALTER TABLE classic_sessions ENABLE ROW LEVEL SECURITY;

-- Owner can read their own sessions (used by P1 #23's "active sessions" view).
-- The middleware itself does NOT rely on this policy — its lookup runs before
-- RLS context is established and bypasses the policy via the raw pool.
CREATE POLICY classic_sessions_owner_select ON classic_sessions
    FOR SELECT
    USING (user_id = current_setting('app.current_user_id', true)::uuid);

-- Owner can revoke their own sessions (logout / sign-out-everywhere).
CREATE POLICY classic_sessions_owner_delete ON classic_sessions
    FOR DELETE
    USING (user_id = current_setting('app.current_user_id', true)::uuid);

-- Admins can read all sessions (force-logout / audit).
CREATE POLICY classic_sessions_admin_all ON classic_sessions
    FOR ALL
    USING (current_setting('app.is_admin', true) = 'true');
