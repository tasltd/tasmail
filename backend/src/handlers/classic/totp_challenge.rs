// Added (TMAIL-361): GET + POST handlers for /classic/login/2fa — the TOTP
// challenge that gates `/classic` logins when the resolved mailbox has
// `totp_enabled = true`.
//
// FLOW (top-level)
// ----------------
// 1. `POST /classic/login` resolves a TOTP-enrolled user. INSTEAD of creating
//    a `classic_sessions` row it:
//      * generates a CSRF token,
//      * creates a `pending_2fa_tokens` row (5-min fixed TTL),
//      * sets the `tasmail_classic_pending_2fa` cookie carrying
//        `<uuid_hex>.<hmac_b64>`,
//      * 303-redirects to `/classic/login/2fa`.
// 2. `GET /classic/login/2fa` reads the cookie, verifies the HMAC signature,
//    looks up the active pending row, and renders `templates/classic/
//    2fa_totp.html` — a 6-digit code field with the row's CSRF token in a
//    hidden `_csrf` input.
// 3. `POST /classic/login/2fa`:
//      * Validates CSRF (form `_csrf` ↔ row's `csrf_token`, constant-time).
//      * Loads the mailbox and verifies the code via `totp_service::verify_totp`.
//      * SUCCESS → delete the pending row, clear the pending cookie, create
//        the real `classic_sessions` row, set its cookie, 303 → INBOX.
//      * FAILURE → increment `failed_attempts`. If the new count reaches
//        `PENDING_2FA_MAX_FAILED_ATTEMPTS`, delete the row, clear the cookie,
//        and bounce to `/classic/login` with a "Too many incorrect codes"
//        flash. Otherwise re-render the 2FA form with a generic error.
//
// Cookie design
// -------------
// `tasmail_classic_pending_2fa = <uuid_simple>.<hmac_b64url>` — same scheme
// as `tasmail_classic_sid` so a stolen DB row alone can't be forged. The
// HMAC key is derived from `JWT_SECRET`. Attributes:
//   * HttpOnly + Secure + SameSite=Strict (tighter than the session cookie
//     because the gate window is short and there's no deep-link use-case).
//   * Path=/classic/login (scoped narrow so it doesn't leak to other routes
//     or collide with the post-login session cookie's Path=/ scope).
//   * Max-Age=300 — server-side row is the source of truth, the browser
//     window is a hint.
//
// Why not piggy-back on classic_session_middleware
// ------------------------------------------------
// The classic_session_middleware bounces ANY missing/invalid cookie to
// /classic/login. The 2FA page sits between login and the inbox; it MUST
// be reachable without a real session. Mounting it on the *public*
// sub-router (alongside `/classic/login`) keeps the existing flow untouched
// and avoids tunneling a "half-session" through the middleware that wasn't
// designed for one.

use askama::Template;
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Extension,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::classic_csrf::validate_csrf_token;
use crate::models::mailbox::Mailbox;
use crate::models::pending_2fa_token::{
    PendingTwoFactorToken, PENDING_2FA_MAX_FAILED_ATTEMPTS, PENDING_2FA_TTL_SECS,
};
use crate::services::totp_service;
use crate::state::AppState;

use super::auth::{create_session_and_cookie, INBOX_PATH, LOGIN_PATH};
use super::CspNonce;

type HmacSha256 = Hmac<Sha256>;

/// Public so the login handler can both set this cookie (after a successful
/// password check on a TOTP-enrolled account) and clear it on bail-out paths.
pub const PENDING_2FA_COOKIE: &str = "tasmail_classic_pending_2fa";

/// Where to send the user when password evaluation has succeeded and TOTP
/// is the next step. Symbolic constant so future renames touch one place.
pub const CHALLENGE_PATH: &str = "/classic/login/2fa";

/// Generic error rendered for any failed code submission (wrong code, empty
/// code, malformed code). Same copy for every branch — the gap analysis
/// calls for the same account-enumeration-blind shape as the credential
/// error on /classic/login.
const GENERIC_CODE_ERROR: &str = "Incorrect verification code.";

