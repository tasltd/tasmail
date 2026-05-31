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
use crate::services::auth_service::{
    classify_login_failure, evaluate_password_login, lookup_lockout_state,
    LoginFailureDisposition,
};
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
/// account is currently in lockout.
///
/// Changed (TMAIL-385): the friendlier locked-state / warning copy now lives
/// behind the disposition dispatcher below. This constant is still the
/// fallback used for the Generic disposition (unknown email + cold-start
/// wrong-password) AND for the empty-fields / CSRF branches, so every
/// "harmless bad attempt" surfaces the same string.
const GENERIC_LOGIN_ERROR: &str = "Incorrect email or password.";

/// Message rendered when the CSRF token cookie / form pairing fails. Kept
/// distinct from the credential error so a user whose cookie was stripped
/// by an over-zealous extension can self-diagnose. Doesn't leak any
/// account-existence signal.
const CSRF_ERROR_MESSAGE: &str =
    "Your session expired before you submitted the form. Please try again.";

/// Added (TMAIL-385): Locked-state banner copy. Same text whether the
/// lockout was already in effect on entry or the current attempt just
/// tripped the threshold — the countdown numbers come from the
/// per-render view-model, the prose is fixed so a future i18n pass
/// (P1 #33) has a single source string to translate. Designed to be
/// terse + scannable in lynx/w3m: short header + countdown + link.
pub(crate) const LOCKOUT_HEADING: &str = "Account temporarily locked";
pub(crate) const LOCKOUT_PASSWORD_RESET_HINT: &str =
    "If this was you, you can reset your password to sign back in immediately.";

/// Added (TMAIL-385): "N attempts remaining" wording stem. The template
/// fills in the numeric `remaining` on render — keeping the stem here so
/// the future i18n loader can swap the English copy for a Twi/Ewe/Ga/
/// Hausa translation without touching the dispatcher.
pub(crate) const ATTEMPTS_WARNING_PREFIX: &str = "Warning:";
pub(crate) const ATTEMPTS_WARNING_SUFFIX: &str =
    "attempts remaining before your account is locked for security.";

/// Added (TMAIL-385): Per-render view-model surfacing the locked-state
/// countdown copy. Built by the POST handler when `classify_login_failure`
/// resolves to `Locked`. Pre-computed so the template stays arithmetic-free
/// (Askama can't do dates / math without custom filters).
///
/// Fields:
///   * `minutes_remaining` — rounded-up whole minutes until the lockout
///     expires. We round UP so a "59 seconds left" countdown never
///     renders as "0 minutes" (which would tell a confused user "wait 0
///     minutes" and they'd retry instantly).
///   * `seconds_remaining` — extra precision for the secondary line
///     ("about 4 min 32 sec"). Always 0..=59 — the minute value carries
///     the whole-minute part.
///   * `minute_is_singular` — precomputed boolean so the template can
///     pick between "minute" / "minutes" without doing an integer
///     comparison (Askama's `==` against integer literals is fragile
///     when the field is a borrowed reference — easier to compute the
///     plural decision here once).
///   * `iso_until` — RFC 3339 / ISO 8601 timestamp of `locked_until`, so
///     a screen reader user gets a machine-readable hint and a future
///     client-side script (if anyone ships one) can do a live countdown.
#[derive(Debug, Clone)]
pub struct LockoutDisplay {
    pub minutes_remaining: i64,
    pub seconds_remaining: i64,
    pub minute_is_singular: bool,
    pub iso_until: String,
}

impl LockoutDisplay {
    /// Build a `LockoutDisplay` from the absolute `locked_until` timestamp
    /// and the current wall clock. Saturates at 0 — `locked_until <= now`
    /// would normally render Generic via the classifier above, but we
    /// guard here too so a clock-skew edge case never renders negative.
    pub fn build(
        locked_until: chrono::DateTime<chrono::Utc>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let remaining = (locked_until - now).num_seconds().max(0);
        // Round-up whole minutes so "30 seconds left" renders as "1 minute"
        // — never "0 minutes".
        let minutes_remaining = (remaining + 59) / 60;
        let seconds_remaining = remaining % 60;
        Self {
            minutes_remaining,
            seconds_remaining,
            minute_is_singular: minutes_remaining == 1,
            iso_until: locked_until.to_rfc3339(),
        }
    }
}

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
    /// Added (TMAIL-385): when set, render the locked-state banner with a
    /// countdown to `lockout.iso_until` + a password-reset CTA. The form
    /// itself is still rendered (the disposition pre-check is "best
    /// effort": a user whose lockout expired between page load and submit
    /// must still be able to retry without a hard reload). `None` on
    /// every non-locked render.
    pub lockout: Option<LockoutDisplay>,
    /// Added (TMAIL-385): when set, render the "Warning: N attempts
    /// remaining" copy ABOVE the generic credential error. Always paired
    /// with `error = Some(GENERIC_LOGIN_ERROR)` so the warning is
    /// supplementary, not standalone.
    pub attempts_remaining: Option<AttemptsWarning>,
}

