// Added (TMAIL-359): GET + POST handlers for /classic/login on the no-JS
// surface.
//
// Why this lives in its own module
// --------------------------------
// `handlers/classic/auth.rs` (TMAIL-357) owns the session-management
// primitives (`generate_csrf_token`, `create_session_and_cookie`,
// `destroy_session_and_cookie`). This file owns the user-visible login flow
// that sits on top of them. Splitting the two keeps the "session glue"
// reusable for SAML/OIDC callbacks (P1 #28) and the password-change
// re-auth path (P1 #22) without making them depend on the form rendering.
//
// CSRF on the login form — the chicken-and-egg
// --------------------------------------------
// The canonical CSRF middleware (TMAIL-358) needs a `ClassicSession` row to
// know the expected token. The login form runs BEFORE any session exists.
// To still get CSRF coverage we use the OWASP double-submit-cookie pattern:
//
//   * `GET /classic/login` issues a short-lived pre-session cookie
//     `tasmail_classic_login_csrf=<token>; HttpOnly; SameSite=Strict;
//      Secure; Path=/classic/login; Max-Age=900` AND renders the same token
//     into the form as the hidden `_csrf` input.
//   * `POST /classic/login` validates that the cookie token byte-equals the
//     form token (constant-time compare). Mismatch / missing → render the
//     same form with a generic error.
//
// SameSite=Strict on the pre-session cookie prevents a cross-site form
// submission from carrying it. Path=/classic/login scopes it tight so it
// can't collide with the post-login `tasmail_classic_sid` cookie.
//
// Lockout-aware error rendering
// -----------------------------
// `evaluate_password_login` returns `AppError::Unauthorized` for a bad
// password and `AppError::AccountLocked` for an in-effect lockout. The gap
// analysis (P0 #5) says to render a generic "incorrect email or password"
// for the simple bad-password case while still bumping the counter — the
// extra lockout-countdown copy lands in P1 #31. We honour that here: both
// `Unauthorized` and `AccountLocked` surface the same generic message,
// avoiding account enumeration AND deferring the friendlier lockout copy.

use askama::Template;
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
};

use crate::error::AppError;
use crate::middleware::classic_csrf::validate_csrf_token;
use crate::models::pending_2fa_token::PendingTwoFactorToken;
use crate::services::auth_service::evaluate_password_login;
use crate::state::AppState;

use super::auth::{create_session_and_cookie, generate_csrf_token, INBOX_PATH, LOGIN_PATH};
use super::totp_challenge::{
    build_set_pending_cookie_header, CHALLENGE_PATH, EXPIRED_OR_INVALID_ERROR,
    TOO_MANY_CODES_ERROR,
};
use super::CspNonce;

/// Pre-session cookie name. Distinct from `tasmail_classic_sid` so the two
/// never collide (different Path scope makes this belt-and-braces).
pub const LOGIN_CSRF_COOKIE: &str = "tasmail_classic_login_csrf";

/// How long the pre-session CSRF cookie stays valid. 15 minutes covers a
/// user who opens the login page, walks away, comes back and types their
/// credentials without forcing a retry — but it's short enough that a
/// leaked cookie isn't useful for a determined attacker.
const LOGIN_CSRF_TTL_SECS: i64 = 900;

/// Generic message rendered for ANY failure mode that shouldn't leak whether
/// the email exists, whether the password was correct, or whether the
/// account is currently in lockout. The P1 lockout-countdown work (#31)
/// adds a separate friendlier branch for known-locked accounts; until then
/// every failure looks the same.
const GENERIC_LOGIN_ERROR: &str = "Incorrect email or password.";

/// Message rendered when the CSRF token cookie / form pairing fails. Kept
/// distinct from the credential error so a user whose cookie was stripped
/// by an over-zealous extension can self-diagnose. Doesn't leak any
/// account-existence signal.
const CSRF_ERROR_MESSAGE: &str =
    "Your session expired before you submitted the form. Please try again.";

/// Askama template struct backing `templates/classic/login.html`.
///
/// Field names must match the template `{{ var }}` placeholders exactly —
/// Askama validates this at compile time so a rename here without the
/// matching template edit fails `cargo build`.
#[derive(Template)]
#[template(path = "classic/login.html")]
pub struct LoginTemplate {
    /// Pre-fill the email field across a failed POST so the user doesn't
    /// retype it. Always HTML-escaped by Askama's `.html`-extension
    /// auto-escape. Empty string on a fresh GET.
    pub email: String,
    /// `Some("…")` on a failed login or CSRF rejection; `None` on a fresh
    /// GET. Rendered inside a `role="alert"` block.
    pub error: Option<String>,
    /// The pre-session CSRF token that also sits in the
    /// `tasmail_classic_login_csrf` cookie.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html (TMAIL-356).
    pub csp_nonce: String,
}

