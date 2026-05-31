// Added (TMAIL-357): Cookie-based session middleware for the `/classic` no-JS
// surface.
//
// WHY THIS EXISTS
// ---------------
// The SPA's `auth_middleware` expects `Authorization: Bearer <jwt>`, which is
// unreachable without JavaScript to read the token from localStorage and
// attach it to fetch(). The Classic UI uses an HttpOnly cookie instead:
//
//     tasmail_classic_sid = <session_id_uuid_hex>.<hmac_base64url>
//          HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=86400
//
// The signature is HMAC-SHA256 over the session id, keyed off the JWT secret.
// That way a stolen `classic_sessions` row alone can't be used to forge a
// cookie — the attacker also needs the JWT_SECRET-derived key. (Logically
// equivalent to axum-extra's `SignedCookieJar`, just without the `cookie::Key`
// plumbing through AppState.)
//
// FLOW
// ----
// 1. Parse cookie header → extract `tasmail_classic_sid`.
// 2. Split on `.` → `(session_id_hex, hmac_b64)`. Reject if missing either.
// 3. Verify HMAC; reject on mismatch (constant-time compare). On any
//    rejection, *clear* the cookie on the response so the browser stops
//    sending the stale value, and bounce to `/classic/login`.
// 4. `ClassicSession::find_active(pool, session_id)` — returns None for
//    expired/missing rows. None → same redirect-to-login path.
// 5. Load the owning `Mailbox` for AuthUser context. Build the same `Claims`
//    struct the JWT path injects, so handlers + the future shared CSRF
//    extractor see one consistent shape.
// 6. `touch()` bumps `expires_at` + `last_seen_*` on the session row.
// 7. Inject `Claims` + `ClassicSession` into request extensions for handlers
//    and templates (CSRF token lives on the session row).
// 8. Re-issue the cookie with the same id and a refreshed Max-Age so the
//    browser-side window stays in sync with the server-side sliding expiry.
//
// PUBLIC PATHS
// ------------
// `/classic/login`, `/classic/signup`, the static `/classic/` redirect, and
// the assets path don't need a session. The middleware is therefore wired
// in on the AUTHENTICATED sub-router only (see router.rs), not as a global
// layer over the whole `/classic/*` tree. Public handlers stay free to
// render their own templates without a Claims extension.

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppError;
use crate::handlers::classic::CLASSIC_SESSION_COOKIE;
use crate::models::classic_session::ClassicSession;
use crate::models::mailbox::Mailbox;
use crate::services::auth_service::Claims;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Where to send a user whose cookie is missing / forged / expired. Centralised
/// so login/logout handlers can target the same path symbolically.
const LOGIN_PATH: &str = "/classic/login";

/// Compute the HMAC-SHA256 signature for a session id, returned base64url
/// (no padding) so it's safe inside a cookie value without further escaping.
fn sign_session_id(jwt_secret: &str, session_id: Uuid) -> String {
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes())
        .expect("HMAC-SHA256 accepts keys of any length");
    mac.update(session_id.as_bytes());
    let sig = mac.finalize().into_bytes();
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
    B64URL.encode(sig)
}

