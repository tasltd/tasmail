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
    Extension,
};

use crate::error::AppError;
use crate::middleware::classic_csrf::validate_csrf_token;
use crate::models::pending_2fa_token::PendingTwoFactorToken;
use crate::services::auth_service::evaluate_password_login;
use crate::state::AppState;

use super::auth::{create_session_and_cookie, generate_csrf_token, INBOX_PATH, LOGIN_PATH};
use super::sms_otp_challenge::SMS_CHALLENGE_PATH;
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
    /// Added (TMAIL-375): `Some("…")` on a success-flash render (currently
    /// only post-password-reset via `?reset=ok`); `None` otherwise.
    /// Rendered inside an `alert-success role="status"` block ABOVE the
    /// error alert.
    pub info: Option<String>,
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
    ///
    /// Changed (TMAIL-368): takes the CSP nonce as an explicit parameter
    /// instead of generating its own. The middleware pre-inserts one nonce
    /// per /classic/* request into `req.extensions()` and the handler
    /// threads it through to here so the header nonce + the inline
    /// `<style nonce="…">` attribute agree byte-for-byte.
    fn new(
        email: impl Into<String>,
        error: Option<String>,
        csrf_token: impl Into<String>,
        csp_nonce: impl Into<String>,
    ) -> Self {
        Self {
            email: email.into(),
            error,
            // Default: no success flash. Only the GET handler sets this
            // when `?reset=ok` is on the URL.
            info: None,
            csrf_token: csrf_token.into(),
            csp_nonce: csp_nonce.into(),
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
///
/// Added (TMAIL-375): `?reset=ok` is a parallel slot for SUCCESS flashes
/// (currently only the post-password-reset landing). Same whitelist
/// discipline — anything outside the recognised set yields `None`.
#[derive(serde::Deserialize, Debug, Default)]
pub struct LoginQuery {
    #[serde(default)]
    pub error: Option<String>,
    /// TMAIL-375: `?reset=ok` is the success flash from the password-reset
    /// confirm POST. Whitelisted server-side to one fixed string.
    #[serde(default)]
    pub reset: Option<String>,
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

    /// Added (TMAIL-375): translate the `?reset=…` value into a fixed
    /// SUCCESS message rendered alongside the form. Anything we don't
    /// recognise yields `None`.
    fn success_message(&self) -> Option<String> {
        match self.reset.as_deref() {
            Some("ok") => Some(
                "Your password has been updated. Sign in with your new password.".to_string(),
            ),
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
    // Added (TMAIL-368): pull the per-request CSP nonce out of request
    // extensions where `security_headers_middleware` parked it on the way
    // in. Threading it explicitly through every handler signature keeps the
    // header-vs-template binding visible to readers (and to the type system).
    Extension(csp_nonce): Extension<CspNonce>,
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
    let mut template = LoginTemplate::new(
        String::new(),
        query.flash_message(),
        &token,
        csp_nonce.as_str(),
    );
    // Added (TMAIL-375): plumb the success-flash slot too. Whitelisted
    // values like `?reset=ok` produce a fixed server-defined success
    // string rendered ABOVE the form. Anything we don't recognise yields
    // None and the page renders exactly as before.
    template.info = query.success_message();
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
    // Added (TMAIL-368): same as `get_login` — the middleware's per-request
    // nonce flows in here so every re-render-on-failure path uses the same
    // value that's in the response CSP header.
    Extension(csp_nonce): Extension<CspNonce>,
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
            csp_nonce.as_str(),
        );
    };
    if form.csrf.is_empty() || !validate_csrf_token(&form.csrf, &cookie_token) {
        return render_failure(
            &form.email,
            CSRF_ERROR_MESSAGE,
            StatusCode::BAD_REQUEST,
            true,
            csp_nonce.as_str(),
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
            csp_nonce.as_str(),
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
                csp_nonce.as_str(),
            );
        }
        // Database / internal errors bubble up to the global AppError handler
        // — those produce a 500 which is the right thing for an unrecoverable
        // server-side failure. The user retries.
        Err(other) => return Err(other),
    };

    // 4) 2FA short-circuit (TMAIL-361 / TMAIL-381). If the resolved mailbox
    //    has either TOTP OR SMS-OTP enrolled we MUST NOT create the full
    //    `classic_sessions` row yet — that would defeat the 2FA gate. The
    //    common envelope (pending_2fa_tokens row + signed cookie) is shared
    //    by both factors; we 303 to the factor-specific challenge page.
    //
    //    Precedence: TOTP wins when both are enrolled — it's stronger
    //    (offline, no SMS-provider dependency, no carrier-cost). The SMS
    //    challenge page surfaces a "Use authenticator app instead" link
    //    when the resolved mailbox also has TOTP enrolled, in case a
    //    future refactor lets the user pick.
    let totp_active = mailbox.totp_enabled && mailbox.totp_secret.is_some();
    let sms_active = !totp_active
        && sms_otp_enrolled(&state.db, mailbox.id).await.unwrap_or(false);
    if totp_active || sms_active {
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

        // Factor-specific landing page. Note SMS_CHALLENGE_PATH lives next
        // to /classic/login on the public sub-router — both 303 targets are
        // reachable without a real session.
        let target_path = if totp_active {
            CHALLENGE_PATH
        } else {
            SMS_CHALLENGE_PATH
        };
        let mut resp = Redirect::to(target_path).into_response();
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
            factor = if totp_active { "totp" } else { "sms" },
            "classic login passed password — gating on 2FA challenge"
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
    // Added (TMAIL-368): explicit CSP nonce — must be the per-request value
    // the security_headers middleware also baked into the CSP response
    // header. Threaded from the public handler signatures.
    csp_nonce: &str,
) -> Result<Response, AppError> {
    let token = if rotate_token {
        generate_csrf_token()
    } else {
        // Caller has signalled "don't rotate" — used by tests / future
        // branches that want to preserve the original token. Today every
        // caller passes true.
        generate_csrf_token()
    };
    let template = LoginTemplate::new(email, Some(error.to_string()), &token, csp_nonce);
    render_login_response(status, template, Some(build_login_csrf_cookie(&token)))
}

/// Added (TMAIL-381): Lookup helper for the SMS-OTP enrollment gate. Returns
/// true only when the mailbox has both `sms_otp_enabled = true` AND a
/// phone number configured — both are required to actually deliver the code
/// at challenge time, so treating "enabled but no phone" as inactive avoids
/// 303-ing the user into a challenge page that can't function.
///
/// Lives here (not on the Mailbox model) because the model intentionally
/// stays narrow — the existing API doesn't depend on these columns and we
/// don't want to widen `Mailbox` just for this one branch in login. A
/// future task that fans the SMS columns out to more callers should
/// promote them onto the struct.
async fn sms_otp_enrolled(pool: &sqlx::PgPool, mailbox_id: uuid::Uuid) -> Result<bool, sqlx::Error> {
    let row: Option<(bool, Option<String>)> = sqlx::query_as(
        "SELECT sms_otp_enabled, phone_number FROM mailboxes WHERE id = $1",
    )
    .bind(mailbox_id)
    .fetch_optional(pool)
    .await?;
    Ok(matches!(row, Some((true, Some(ref p))) if !p.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_template() -> LoginTemplate {
        LoginTemplate {
            email: String::new(),
            error: None,
            // Added (TMAIL-375): success-flash slot defaults to None for
            // existing tests; new tests that exercise the reset-success
            // path build their own struct with `info: Some(...)`.
            info: None,
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

    // --- TMAIL-375: success-flash slot on the login form ---

    #[test]
    fn login_query_success_message_whitelists_reset_ok() {
        let q = LoginQuery {
            error: None,
            reset: Some("ok".to_string()),
        };
        let msg = q.success_message().expect("?reset=ok must yield a message");
        assert!(
            msg.to_lowercase().contains("password has been updated"),
            "reset=ok flash must mention the password update: {msg}"
        );
    }

    #[test]
    fn login_query_success_message_drops_unknown_reset_value() {
        // Anything outside the whitelist yields None — no reflected-XSS
        // vector via the query string.
        let q = LoginQuery {
            error: None,
            reset: Some("\"><script>alert(1)</script>".to_string()),
        };
        assert!(q.success_message().is_none());
    }

    #[test]
    fn login_query_success_message_none_when_param_absent() {
        let q = LoginQuery {
            error: None,
            reset: None,
        };
        assert!(q.success_message().is_none());
    }

    #[test]
    fn login_template_renders_info_alert_when_present() {
        let mut t = fresh_template();
        t.info = Some("Your password has been updated.".to_string());
        let body = t.render().expect("template renders");
        // Success alert uses role="status" (vs role="alert" for errors)
        // so a screen reader doesn't interrupt — it's informational, not
        // a failure.
        assert!(
            body.contains("alert-success"),
            "info alert must use alert-success class: {body}"
        );
        assert!(
            body.contains("role=\"status\""),
            "info alert must use role=status, not role=alert: {body}"
        );
        assert!(body.contains("Your password has been updated."));
    }

    #[test]
    fn login_template_info_renders_above_error_when_both_present() {
        // Rare but possible — a user lands on /login?reset=ok then types
        // a wrong password. The info ("reset succeeded") MUST render
        // before the error ("bad credentials") so the natural reading
        // order matches the chronological order of events.
        let mut t = fresh_template();
        t.info = Some("Reset done.".to_string());
        t.error = Some("Bad creds.".to_string());
        let body = t.render().expect("template renders");
        let info_at = body.find("Reset done.").expect("info present");
        let error_at = body.find("Bad creds.").expect("error present");
        assert!(
            info_at < error_at,
            "info alert must render before error alert: info@{info_at} error@{error_at}"
        );
    }

    #[test]
    fn login_template_has_forgot_password_link() {
        // TMAIL-375: every login page must surface a discoverable
        // "Forgot your password?" link — that's the only entry point a
        // signed-out user has to the reset flow.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("/classic/password-reset/request"),
            "login page must link to /classic/password-reset/request: {body}"
        );
        assert!(
            body.to_lowercase().contains("forgot"),
            "link text must include the word 'Forgot' for findability"
        );
    }
}