impl LoginTemplate {
    /// Build a fresh template with no error and an empty email. Used by
    /// `GET /classic/login` and by the POST handler when it needs to
    /// re-render the form on failure.
    fn new(email: impl Into<String>, error: Option<String>, csrf_token: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            error,
            csrf_token: csrf_token.into(),
            csp_nonce: CspNonce::new().into_string(),
        }
    }
}

/// Body shape for the POST form. axum's built-in `Form` extractor uses
/// `serde_urlencoded` under the hood, which handles `application/x-www-form-
/// urlencoded` correctly.
#[derive(serde::Deserialize, Debug)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    /// Hidden CSRF input from the rendered form. Matches the cookie value
    /// set by `GET /classic/login`.
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// Build the pre-session CSRF cookie header value. Strict SameSite means
/// the browser will refuse to send this cookie alongside a cross-origin
/// POST — layer-1 CSRF defence. The `_csrf` form-field check is layer 2.
///
/// Path=/classic/login scopes the cookie tight so it doesn't leak to other
/// classic routes (and doesn't fight with the post-login session cookie's
/// Path=/ scope).
fn build_login_csrf_cookie(token: &str) -> String {
    format!(
        "{LOGIN_CSRF_COOKIE}={token}; HttpOnly; Secure; SameSite=Strict; Path=/classic/login; Max-Age={LOGIN_CSRF_TTL_SECS}"
    )
}

/// Clear the pre-session CSRF cookie. Called after a SUCCESSFUL login (the
/// token is one-shot) and on the next GET render so a stale token can't be
/// replayed against a fresh form.
fn build_clear_login_csrf_cookie() -> String {
    format!(
        "{LOGIN_CSRF_COOKIE}=; HttpOnly; Secure; SameSite=Strict; Path=/classic/login; Max-Age=0"
    )
}

/// Pull the pre-session CSRF cookie value out of the request headers.
/// Returns None if the cookie is absent or malformed — the POST handler
/// treats that identically (re-render with the CSRF error message).
fn extract_login_csrf_cookie(headers: &HeaderMap) -> Option<String> {
    let header_val = headers.get(header::COOKIE)?.to_str().ok()?;
    header_val
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(&format!("{LOGIN_CSRF_COOKIE}=")))
        .filter(|v| !v.is_empty())
        .map(|s| s.to_string())
}

/// First-hop X-Forwarded-For + User-Agent for the session row's audit
/// fields. Same shape `classic_session_middleware::extract_audit_fields`
/// uses; deliberately not shared because that function is `pub(crate)` to
/// keep the middleware contract narrow and the duplication here is six
/// lines.
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

/// Build a full HTTP response carrying the login form + a freshly-set
/// pre-session CSRF cookie. Shared by the GET handler and the POST
/// handler's re-render-on-failure branch.
fn render_login_response(
    status: StatusCode,
    template: LoginTemplate,
    set_cookie: Option<String>,
) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!("classic login template render failed: {e}"))
    })?;
    let mut resp = (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response();
    if let Some(cookie) = set_cookie {
        if let Ok(hv) = HeaderValue::from_str(&cookie) {
            resp.headers_mut().append(header::SET_COOKIE, hv);
        }
    }
    Ok(resp)
}

/// Query string for GET /classic/login. The `?error=…` slot lets the 2FA
/// challenge bounce surface a flash on the login page without a session
/// to store it in. Only whitelisted values are honoured (see `LoginQuery::
/// flash_message`) — anything else is silently dropped to keep the form
/// from being weaponised as a reflected-XSS-by-error-string vector.
#[derive(serde::Deserialize, Debug, Default)]
pub struct LoginQuery {
    #[serde(default)]
    pub error: Option<String>,
}

impl LoginQuery {
    /// Translate the (untrusted) `?error=` value into a fixed, server-defined
    /// string. Anything we don't recognise yields `None` so the form
    /// re-renders without a flash.
    fn flash_message(&self) -> Option<String> {
        match self.error.as_deref() {
            Some("2fa_expired") => Some(EXPIRED_OR_INVALID_ERROR.to_string()),
            Some("2fa_too_many") => Some(TOO_MANY_CODES_ERROR.to_string()),
            _ => None,
        }
    }
}