/// Verify an HMAC signature against an expected session id in constant time.
fn verify_signature(jwt_secret: &str, session_id: Uuid, supplied_sig_b64: &str) -> bool {
    let expected = sign_session_id(jwt_secret, session_id);
    // `Hmac::verify` requires recomputing through MAC state; comparing the
    // base64-encoded form is fine as long as the comparison is constant-time.
    // `subtle::ConstantTimeEq` would be ideal, but we don't pull it in just for
    // this. Manual ct compare:
    if expected.len() != supplied_sig_b64.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in expected.as_bytes().iter().zip(supplied_sig_b64.as_bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Build the `tasmail_classic_sid` cookie value `<uuid_hex>.<sig_b64url>` that
/// goes into the Set-Cookie response header.
pub fn build_cookie_value(jwt_secret: &str, session_id: Uuid) -> String {
    let sig = sign_session_id(jwt_secret, session_id);
    format!("{}.{}", session_id.as_simple(), sig)
}

/// Build the full Set-Cookie header value for a fresh / refreshed session.
/// `max_age_secs` is what the browser will count down — kept in sync with the
/// server-side sliding window so both expire together.
///
/// Cookie attributes:
///   * HttpOnly — JavaScript can never read it (defence against XSS exfil).
///   * Secure — only sent over HTTPS (irrelevant on localhost but mandatory
///     in production behind the Apache vhost).
///   * SameSite=Lax — submits cross-origin GETs (so a deep link from email
///     lands logged in) but blocks cross-origin POSTs (CSRF defence layer 1;
///     the `_csrf` form field is layer 2, added by TMAIL-358).
///   * Path=/ — visible to every path. The cookie is name-prefixed
///     (`tasmail_classic_*`) to make collisions with future cookies obvious.
pub fn build_set_cookie_header(jwt_secret: &str, session_id: Uuid, max_age_secs: i64) -> String {
    format!(
        "{}={}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={}",
        CLASSIC_SESSION_COOKIE,
        build_cookie_value(jwt_secret, session_id),
        max_age_secs
    )
}

/// Header value that clears the cookie. Used on logout and when the middleware
/// rejects an invalid cookie, so the browser stops sending the stale value.
pub fn build_clear_cookie_header() -> String {
    format!(
        "{}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0",
        CLASSIC_SESSION_COOKIE
    )
}

/// Parse the request's Cookie header and pull out the `tasmail_classic_sid`
/// value as `(session_id, signature)`. Returns None if the cookie is absent
/// or malformed — the middleware treats those identically (bounce to login).
fn extract_session_cookie(req: &Request) -> Option<(Uuid, String)> {
    let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    let raw = cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(&format!("{}=", CLASSIC_SESSION_COOKIE)))?;
    let (id_part, sig_part) = raw.split_once('.')?;
    let id = Uuid::parse_str(id_part).ok()?;
    Some((id, sig_part.to_string()))
}

/// Pull the best-effort client IP for audit fields. The proxy chain sets
/// `X-Forwarded-For`; we take the first hop only. Falls back to the direct
/// peer's TCP address (which axum doesn't expose in middleware without a
/// `ConnectInfo` extractor — out of scope for this task).
fn extract_audit_fields(req: &Request) -> (Option<String>, Option<String>) {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string());
    let ua = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        // Trim absurd UA strings to a sane bound — the column is TEXT but a
        // 10 KB UA from a fuzzer doesn't belong in our audit log.
        .map(|s| s.chars().take(256).collect::<String>());
    (ip, ua)
}