/// Added (TMAIL-385): View-model for the attempts-remaining warning copy.
///
/// `remaining` carries the raw count for the visible "N attempts" string.
/// `is_singular` is the precomputed boolean the template uses to pick
/// between "attempt" / "attempts" — avoids fighting Askama's borrowed-
/// integer-vs-literal comparison story.
#[derive(Debug, Clone, Copy)]
pub struct AttemptsWarning {
    pub remaining: i32,
    pub is_singular: bool,
}

impl AttemptsWarning {
    /// Build an `AttemptsWarning` from the bare remaining count. Clamped
    /// to >= 0 so a misconfigured (threshold-0) classifier can't render
    /// "-2 attempts remaining" — defensive layer on top of the classifier.
    pub fn new(remaining: i32) -> Self {
        let clamped = remaining.max(0);
        Self {
            remaining: clamped,
            is_singular: clamped == 1,
        }
    }
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
            // Added (TMAIL-385): default to no lockout view-model + no
            // warning copy. The POST handler sets these when the
            // disposition resolves to `Locked` / `AttemptsWarning`.
            lockout: None,
            attempts_remaining: None,
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
    let trimmed_email = form.email.trim();

    // 3) Pre-evaluation lockout short-circuit (TMAIL-385).
    //    Look up just the three lockout columns for this username — when
    //    the row is currently locked, render the locked-state banner with
    //    a countdown WITHOUT calling `evaluate_password_login` at all.
    //    The acceptance criteria explicitly require "lockout-active → no
    //    auth attempt is made server-side" so we MUST NOT touch the
    //    password hash on a locked row. The lookup is the same SELECT
    //    `evaluate_password_login` would do, minus the password column.
    //
    //    On a DB lookup error we log + fall through to the normal
    //    path — surfacing a 500 here would be worse than letting the
    //    next step's own DB lookup re-resolve (and a transient error
    //    means the user retries one extra time, no security regression).
    let now = chrono::Utc::now();
    match lookup_lockout_state(&state.db, trimmed_email).await {
        Ok(Some(state_row)) => {
            let pre_disposition =
                classify_login_failure(&state_row, &state.config.lockout, now);
            if let LoginFailureDisposition::Locked { .. } = pre_disposition {
                return render_disposition_failure(
                    &form.email,
                    pre_disposition,
                    now,
                    csp_nonce.as_str(),
                );
            }
        }
        Ok(None) => { /* Unknown email — fall through. evaluate_password_login
                       will resolve the generic Unauthorized branch with
                       constant-time-shaped behaviour. */ }
        Err(e) => {
            tracing::warn!(
                error = ?e,
                "classic login: lockout pre-check lookup failed — falling through"
            );
        }
    }