/// GET /classic/login — render the form with a fresh pre-session CSRF
/// token.
///
/// If the user already has a valid session cookie, the natural flow is to
/// bounce them straight to the inbox. The cookie *signature* isn't
/// validated here (that's classic_session_middleware's job) — we just want
/// to avoid showing a login form to a logged-in user; if the cookie is
/// stale they'll bounce to the inbox, the middleware will reject, and
/// they'll land back here anyway.
///
/// Added (TMAIL-361): also accepts an optional `?error=2fa_expired|2fa_too_many`
/// query param so the 2FA challenge bounce can flash a message without a
/// session to store it in. The mapping is whitelisted to fixed server
/// strings — see `LoginQuery::flash_message`.
pub async fn get_login(
    headers: HeaderMap,
    Query(query): Query<LoginQuery>,
) -> Result<Response, AppError> {
    // Already-signed-in shortcut. Cheap presence check matches what
    // `handlers::classic::index_redirect` does for the /classic/ root.
    if headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(';')
                .map(str::trim)
                .any(|pair| pair.starts_with(&format!("{}=", super::CLASSIC_SESSION_COOKIE)))
        })
        .unwrap_or(false)
    {
        return Ok(Redirect::to(INBOX_PATH).into_response());
    }

    let token = generate_csrf_token();
    let template = LoginTemplate::new(String::new(), query.flash_message(), &token);
    render_login_response(
        StatusCode::OK,
        template,
        Some(build_login_csrf_cookie(&token)),
    )
}

/// POST /classic/login — validate CSRF, evaluate password, create session.
///
/// Returns a 303 redirect to `/classic/folders/INBOX` on success. On any
/// failure (bad CSRF, missing fields, bad credentials, lockout) re-renders
/// the form with a generic error message — same copy for every failure
/// branch, so an attacker can't enumerate which accounts exist.
///
/// CSRF is the FIRST check so an attacker who can't read the cookie
/// (cross-origin) can't even get a timing signal off the password
/// verification step.
pub async fn post_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<LoginForm>,
) -> Result<Response, AppError> {
    // 1) CSRF check: cookie value must byte-equal the form's _csrf field.
    //    Either side missing → reject with the CSRF-specific error so the
    //    user can self-diagnose a cookie-stripping extension.
    let Some(cookie_token) = extract_login_csrf_cookie(&headers) else {
        return render_failure(
            &form.email,
            CSRF_ERROR_MESSAGE,
            StatusCode::BAD_REQUEST,
            true,
        );
    };
    if form.csrf.is_empty() || !validate_csrf_token(&form.csrf, &cookie_token) {
        return render_failure(
            &form.email,
            CSRF_ERROR_MESSAGE,
            StatusCode::BAD_REQUEST,
            true,
        );
    }

    // 2) Required-field check — keep symmetric with the post-validation
    //    branch so empty inputs don't leak account existence by 400'ing
    //    differently. Re-render with the generic credential error.
    if form.email.trim().is_empty() || form.password.is_empty() {
        return render_failure(
            &form.email,
            GENERIC_LOGIN_ERROR,
            StatusCode::BAD_REQUEST,
            true,
        );
    }

    let (ip, ua) = extract_audit_fields(&headers);

    // 3) Password evaluation + lockout bookkeeping. The reusable helper
    //    handles all three of:
    //      - unknown email           → AppError::Unauthorized
    //      - wrong password          → AppError::Unauthorized (+ increment)
    //      - active lockout window   → AppError::AccountLocked
    //    Every branch surfaces the same generic error to avoid enumeration.
    let mailbox = match evaluate_password_login(
        &state.db,
        &state.config.lockout,
        form.email.trim(),
        &form.password,
        ip.as_deref(),
        ua.as_deref(),
    )
    .await
    {
        Ok(m) => m,
        Err(AppError::Unauthorized(_)) | Err(AppError::AccountLocked(_)) => {
            return render_failure(
                &form.email,
                GENERIC_LOGIN_ERROR,
                StatusCode::UNAUTHORIZED,
                true,
            );
        }
        // Database / internal errors bubble up to the global AppError handler
        // — those produce a 500 which is the right thing for an unrecoverable
        // server-side failure. The user retries.
        Err(other) => return Err(other),
    };

    // 4) 2FA short-circuit (TMAIL-361). If the resolved mailbox has TOTP
    //    enrolled we MUST NOT create the full `classic_sessions` row yet —
    //    that would defeat the 2FA gate. Instead:
    //      a) Clear any stale pending-2FA rows for this user (e.g. an
    //         abandoned previous attempt) so we don't accumulate them.
    //      b) Insert a fresh `pending_2fa_tokens` row (5 min fixed TTL).
    //      c) Set the `tasmail_classic_pending_2fa` cookie carrying the
    //         row's id + an HMAC signature.
    //      d) Also clear the pre-session login CSRF cookie (it's been
    //         consumed by this POST and the next stage doesn't need it).
    //      e) 303 → /classic/login/2fa.
    if mailbox.totp_enabled && mailbox.totp_secret.is_some() {
        // Best-effort clear of previous gates for this user. Failures here
        // are not fatal — the new row will still resolve via its cookie.
        let _ = PendingTwoFactorToken::delete_for_user(&state.db, mailbox.id).await;

        let challenge_csrf = generate_csrf_token();
        let pending = PendingTwoFactorToken::create(
            &state.db,
            mailbox.id,
            &challenge_csrf,
            ip.as_deref(),
            ua.as_deref(),
        )
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "pending_2fa_tokens insert failed: {e}"
            ))
        })?;

        let mut resp = Redirect::to(CHALLENGE_PATH).into_response();
        if let Ok(hv) =
            HeaderValue::from_str(&build_set_pending_cookie_header(&state.config.jwt.secret, pending.id))
        {
            resp.headers_mut().append(header::SET_COOKIE, hv);
        }
        if let Ok(hv) = HeaderValue::from_str(&build_clear_login_csrf_cookie()) {
            resp.headers_mut().append(header::SET_COOKIE, hv);
        }
        tracing::info!(
            user_id = ?mailbox.id,
            pending_id = ?pending.id,
            "classic login passed password — gating on TOTP challenge"
        );
        return Ok(resp);
    }

    // 5) Establish a real classic_sessions row + signed cookie.
    let established = create_session_and_cookie(&state, mailbox.id, ip.as_deref(), ua.as_deref()).await?;

    // 6) 303 See Other so the browser switches to GET for the inbox load.
    //    Attach BOTH the session cookie AND a cookie-clearing header for
    //    the one-shot pre-session CSRF cookie.
    let mut resp = Redirect::to(INBOX_PATH).into_response();
    resp.headers_mut().append(header::SET_COOKIE, established.set_cookie);
    if let Ok(hv) = HeaderValue::from_str(&build_clear_login_csrf_cookie()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    Ok(resp)
}