/// Bounce response used for every "no valid session" branch. Clears the stale
/// cookie on its way back so the browser doesn't keep re-sending it.
fn bounce_to_login() -> Response {
    let mut resp = Redirect::to(LOGIN_PATH).into_response();
    if let Ok(hv) = HeaderValue::from_str(&build_clear_cookie_header()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    resp
}

/// The middleware itself. Wire onto authenticated `/classic/*` routes only —
/// `/classic/login` and the catch-all 404 should NOT be behind this layer.
pub async fn classic_session_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 1 — Extract cookie. Missing/malformed → bounce.
    let Some((session_id, sig)) = extract_session_cookie(&req) else {
        return Ok(bounce_to_login());
    };

    // 2 — Verify signature in constant time. Forged → bounce + clear cookie.
    if !verify_signature(&state.config.jwt.secret, session_id, &sig) {
        tracing::warn!(
            ?session_id,
            "classic session cookie signature mismatch — possible tampering"
        );
        return Ok(bounce_to_login());
    }

    // 3 — Look up the row. Missing/expired → bounce + clear cookie.
    let Some(session) = ClassicSession::find_active(&state.db, session_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic session lookup failed: {e}")))?
    else {
        return Ok(bounce_to_login());
    };

    // 4 — Load the user for AuthUser context. A session whose user has been
    // deleted (mailbox CASCADE will normally clean this up first, but a race
    // is conceivable) → bounce.
    let Some(mailbox) = Mailbox::find_by_id(&state.db, session.user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic session user lookup failed: {e}")))?
    else {
        return Ok(bounce_to_login());
    };

    // Also reject inactive accounts up-front — same path as the JWT login.
    if !mailbox.active {
        return Ok((StatusCode::FORBIDDEN, "Account inactive").into_response());
    }

    // 5 — Build the same Claims shape the JWT path injects, so handlers /
    // the future shared CSRF extractor see one consistent type. `exp` and
    // `iat` here reflect the *session* window, not a JWT lifetime — handlers
    // that need real JWT timing should always go through `auth_middleware`.
    let now = chrono::Utc::now();
    let claims = Claims {
        sub: mailbox.id.to_string(),
        username: mailbox.username.clone(),
        is_admin: mailbox.is_admin,
        is_compliance_officer: mailbox.is_compliance_officer,
        iat: now.timestamp() as usize,
        exp: session.expires_at.timestamp() as usize,
    };

    // 6 — Bump sliding expiry + audit fields. Failure here shouldn't kill
    // the request (we already authenticated the user); log and proceed.
    let (ip, ua) = extract_audit_fields(&req);
    if let Err(e) = ClassicSession::touch(&state.db, session.id, ip.as_deref(), ua.as_deref()).await {
        tracing::warn!(?e, ?session_id, "classic session touch failed; continuing");
    }

    // 7 — Inject for downstream handlers + templates.
    req.extensions_mut().insert(claims);
    req.extensions_mut().insert(session.clone());

    // 8 — Run the handler, then re-issue the cookie with a fresh Max-Age so
    // the browser's expiry tracks the server-side sliding window. We don't
    // change the session id — same row, same signature, just refreshed TTL.
    let mut resp = next.run(req).await;
    let max_age = chrono::Duration::hours(
        crate::models::classic_session::CLASSIC_SESSION_TTL_HOURS,
    )
    .num_seconds();
    if let Ok(hv) = HeaderValue::from_str(&build_set_cookie_header(
        &state.config.jwt.secret,
        session.id,
        max_age,
    )) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    Ok(resp)
}

/// Lightweight accessor: pull the Claims a handler / template needs. Mirrors
/// `middleware::auth::extract_claims` so call sites under `/classic` look
/// identical to call sites under `/api`.
#[allow(dead_code)]
pub fn extract_claims(req: &Request) -> Result<&Claims, AppError> {
    req.extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("No classic session claims in request".to_string()))
}

/// Accessor for the live `ClassicSession` row — handlers use this to read
/// the CSRF token for form rendering and to validate inbound `_csrf` form
/// fields (the validation itself ships in TMAIL-358 P0 #4).
#[allow(dead_code)]
pub fn extract_session(req: &Request) -> Result<&ClassicSession, AppError> {
    req.extensions()
        .get::<ClassicSession>()
        .ok_or_else(|| AppError::Unauthorized("No classic session row in request".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    const TEST_SECRET: &str = "test-secret-do-not-reuse-7Hkj2";

    #[test]
    fn signature_roundtrips_through_verify() {
        let id = Uuid::new_v4();
        let sig = sign_session_id(TEST_SECRET, id);
        assert!(verify_signature(TEST_SECRET, id, &sig));
    }

    #[test]
    fn signature_rejects_wrong_secret() {
        let id = Uuid::new_v4();
        let sig = sign_session_id(TEST_SECRET, id);
        assert!(!verify_signature("different-secret", id, &sig));
    }

    #[test]
    fn signature_rejects_wrong_id() {
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let sig_a = sign_session_id(TEST_SECRET, id_a);
        assert!(!verify_signature(TEST_SECRET, id_b, &sig_a));
    }

    #[test]
    fn signature_rejects_truncated_sig() {
        let id = Uuid::new_v4();
        let sig = sign_session_id(TEST_SECRET, id);
        let truncated = &sig[..sig.len() - 1];
        assert!(!verify_signature(TEST_SECRET, id, truncated));
    }

    #[test]
    fn signature_rejects_empty_sig() {
        let id = Uuid::new_v4();
        assert!(!verify_signature(TEST_SECRET, id, ""));
    }

    #[test]
    fn signature_is_url_safe_base64_no_pad() {
        // Cookies don't need extra escaping for URL-safe base64; assert the
        // alphabet so a future engine swap doesn't introduce `+`/`/` chars
        // that some old browsers / proxies have historically tripped on.
        let id = Uuid::new_v4();
        let sig = sign_session_id(TEST_SECRET, id);
        for ch in sig.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "non-URL-safe-base64 character {:?} in signature {:?}",
                ch,
                sig
            );
        }
    }

    #[test]
    fn cookie_value_has_expected_shape() {
        let id = Uuid::new_v4();
        let v = build_cookie_value(TEST_SECRET, id);
        let (id_part, sig_part) = v.split_once('.').expect("cookie has a `.`");
        // Simple-form UUID = 32 hex chars (no dashes).
        assert_eq!(id_part.len(), 32);
        assert!(id_part.chars().all(|c| c.is_ascii_hexdigit()));
        // Signature non-empty.
        assert!(!sig_part.is_empty());
    }

    #[test]
    fn set_cookie_header_includes_security_attributes() {
        let id = Uuid::new_v4();
        let header = build_set_cookie_header(TEST_SECRET, id, 86400);
        assert!(header.contains("HttpOnly"), "HttpOnly missing: {header}");
        assert!(header.contains("Secure"), "Secure missing: {header}");
        assert!(header.contains("SameSite=Lax"), "SameSite=Lax missing: {header}");
        assert!(header.contains("Path=/"), "Path=/ missing: {header}");
        assert!(header.contains("Max-Age=86400"), "Max-Age missing: {header}");
        assert!(
            header.starts_with(&format!("{}=", CLASSIC_SESSION_COOKIE)),
            "cookie name prefix missing: {header}"
        );
    }

    #[test]
    fn clear_cookie_header_uses_max_age_zero() {
        let h = build_clear_cookie_header();
        assert!(h.contains("Max-Age=0"), "expected clear cookie Max-Age=0: {h}");
        assert!(h.contains("HttpOnly") && h.contains("Secure") && h.contains("SameSite=Lax"));
    }

    /// Build a minimal axum Request with a cookie header for the parser.
    fn req_with_cookie(value: &str) -> Request {
        let mut req = Request::new(Body::empty());
        req.headers_mut().insert(
            header::COOKIE,
            HeaderValue::from_str(value).expect("test cookie header parses"),
        );
        req
    }

    #[test]
    fn extracts_well_formed_cookie() {
        let id = Uuid::new_v4();
        let v = build_cookie_value(TEST_SECRET, id);
        let req = req_with_cookie(&format!("{}={}", CLASSIC_SESSION_COOKIE, v));
        let (got_id, got_sig) = extract_session_cookie(&req).expect("extraction succeeds");
        assert_eq!(got_id, id);
        assert!(verify_signature(TEST_SECRET, got_id, &got_sig));
    }

    #[test]
    fn extracts_cookie_among_other_cookies() {
        let id = Uuid::new_v4();
        let v = build_cookie_value(TEST_SECRET, id);
        let req = req_with_cookie(&format!(
            "session_id=abc; {}={}; theme=dark",
            CLASSIC_SESSION_COOKIE, v
        ));
        let (got_id, _) = extract_session_cookie(&req).expect("extraction succeeds");
        assert_eq!(got_id, id);
    }

    #[test]
    fn returns_none_when_cookie_missing() {
        let req = req_with_cookie("other=foo");
        assert!(extract_session_cookie(&req).is_none());
    }

    #[test]
    fn returns_none_when_no_dot_separator() {
        let req = req_with_cookie(&format!("{}=not-a-cookie-shape", CLASSIC_SESSION_COOKIE));
        assert!(extract_session_cookie(&req).is_none());
    }

    #[test]
    fn returns_none_when_id_part_is_not_uuid() {
        let req = req_with_cookie(&format!(
            "{}=not-a-uuid.somesignature",
            CLASSIC_SESSION_COOKIE
        ));
        assert!(extract_session_cookie(&req).is_none());
    }

    #[test]
    fn audit_fields_pick_first_forwarded_ip_and_truncate_ua() {
        let mut req = Request::new(Body::empty());
        req.headers_mut().insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.5, 10.0.0.1, 10.0.0.2"),
        );
        let long_ua = "X".repeat(1024);
        req.headers_mut()
            .insert(header::USER_AGENT, HeaderValue::from_str(&long_ua).unwrap());
        let (ip, ua) = extract_audit_fields(&req);
        assert_eq!(ip.as_deref(), Some("203.0.113.5"));
        // UA truncated to 256 chars.
        assert_eq!(ua.as_deref().map(str::len), Some(256));
    }

    #[test]
    fn audit_fields_handle_missing_headers() {
        let req = Request::new(Body::empty());
        let (ip, ua) = extract_audit_fields(&req);
        assert!(ip.is_none());
        assert!(ua.is_none());
    }
}