    // 4) Password evaluation + lockout bookkeeping. The reusable helper
    //    handles all three of:
    //      - unknown email           → AppError::Unauthorized
    //      - wrong password          → AppError::Unauthorized (+ increment)
    //      - active lockout window   → AppError::AccountLocked
    //
    //    Changed (TMAIL-385): on a failure, look up the mailbox state
    //    AGAIN and use the disposition classifier to decide whether the
    //    user-visible copy is:
    //      * Generic            — unknown email / cold-start wrong pw
    //      * AttemptsWarning    — inside the warning window (3 or 4 of 5)
    //      * Locked             — just tripped the threshold
    //    Errors from the lookup degrade gracefully to Generic so a
    //    transient DB blip never escalates to a 500 on the login page.
    let mailbox = match evaluate_password_login(
        &state.db,
        &state.config.lockout,
        trimmed_email,
        &form.password,
        ip.as_deref(),
        ua.as_deref(),
    )
    .await
    {
        Ok(m) => m,
        Err(AppError::Unauthorized(_)) | Err(AppError::AccountLocked(_)) => {
            let disposition = match lookup_lockout_state(&state.db, trimmed_email).await {
                Ok(Some(state_row)) => {
                    classify_login_failure(&state_row, &state.config.lockout, chrono::Utc::now())
                }
                Ok(None) => LoginFailureDisposition::Generic,
                Err(e) => {
                    tracing::warn!(
                        error = ?e,
                        "classic login: post-failure lockout lookup failed — falling back to Generic"
                    );
                    LoginFailureDisposition::Generic
                }
            };
            return render_disposition_failure(
                &form.email,
                disposition,
                chrono::Utc::now(),
                csp_nonce.as_str(),
            );
        }
        // Database / internal errors bubble up to the global AppError handler
        // — those produce a 500 which is the right thing for an unrecoverable
        // server-side failure. The user retries.
        Err(other) => return Err(other),
    };

    // 5) 2FA short-circuit (TMAIL-361 / TMAIL-381). If the resolved mailbox
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

    // 6) Establish a real classic_sessions row + signed cookie.
    let established = create_session_and_cookie(&state, mailbox.id, ip.as_deref(), ua.as_deref()).await?;