/// Re-render the form with an error message. Issues a FRESH pre-session
/// CSRF cookie so the next submission has a new token to match — the old
/// one is one-shot whether it succeeded or not, which keeps the threat
/// model simple (no need to track "consumed" tokens server-side).
fn render_failure(
    email: &str,
    error: &str,
    status: StatusCode,
    rotate_token: bool,
) -> Result<Response, AppError> {
    let token = if rotate_token {
        generate_csrf_token()
    } else {
        // Caller has signalled "don't rotate" — used by tests / future
        // branches that want to preserve the original token. Today every
        // caller passes true.
        generate_csrf_token()
    };
    let template = LoginTemplate::new(email, Some(error.to_string()), &token);
    render_login_response(status, template, Some(build_login_csrf_cookie(&token)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_template() -> LoginTemplate {
        LoginTemplate {
            email: String::new(),
            error: None,
            csrf_token: "fixed-csrf-token-for-tests".to_string(),
            csp_nonce: "fixed-nonce-for-tests".to_string(),
        }
    }

    #[test]
    fn login_paths_point_under_classic() {
        // Sanity check on the constants we import from auth.rs — locks down
        // the contract so a rename can't silently send users into the SPA.
        assert_eq!(LOGIN_PATH, "/classic/login");
        assert_eq!(INBOX_PATH, "/classic/folders/INBOX");
    }

    #[test]
    fn login_csrf_cookie_has_strict_attributes() {
        let h = build_login_csrf_cookie("token-value");
        assert!(h.contains(LOGIN_CSRF_COOKIE), "cookie name missing: {h}");
        assert!(h.contains("=token-value"), "token value missing: {h}");
        assert!(h.contains("HttpOnly"), "HttpOnly missing: {h}");
        assert!(h.contains("Secure"), "Secure missing: {h}");
        assert!(h.contains("SameSite=Strict"), "SameSite=Strict missing: {h}");
        assert!(
            h.contains("Path=/classic/login"),
            "narrow Path scope missing: {h}"
        );
        assert!(h.contains("Max-Age=900"), "Max-Age missing: {h}");
    }

    #[test]
    fn clear_login_csrf_cookie_uses_max_age_zero() {
        let h = build_clear_login_csrf_cookie();
        assert!(h.contains("Max-Age=0"), "Max-Age=0 missing: {h}");
        assert!(h.contains("HttpOnly") && h.contains("SameSite=Strict"));
        assert!(h.contains("Path=/classic/login"));
    }

    #[test]
    fn extract_login_csrf_cookie_finds_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("foo=bar; tasmail_classic_login_csrf=tok42; baz=qux"),
        );
        assert_eq!(
            extract_login_csrf_cookie(&headers).as_deref(),
            Some("tok42")
        );
    }

    #[test]
    fn extract_login_csrf_cookie_returns_none_when_empty() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("tasmail_classic_login_csrf="),
        );
        assert!(extract_login_csrf_cookie(&headers).is_none());
    }

    #[test]
    fn extract_login_csrf_cookie_returns_none_when_absent() {
        let headers = HeaderMap::new();
        assert!(extract_login_csrf_cookie(&headers).is_none());

        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("other=cookie"));
        assert!(extract_login_csrf_cookie(&headers).is_none());
    }

    #[test]
    fn login_template_renders_form_action_and_fields() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("action=\"/classic/login\""),
            "form action missing: {body}"
        );
        assert!(body.contains("method=\"post\""), "method=post missing");
        assert!(
            body.contains("name=\"email\"") && body.contains("type=\"email\""),
            "email input missing"
        );
        assert!(
            body.contains("name=\"password\"") && body.contains("type=\"password\""),
            "password input missing"
        );
        // Hidden CSRF field carries the same token name the post-login
        // forms use, so a future migration to unified middleware doesn't
        // need to rename inputs.
        assert!(
            body.contains("name=\"_csrf\"") && body.contains("value=\"fixed-csrf-token-for-tests\""),
            "hidden _csrf field missing or wrong value: {body}"
        );
        assert!(
            body.contains("autocomplete=\"username\""),
            "autocomplete hint missing on email input"
        );
        assert!(
            body.contains("autocomplete=\"current-password\""),
            "autocomplete hint missing on password input"
        );
    }

    #[test]
    fn login_template_omits_error_on_fresh_render() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("role=\"alert\""),
            "alert block must be absent on fresh GET, found: {body}"
        );
    }

    #[test]
    fn login_template_shows_generic_error_on_failure() {
        let mut t = fresh_template();
        t.error = Some("Incorrect email or password.".to_string());
        let body = t.render().expect("template renders");
        assert!(body.contains("role=\"alert\""), "alert role missing");
        assert!(body.contains("Incorrect email or password."));
    }

    #[test]
    fn login_template_preserves_email_on_failure() {
        let mut t = fresh_template();
        t.email = "user@example.com".to_string();
        t.error = Some("Incorrect email or password.".to_string());
        let body = t.render().expect("template renders");
        assert!(
            body.contains("value=\"user@example.com\""),
            "submitted email must round-trip into the form: {body}"
        );
    }

    #[test]
    fn login_template_html_escapes_hostile_email() {
        // Defence in depth: Askama auto-escapes for .html, but lock it
        // down so a config change can't silently turn it off.
        let mut t = fresh_template();
        t.email = "\"><script>alert(1)</script>".to_string();
        let body = t.render().expect("template renders");
        assert!(
            !body.contains("\"><script>alert(1)</script>"),
            "raw <script> leaked into form input value attribute: {body}"
        );
    }

    #[test]
    fn login_template_extends_base_layout() {
        // The login page is the FIRST page an anonymous user sees — it
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
    fn login_template_has_zero_script_tags() {
        // Hard rule per the gap analysis: the Classic UI is no-JS.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("<script"),
            "login template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn generic_error_message_does_not_leak_account_existence() {
        // Lock down the copy so a future "helpful" tweak ("Account locked,
        // try again in N minutes") doesn't accidentally turn the form into
        // an account-enumeration oracle. P1 #31 ships the friendlier
        // lockout-countdown copy as a separate task.
        assert!(
            !GENERIC_LOGIN_ERROR.to_lowercase().contains("locked"),
            "generic login error must not mention lockout: {GENERIC_LOGIN_ERROR}"
        );
        assert!(
            !GENERIC_LOGIN_ERROR.to_lowercase().contains("does not exist"),
            "generic login error must not mention account existence"
        );
        assert!(
            !GENERIC_LOGIN_ERROR.to_lowercase().contains("no such"),
            "generic login error must not mention account existence"
        );
    }

    #[test]
    fn csrf_error_message_is_distinct_from_credential_error() {
        // Different copy for the two failure branches lets a user whose
        // cookie was stripped by an extension self-diagnose, without
        // creating an enumeration oracle (CSRF state is per-browser, not
        // per-account).
        assert_ne!(CSRF_ERROR_MESSAGE, GENERIC_LOGIN_ERROR);
    }
}