/// Distinct error rendered when the per-cookie attempt counter hits the
/// configured max. Surfaced through the login page (not the 2FA page) since
/// the pending token has been deleted and there's no longer a gate to
/// re-render. The wording deliberately mentions "code" rather than the
/// account so an attacker who lacks the password can't tell whether a
/// brute-force was even tried.
pub const TOO_MANY_CODES_ERROR: &str =
    "Too many incorrect verification codes. Please sign in again.";

/// Distinct error for the "cookie expired / missing / forged" case. Same
/// surface as TOO_MANY_CODES — render through /classic/login because the
/// gate is gone.
pub const EXPIRED_OR_INVALID_ERROR: &str =
    "Your verification session expired. Please sign in again.";

// ---------------------------------------------------------------------------
// Cookie helpers — separate module-private HMAC pipeline so a future move to
// signed-cookie crate semantics doesn't touch the session middleware.
// ---------------------------------------------------------------------------

/// HMAC-SHA256 the pending-2FA token id under the JWT secret, base64url-encoded
/// without padding. Same shape as `classic_session::sign_session_id`.
///
/// Changed (TMAIL-381): exposed `pub(super)` so the sibling SMS-OTP challenge
/// module can reuse the pending-2FA cookie envelope. The signature shape is
/// factor-agnostic — only the verification step differs between TOTP and
/// SMS-OTP, so sharing the cookie helpers keeps the gate semantics identical.
pub(super) fn sign_token_id(jwt_secret: &str, token_id: Uuid) -> String {
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes())
        .expect("HMAC-SHA256 accepts keys of any length");
    mac.update(token_id.as_bytes());
    B64URL.encode(mac.finalize().into_bytes())
}

/// Constant-time HMAC verify. Matches the manual ct compare used elsewhere
/// in the Classic surface (we don't pull in `subtle` just for this).
///
/// Changed (TMAIL-381): exposed `pub(super)` so the SMS challenge can run
/// the same cookie-signature check the TOTP challenge does.
pub(super) fn verify_token_signature(jwt_secret: &str, token_id: Uuid, supplied_sig: &str) -> bool {
    let expected = sign_token_id(jwt_secret, token_id);
    if expected.len() != supplied_sig.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in expected.as_bytes().iter().zip(supplied_sig.as_bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Compose the cookie body `<uuid_simple>.<sig_b64url>`. Public so the login
/// handler can set this when it short-circuits to the challenge page.
pub fn build_pending_cookie_value(jwt_secret: &str, token_id: Uuid) -> String {
    format!("{}.{}", token_id.as_simple(), sign_token_id(jwt_secret, token_id))
}

/// Full Set-Cookie header value for a fresh pending-2FA cookie. Public so the
/// login handler can attach it to the 303 redirect.
pub fn build_set_pending_cookie_header(jwt_secret: &str, token_id: Uuid) -> String {
    format!(
        "{PENDING_2FA_COOKIE}={}; HttpOnly; Secure; SameSite=Strict; Path=/classic/login; Max-Age={PENDING_2FA_TTL_SECS}",
        build_pending_cookie_value(jwt_secret, token_id)
    )
}

/// Cookie-clearing Set-Cookie value. Used after a successful TOTP verify, on
/// exhaustion of `MAX_FAILED_ATTEMPTS`, and on cookie expiry / forgery.
pub fn build_clear_pending_cookie_header() -> String {
    format!(
        "{PENDING_2FA_COOKIE}=; HttpOnly; Secure; SameSite=Strict; Path=/classic/login; Max-Age=0"
    )
}

/// Parse the cookie header and pull out `(token_id, signature)`.
///
/// Changed (TMAIL-381): exposed `pub(super)` so the SMS challenge module
/// can reuse the same cookie envelope parser. Keeping a single source of
/// truth for the cookie shape means TOTP and SMS gates can never drift
/// out of step on a cookie-rename / signature-format change.
pub(super) fn extract_pending_cookie(headers: &HeaderMap) -> Option<(Uuid, String)> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    let raw = cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(&format!("{PENDING_2FA_COOKIE}=")))?;
    let (id_part, sig_part) = raw.split_once('.')?;
    let id = Uuid::parse_str(id_part).ok()?;
    Some((id, sig_part.to_string()))
}