    // 7) 303 See Other so the browser switches to GET for the inbox load.
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

/// Added (TMAIL-385): Render a credential-failure response shaped by the
/// `LoginFailureDisposition` from `auth_service`.
///
/// Three branches, all of which surface the generic `GENERIC_LOGIN_ERROR`
/// as the primary alert but differ in the supplementary copy:
///   * `Generic`         → just the generic error.
///   * `AttemptsWarning` → generic error + "Warning: N attempts
///                         remaining…" prefix banner.
///   * `Locked`          → 423 + locked-state banner (countdown +
///                         password-reset hint). The form itself stays
///                         rendered so a user whose lockout window
///                         expired between page load and submit can
///                         retry without a hard refresh.
fn render_disposition_failure(
    email: &str,
    disposition: LoginFailureDisposition,
    now: chrono::DateTime<chrono::Utc>,
    csp_nonce: &str,
) -> Result<Response, AppError> {
    let token = generate_csrf_token();
    let (status, template) = match disposition {
        LoginFailureDisposition::Generic => {
            let tpl = LoginTemplate::new(
                email,
                Some(GENERIC_LOGIN_ERROR.to_string()),
                &token,
                csp_nonce,
            );
            (StatusCode::UNAUTHORIZED, tpl)
        }
        LoginFailureDisposition::AttemptsWarning { remaining } => {
            let mut tpl = LoginTemplate::new(
                email,
                Some(GENERIC_LOGIN_ERROR.to_string()),
                &token,
                csp_nonce,
            );
            tpl.attempts_remaining = Some(AttemptsWarning::new(remaining));
            (StatusCode::UNAUTHORIZED, tpl)
        }
        LoginFailureDisposition::Locked { locked_until } => {
            // 423 Locked matches the JWT path's `AccountLocked` mapping
            // — keeps the two surfaces' status codes in lockstep.
            let mut tpl = LoginTemplate::new(email, None, &token, csp_nonce);
            tpl.lockout = Some(LockoutDisplay::build(locked_until, now));
            (StatusCode::LOCKED, tpl)
        }
    };
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
            // Added (TMAIL-385): lockout view-model + warning copy
            // default to None — existing tests don't touch them; new
            // tests build their own structs with these fields set.
            lockout: None,
            attempts_remaining: None,
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
        // try again in N minutes") doesn't accidentally turn the GENERIC
        // branch into an account-enumeration oracle. The friendlier
        // locked-state copy lands in its OWN template branch (TMAIL-385),
        // not via this generic string — those branches are tested
        // separately below.
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

    // -----------------------------------------------------------------------
    // TMAIL-385 — Lockout countdown + attempts-remaining warning rendering.
    //
    // The pure classifier + lookup helpers live in `auth_service`. These
    // tests pin the Classic UI side: the LockoutDisplay arithmetic, the
    // template's locked-state + warning blocks, and the dispatcher's
    // status-code + view-model assignments.
    // -----------------------------------------------------------------------

    #[test]
    fn lockout_display_rounds_seconds_up_to_next_minute() {
        // 30 seconds remaining must surface as "1 minute" so a confused
        // user doesn't see "0 minutes" and retry instantly.
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(30);
        let display = LockoutDisplay::build(until, now);
        assert_eq!(display.minutes_remaining, 1);
        assert_eq!(display.seconds_remaining, 30);
        assert!(
            display.iso_until.contains('T'),
            "iso_until must be an RFC3339 timestamp, got: {}",
            display.iso_until
        );
    }

    #[test]
    fn lockout_display_handles_exact_minutes() {
        // Exactly 120 seconds → 2 minutes, 0 seconds. Round-up logic
        // must not push 120s into 3 minutes.
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(120);
        let display = LockoutDisplay::build(until, now);
        assert_eq!(display.minutes_remaining, 2);
        assert_eq!(display.seconds_remaining, 0);
    }

    #[test]
    fn lockout_display_saturates_to_zero_in_past() {
        // Clock skew defensive: if the caller hands in a `locked_until`
        // already in the past, surface 0/0 instead of negative numbers.
        let now = chrono::Utc::now();
        let until = now - chrono::Duration::seconds(60);
        let display = LockoutDisplay::build(until, now);
        assert_eq!(display.minutes_remaining, 0);
        assert_eq!(display.seconds_remaining, 0);
    }

    #[test]
    fn login_template_renders_locked_banner_with_countdown() {
        // Locked state must surface the heading, the minute count, AND
        // the password-reset CTA — every legitimate user's escape hatch.
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(7 * 60 + 30); // 7m30s
        let mut t = fresh_template();
        t.lockout = Some(LockoutDisplay::build(until, now));
        let body = t.render().expect("template renders");

        assert!(
            body.contains("Account temporarily locked"),
            "locked banner heading missing: {body}"
        );
        // Round-up "8 minutes" for 7m30s.
        assert!(
            body.contains("<strong>8</strong>"),
            "locked banner must show rounded-up minute count: {body}"
        );
        assert!(
            body.contains("/classic/password-reset/request"),
            "locked banner must link to password reset: {body}"
        );
        // role=alert + assertive live region so a screen reader announces
        // the lockout immediately on render.
        assert!(
            body.contains("role=\"alert\""),
            "locked banner must use role=alert: {body}"
        );
        assert!(
            body.contains("aria-live=\"assertive\""),
            "locked banner must use aria-live=assertive: {body}"
        );
        // Machine-readable expiry timestamp for assistive tech.
        assert!(
            body.contains("<time datetime="),
            "locked banner must surface a <time datetime=...> stamp: {body}"
        );
    }

    #[test]
    fn login_template_locked_banner_uses_singular_minute_when_one() {
        // Plural-handling: "1 minute" not "1 minutes". Lock the
        // `{% if == 1 %}minute{% else %}minutes{% endif %}` branch
        // down so a refactor can't silently swap them.
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(1);
        let mut t = fresh_template();
        t.lockout = Some(LockoutDisplay::build(until, now));
        let body = t.render().expect("template renders");
        // 1 second → minutes_remaining=1 via round-up.
        assert!(
            body.contains("<strong>1</strong>\n        minute\n"),
            "1-minute lockout must render singular 'minute': {body}"
        );
        assert!(
            !body.contains("<strong>1</strong>\n        minutes\n"),
            "must not render plural 'minutes' for 1-minute lockout: {body}"
        );
    }

    #[test]
    fn login_template_renders_attempts_remaining_warning() {
        // Warning banner must surface the prefix "Warning:", the
        // exact remaining count, AND announce as role=status (not
        // role=alert — see template comment). The two sub-strings
        // "N attempts" and "remaining before your account is locked"
        // are validated separately because the template line-wraps
        // between them in the rendered HTML.
        let mut t = fresh_template();
        t.attempts_remaining = Some(AttemptsWarning::new(2));
        t.error = Some(GENERIC_LOGIN_ERROR.to_string());
        let body = t.render().expect("template renders");
        assert!(
            body.contains("Warning:"),
            "warning banner missing 'Warning:' prefix: {body}"
        );
        assert!(
            body.contains("2 attempts"),
            "warning banner must show '2 attempts' substring: {body}"
        );
        assert!(
            body.contains("remaining before your account is locked"),
            "warning banner must use the full 'remaining before…is locked' wording: {body}"
        );
        // Supplementary copy — role=status, NOT role=alert.
        let warning_at = body
            .find("Warning:")
            .expect("warning banner present");
        let banner_start = body[..warning_at]
            .rfind("<div class=\"alert")
            .expect("warning banner div present");
        let banner_end = banner_at_close_offset(&body, banner_start);
        let banner_html = &body[banner_start..banner_end];
        assert!(
            banner_html.contains("role=\"status\""),
            "warning banner must use role=status (not role=alert): {banner_html}"
        );
        assert!(
            banner_html.contains("alert-warning"),
            "warning banner must use alert-warning class: {banner_html}"
        );
    }

    /// Local helper: find the `</div>` that closes a banner div opened at
    /// `start`. We can't just use `body[start..].find("</div>")` because
    /// the next sibling div might come first in a malformed render — but
    /// for our well-formed templates the first `</div>` after the
    /// opening tag is the right one.
    fn banner_at_close_offset(body: &str, start: usize) -> usize {
        let close = body[start..]
            .find("</div>")
            .expect("banner must have a closing tag");
        start + close + "</div>".len()
    }

    #[test]
    fn login_template_warning_singular_when_one_attempt_left() {
        // Plural-handling on the warning copy too: "1 attempt" not
        // "1 attempts".
        let mut t = fresh_template();
        t.attempts_remaining = Some(AttemptsWarning::new(1));
        t.error = Some(GENERIC_LOGIN_ERROR.to_string());
        let body = t.render().expect("template renders");
        assert!(
            body.contains(" 1 attempt\n"),
            "1-remaining warning must render singular 'attempt': {body}"
        );
        assert!(
            !body.contains(" 1 attempts\n"),
            "must not render plural 'attempts' for 1-remaining: {body}"
        );
    }

    #[test]
    fn login_template_locked_banner_replaces_generic_error_position() {
        // When both `lockout` and `error` are set (defensive: shouldn't
        // happen in production but we want a sane render), the lockout
        // banner must render BEFORE the generic error so a screen-reader
        // user hears the lockout first.
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(300);
        let mut t = fresh_template();
        t.lockout = Some(LockoutDisplay::build(until, now));
        t.error = Some("Some other failure".to_string());
        let body = t.render().expect("template renders");
        let locked_at = body
            .find("Account temporarily locked")
            .expect("locked banner present");
        let error_at = body.find("Some other failure").expect("error present");
        assert!(
            locked_at < error_at,
            "locked banner must render before error: locked@{locked_at} error@{error_at}"
        );
    }

    #[test]
    fn login_template_attempts_warning_renders_above_error() {
        // Warning + error: warning is supplementary; error is the
        // primary failure announcement — but the reading order goes
        // top-down. Warning above error so the user reads "Warning: N
        // remaining" THEN "Incorrect email or password."
        let mut t = fresh_template();
        t.attempts_remaining = Some(AttemptsWarning::new(2));
        t.error = Some(GENERIC_LOGIN_ERROR.to_string());
        let body = t.render().expect("template renders");
        let warning_at = body.find("Warning:").expect("warning present");
        let error_at = body
            .find(GENERIC_LOGIN_ERROR)
            .expect("generic error present");
        assert!(
            warning_at < error_at,
            "warning must render before error: warning@{warning_at} error@{error_at}"
        );
    }

    #[test]
    fn login_template_omits_lockout_banner_on_fresh_render() {
        // Fresh GET / unrelated failure — NO lockout banner copy
        // should leak into the page.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("Account temporarily locked"),
            "fresh render must NOT contain locked banner copy: {body}"
        );
        assert!(
            !body.contains("attempts remaining"),
            "fresh render must NOT contain warning copy: {body}"
        );
    }