// ---------------------------------------------------------------------------
// Template + form
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "classic/2fa_totp.html")]
pub struct TotpChallengeTemplate {
    /// `Some("…")` on a failed code submission; `None` on the fresh GET.
    pub error: Option<String>,
    /// CSRF token bound to the pending row. Rendered into the hidden _csrf
    /// input AND validated against the same row's `csrf_token` column on POST.
    pub csrf_token: String,
    /// Per-request CSP nonce so the inline `<style>` in base.html survives
    /// TMAIL-368's strict CSP.
    pub csp_nonce: String,
}

impl TotpChallengeTemplate {
    /// Changed (TMAIL-368): takes the per-request CSP nonce by argument
    /// rather than generating one internally, so the inline
    /// `<style nonce="…">` on base.html matches the `style-src` source on
    /// the response header that `security_headers_middleware` set.
    fn new(
        error: Option<String>,
        csrf_token: impl Into<String>,
        csp_nonce: impl Into<String>,
    ) -> Self {
        Self {
            error,
            csrf_token: csrf_token.into(),
            csp_nonce: csp_nonce.into(),
        }
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct TotpChallengeForm {
    /// The 6-digit code typed by the user. We accept stray whitespace and
    /// any non-digit character — `verify_totp` will reject anything that
    /// isn't a valid current code; normalising here just keeps the audit
    /// log readable.
    pub code: String,
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// Pull the best-effort client IP + UA for the row's audit fields. Identical
/// shape to `handlers::classic::login::extract_audit_fields` — deliberately
/// duplicated (six lines) to keep the login module's contract narrow.
fn extract_audit_fields(headers: &HeaderMap) -> (Option<String>, Option<String>) {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string());
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(256).collect::<String>());
    (ip, ua)
}

/// Render the challenge form with optional error + the row's CSRF token.
/// Returns the full Response so callers can attach Set-Cookie headers.
fn render_challenge_response(
    status: StatusCode,
    template: TotpChallengeTemplate,
) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic 2FA challenge template render failed: {e}"
        ))
    })?;
    Ok((
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

/// Build the bounce response used when the pending cookie is missing,
/// forged, or expired. Clears the cookie and 303s to /classic/login with
/// an explanatory flash query param so the login page can surface a
/// "your session expired" message without leaking 2FA state.
///
/// We use a query param rather than a server-side flash because the user
/// has no session yet — there's no place to stash state. The login page
/// reads `?error=…` defensively (whitelisted values only) and renders the
/// matching error string.
fn bounce_to_login_with_reason(reason_param: &str) -> Response {
    let url = format!("{LOGIN_PATH}?error={reason_param}");
    let mut resp = Redirect::to(&url).into_response();
    if let Ok(hv) = HeaderValue::from_str(&build_clear_pending_cookie_header()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    resp
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// GET /classic/login/2fa — render the 6-digit code form.
///
/// Pre-conditions are NOT enforced by middleware (this route lives on the
/// public sub-router); we do them inline:
///   1. Read + sig-verify the pending cookie.
///   2. Look up the active pending row.
///   3. Render the form with the row's CSRF token.
///
/// Any failure (cookie missing/forged, row missing/expired) bounces to
/// /classic/login with a query-param flash so the user knows they need to
/// re-enter their password.
pub async fn get_challenge(
    State(state): State<AppState>,
    // Added (TMAIL-368): per-request CSP nonce from the security_headers
    // middleware. Threaded into the challenge template so the inline
    // `<style nonce="…">` on base.html matches the response CSP header.
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let Some((token_id, sig)) = extract_pending_cookie(&headers) else {
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    };

    if !verify_token_signature(&state.config.jwt.secret, token_id, &sig) {
        tracing::warn!(
            ?token_id,
            "classic pending-2FA cookie signature mismatch — possible tampering"
        );
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }

    let pending = match PendingTwoFactorToken::find_active(&state.db, token_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(bounce_to_login_with_reason("2fa_expired")),
        // Database errors bubble up to the global 500 path — the user can
        // retry. Bouncing to login on a transient DB failure would be
        // user-hostile.
        Err(e) => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "pending_2fa_tokens lookup failed: {e}"
            )));
        }
    };

    let template = TotpChallengeTemplate::new(None, pending.csrf_token, csp_nonce.as_str());
    render_challenge_response(StatusCode::OK, template)
}

/// POST /classic/login/2fa — validate CSRF, verify the 6-digit code, and
/// either issue a real session or re-render the form with an error.
pub async fn post_challenge(
    State(state): State<AppState>,
    // Added (TMAIL-368): per-request CSP nonce for the re-render-on-failure
    // path. Bounce branches (Redirect) don't need it; only the wrong-code
    // re-render does.
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<TotpChallengeForm>,
) -> Result<Response, AppError> {
    // 1) Read the cookie. Missing/forged → bounce + clear.
    let Some((token_id, sig)) = extract_pending_cookie(&headers) else {
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    };
    if !verify_token_signature(&state.config.jwt.secret, token_id, &sig) {
        tracing::warn!(
            ?token_id,
            "classic pending-2FA cookie signature mismatch on POST — possible tampering"
        );
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }

    // 2) Look up the active pending row. None → bounce.
    let pending = match PendingTwoFactorToken::find_active(&state.db, token_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(bounce_to_login_with_reason("2fa_expired")),
        Err(e) => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "pending_2fa_tokens lookup failed: {e}"
            )));
        }
    };

    // 3) CSRF: constant-time compare. Fail closed.
    if form.csrf.is_empty() || !validate_csrf_token(&form.csrf, &pending.csrf_token) {
        // CSRF mismatch is a hard "go back to login" — keeps the path simple
        // and avoids re-issuing tokens on a route that may be under attack.
        let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }

    // 4) Load the mailbox so we can read totp_secret. We DO want to defend
    //    against a TOTP-disable race (admin disabled 2FA between password
    //    and code) — if the secret is gone or 2FA was disabled, bounce.
    let mailbox = match Mailbox::find_by_id(&state.db, pending.user_id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            // Mailbox vanished mid-flow — drop the gate and bounce.
            let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
            return Ok(bounce_to_login_with_reason("2fa_expired"));
        }
        Err(e) => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "mailbox lookup for 2FA gate failed: {e}"
            )));
        }
    };
    if !mailbox.totp_enabled || mailbox.totp_secret.is_none() {
        // 2FA was disabled in the gap between password check and code submit.
        // Don't auto-promote — make the user re-auth so the admin-disable
        // action takes effect immediately.
        let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }
    let secret = mailbox.totp_secret.as_deref().unwrap();

    // 5) Verify the code. Normalise stray whitespace; verify_totp itself
    //    rejects malformed input.
    let code = form.code.trim();
    let verified = totp_service::verify_totp(secret, code).unwrap_or(false);

    if !verified {
        // 5a) Wrong code. Bump the counter; if we've hit the limit, invalidate
        //     the row + clear the cookie and bounce to /classic/login.
        let new_count = match PendingTwoFactorToken::increment_failed(&state.db, pending.id).await {
            Ok(n) => n,
            Err(e) => {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "pending_2fa_tokens increment failed: {e}"
                )));
            }
        };
        if new_count >= PENDING_2FA_MAX_FAILED_ATTEMPTS {
            let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
            tracing::warn!(
                user_id = ?pending.user_id,
                attempts = new_count,
                "classic 2FA challenge exhausted attempts — bouncing to login"
            );
            return Ok(bounce_to_login_with_reason("2fa_too_many"));
        }

        // Re-render the form with a generic error. CSRF token stays the same
        // (the same pending row backs it) so the user can immediately retry.
        let template = TotpChallengeTemplate::new(
            Some(GENERIC_CODE_ERROR.to_string()),
            pending.csrf_token,
            csp_nonce.as_str(),
        );
        return render_challenge_response(StatusCode::UNAUTHORIZED, template);
    }

    // 6) Success: pending gate is one-shot, drop the row + clear the cookie,
    //    then create the real classic_sessions row + attach its cookie.
    let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;

    let (ip, ua) = extract_audit_fields(&headers);
    let established =
        create_session_and_cookie(&state, mailbox.id, ip.as_deref(), ua.as_deref()).await?;

    tracing::info!(
        user_id = ?mailbox.id,
        session_id = ?established.session.id,
        "classic 2FA challenge passed; full session created"
    );

    let mut resp = Redirect::to(INBOX_PATH).into_response();
    resp.headers_mut()
        .append(header::SET_COOKIE, established.set_cookie);
    if let Ok(hv) = HeaderValue::from_str(&build_clear_pending_cookie_header()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-jwt-secret-do-not-use";

    #[test]
    fn pending_cookie_has_strict_attributes() {
        let h = build_set_pending_cookie_header(TEST_SECRET, Uuid::nil());
        assert!(h.starts_with(&format!("{PENDING_2FA_COOKIE}=")));
        assert!(h.contains("HttpOnly"), "HttpOnly missing: {h}");
        assert!(h.contains("Secure"), "Secure missing: {h}");
        assert!(
            h.contains("SameSite=Strict"),
            "SameSite=Strict missing: {h}"
        );
        assert!(
            h.contains("Path=/classic/login"),
            "narrow Path scope missing: {h}"
        );
        assert!(h.contains("Max-Age=300"), "Max-Age=300 missing: {h}");
    }

    #[test]
    fn clear_pending_cookie_uses_max_age_zero() {
        let h = build_clear_pending_cookie_header();
        assert!(h.contains("Max-Age=0"), "Max-Age=0 missing: {h}");
        assert!(h.contains("HttpOnly") && h.contains("SameSite=Strict"));
        assert!(h.contains("Path=/classic/login"));
    }

    #[test]
    fn cookie_value_round_trips_through_signature() {
        let id = Uuid::new_v4();
        let body = build_pending_cookie_value(TEST_SECRET, id);
        let (id_part, sig_part) = body.split_once('.').expect("body has uuid.sig shape");
        let parsed = Uuid::parse_str(id_part).expect("uuid hex parses");
        assert_eq!(parsed, id);
        assert!(verify_token_signature(TEST_SECRET, id, sig_part));
    }

    #[test]
    fn signature_verification_rejects_tampered_id() {
        let id = Uuid::new_v4();
        let sig = sign_token_id(TEST_SECRET, id);
        // Same signature but a different id — must fail.
        let other_id = Uuid::new_v4();
        assert_ne!(id, other_id);
        assert!(!verify_token_signature(TEST_SECRET, other_id, &sig));
    }

    #[test]
    fn signature_verification_rejects_wrong_key() {
        let id = Uuid::new_v4();
        let sig = sign_token_id(TEST_SECRET, id);
        assert!(!verify_token_signature("different-secret", id, &sig));
    }

    #[test]
    fn extract_pending_cookie_finds_value() {
        let mut headers = HeaderMap::new();
        let body = "deadbeefdeadbeefdeadbeefdeadbeef.somesig";
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("foo=bar; {PENDING_2FA_COOKIE}={body}; baz=qux"))
                .unwrap(),
        );
        let (id, sig) = extract_pending_cookie(&headers).expect("cookie parsed");
        assert_eq!(id.as_simple().to_string(), "deadbeefdeadbeefdeadbeefdeadbeef");
        assert_eq!(sig, "somesig");
    }

    #[test]
    fn extract_pending_cookie_returns_none_when_absent() {
        let headers = HeaderMap::new();
        assert!(extract_pending_cookie(&headers).is_none());
    }

    #[test]
    fn extract_pending_cookie_returns_none_when_no_dot() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{PENDING_2FA_COOKIE}=just-no-dot-here")).unwrap(),
        );
        assert!(extract_pending_cookie(&headers).is_none());
    }

    #[test]
    fn extract_pending_cookie_returns_none_when_uuid_malformed() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{PENDING_2FA_COOKIE}=notuuid.sig")).unwrap(),
        );
        assert!(extract_pending_cookie(&headers).is_none());
    }

    fn fresh_template() -> TotpChallengeTemplate {
        TotpChallengeTemplate {
            error: None,
            csrf_token: "fixed-csrf-token-for-tests".to_string(),
            csp_nonce: "fixed-nonce-for-tests".to_string(),
        }
    }

    #[test]
    fn challenge_template_renders_form_and_code_input() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains(&format!("action=\"{CHALLENGE_PATH}\"")),
            "form action missing: {body}"
        );
        assert!(body.contains("method=\"post\""), "method=post missing");
        assert!(
            body.contains("name=\"code\"")
                && body.contains("inputmode=\"numeric\"")
                && body.contains("pattern=\"[0-9]"),
            "6-digit code input missing or wrong attrs: {body}"
        );
        assert!(
            body.contains("name=\"_csrf\"")
                && body.contains("value=\"fixed-csrf-token-for-tests\""),
            "hidden _csrf field missing: {body}"
        );
        assert!(
            body.contains("autocomplete=\"one-time-code\""),
            "one-time-code autocomplete hint missing"
        );
    }

    #[test]
    fn challenge_template_omits_error_on_fresh_render() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("role=\"alert\""),
            "alert block must be absent on fresh GET, found: {body}"
        );
    }

    #[test]
    fn challenge_template_renders_error_when_present() {
        let mut t = fresh_template();
        t.error = Some(GENERIC_CODE_ERROR.to_string());
        let body = t.render().expect("template renders");
        assert!(body.contains("role=\"alert\""), "alert role missing");
        assert!(body.contains(GENERIC_CODE_ERROR));
    }

    #[test]
    fn challenge_template_extends_base_layout() {
        // The 2FA page is the SECOND page an unauthenticated user sees — it
        // MUST inherit the accessible base layout (skip-link, nav, main
        // landmark, CSP nonce on inline <style>).
        let body = fresh_template().render().expect("template renders");
        assert!(body.contains("<!DOCTYPE html>"), "missing HTML5 doctype");
        assert!(body.contains("class=\"skip-link\""), "skip-link missing");
        assert!(body.contains("<main id=\"main\""), "<main> landmark missing");
        assert!(
            body.contains("<style nonce=\"fixed-nonce-for-tests\">"),
            "inline <style> must carry the per-request CSP nonce"
        );
    }

    #[test]
    fn challenge_template_has_zero_script_tags() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("<script"),
            "2FA challenge template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn challenge_path_constant_is_under_classic() {
        // Locks the redirect target so a typo can't silently send users
        // to the SPA's /login/2fa route (which doesn't exist).
        assert_eq!(CHALLENGE_PATH, "/classic/login/2fa");
    }

    #[test]
    fn too_many_codes_error_does_not_leak_account_existence() {
        // Lock down the wording so a future "helpful" rewrite ("Account X is
        // now locked") can't accidentally turn this into an enumeration oracle.
        let m = TOO_MANY_CODES_ERROR.to_lowercase();
        assert!(!m.contains("account"), "must not mention account: {TOO_MANY_CODES_ERROR}");
        assert!(!m.contains("does not exist"));
        assert!(!m.contains("no such"));
    }

    #[test]
    fn bounce_response_clears_pending_cookie_and_redirects() {
        let resp = bounce_to_login_with_reason("2fa_expired");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(loc.starts_with(LOGIN_PATH), "redirect target wrong: {loc}");
        assert!(loc.contains("error=2fa_expired"));
        let set_cookies: Vec<_> = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        assert!(
            set_cookies
                .iter()
                .any(|s| s.contains("Max-Age=0") && s.contains(PENDING_2FA_COOKIE)),
            "expected a clear-cookie Set-Cookie header, got: {set_cookies:?}"
        );
    }
}