    #[test]
    fn login_template_form_stays_renderable_in_locked_state() {
        // The form MUST still render even when locked — a user whose
        // window expires between page load and retry submit needs to
        // type their password again without a hard reload.
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(120);
        let mut t = fresh_template();
        t.lockout = Some(LockoutDisplay::build(until, now));
        let body = t.render().expect("template renders");
        assert!(
            body.contains("name=\"email\"") && body.contains("name=\"password\""),
            "locked render must still expose the email + password inputs: {body}"
        );
        assert!(
            body.contains("type=\"submit\""),
            "locked render must still expose the submit button: {body}"
        );
    }

    #[test]
    fn lockout_heading_constant_locked_down() {
        // The product wording is testable via the template; locking
        // the constant down prevents a future "Sign-in locked" /
        // "Account suspended" rewrite from silently changing the
        // user-visible copy without explicit review.
        assert_eq!(LOCKOUT_HEADING, "Account temporarily locked");
    }

    #[test]
    fn attempts_warning_wording_constants_locked_down() {
        // Lock the prefix + suffix wording so future tuning doesn't
        // silently drop "for security" or "Warning:" — the gap analysis
        // P1 #31 acceptance criteria call out the exact wording.
        assert!(ATTEMPTS_WARNING_PREFIX.contains("Warning"));
        assert!(
            ATTEMPTS_WARNING_SUFFIX
                .to_lowercase()
                .contains("attempts remaining before your account is locked")
        );
        assert!(
            ATTEMPTS_WARNING_SUFFIX
                .to_lowercase()
                .contains("for security")
        );
    }

    #[test]
    fn render_disposition_failure_locked_returns_423() {
        // The dispatcher must map the Locked disposition to a 423 status
        // code so any future caller (e.g. a CLI client) sees the same
        // signal as the JWT path's AccountLocked branch.
        use axum::http::StatusCode;
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(180);
        let resp = render_disposition_failure(
            "user@example.com",
            LoginFailureDisposition::Locked { locked_until: until },
            now,
            "test-nonce",
        )
        .expect("response builds");
        assert_eq!(resp.status(), StatusCode::LOCKED);
    }

    #[test]
    fn render_disposition_failure_warning_returns_401() {
        // The warning copy is supplementary — the underlying outcome is
        // still "bad credentials", so 401 is the right status to keep
        // tooling / log aggregators producing consistent counters.
        use axum::http::StatusCode;
        let resp = render_disposition_failure(
            "user@example.com",
            LoginFailureDisposition::AttemptsWarning { remaining: 2 },
            chrono::Utc::now(),
            "test-nonce",
        )
        .expect("response builds");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn render_disposition_failure_generic_returns_401() {
        use axum::http::StatusCode;
        let resp = render_disposition_failure(
            "user@example.com",
            LoginFailureDisposition::Generic,
            chrono::Utc::now(),
            "test-nonce",
        )
        .expect("response builds");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn render_disposition_failure_locked_response_includes_set_cookie() {
        // Every failure render MUST rotate the pre-session CSRF cookie
        // so the next submission can include a fresh matching token.
        // A locked response that didn't rotate would leave the user
        // unable to retry after the window expired without a hard
        // refresh.
        use axum::http::header;
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(60);
        let resp = render_disposition_failure(
            "user@example.com",
            LoginFailureDisposition::Locked { locked_until: until },
            now,
            "test-nonce",
        )
        .expect("response builds");
        let cookies: Vec<_> = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        assert!(
            cookies
                .iter()
                .any(|c| c.contains(LOGIN_CSRF_COOKIE) && !c.contains("Max-Age=0")),
            "locked response must rotate the pre-session CSRF cookie, found: {cookies:?}"
        );
    }

    #[test]
    fn lockout_password_reset_hint_links_to_reset_flow() {
        // The locked banner's only escape hatch is the password reset
        // link — every user must be able to recover without waiting
        // out the window. Pin both the hint text intent + the link.
        let now = chrono::Utc::now();
        let until = now + chrono::Duration::seconds(900);
        let mut t = fresh_template();
        t.lockout = Some(LockoutDisplay::build(until, now));
        let body = t.render().expect("template renders");
        assert!(
            body.contains("reset your password"),
            "locked banner must offer the reset-your-password escape hatch: {body}"
        );
        // Pin the link target separately so a copy-only refactor
        // (e.g. "change your password") still gets caught here.
        assert!(
            body.contains("href=\"/classic/password-reset/request\""),
            "locked banner reset link must point at /classic/password-reset/request: {body}"
        );
        assert!(
            LOCKOUT_PASSWORD_RESET_HINT
                .to_lowercase()
                .contains("reset your password"),
            "password-reset hint constant should mention the recovery action"
        );
    }
}
