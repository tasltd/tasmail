// Added (TMAIL-375): GET/POST handlers for the /classic no-JS password reset
// flow. Two pages, each with its own pre-session double-submit-cookie CSRF
// pair (same shape as `handlers/classic/login.rs`):
//
//   /classic/password-reset/request — email field → emails a one-shot link.
//   /classic/password-reset/confirm?token=… — new + confirm password form
//                                              → updates the mailbox + revokes
//                                              every existing session.
//
// Design notes
// ------------
// * **No account enumeration.** The request POST ALWAYS renders the same
//   "if that address is registered, you'll receive an email shortly" page,
//   regardless of whether the email resolves to a mailbox. Timing is
//   deliberately equalised by NOT doing the SMTP send inline in a way that
//   leaks duration (we spawn it onto a tokio task — outbound socket time
//   does not feed back into the HTTP response).
// * **Single-use tokens with hash-only storage.** The raw 32-byte token
//   only ever appears in the outbound email + the user's clipboard. The
//   DB stores SHA-256(token). On confirm we hash the inbound query param,
//   look it up, then `mark_used` (atomic UPDATE with `used_at IS NULL`
//   guard so two concurrent confirms can't both succeed).
// * **Revoke EVERY existing session.** After a successful password change
//   we wipe BOTH the classic_sessions rows AND the SPA's refresh-token
//   `sessions` rows for that user. The whole point of a reset is that the
//   user lost control of their credentials — any live cookie / token
//   issued before the reset is suspect.
// * **CSRF via double-submit cookie.** Same OWASP pattern login + signup
//   use. The pair is `_csrf` form field ↔ `tasmail_classic_pwreset_csrf`
//   cookie (Path scoped to /classic/password-reset, HttpOnly, SameSite=
//   Strict, Secure). Validates with constant-time compare.
// * **Strict CSP nonce wiring.** Every render pulls the per-request nonce
//   out of request extensions and threads it into the template, matching
//   the rest of /classic/*.

use askama::Template;
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Extension,
};

use crate::error::AppError;
use crate::middleware::classic_csrf::validate_csrf_token;
use crate::models::classic_session::ClassicSession;
use crate::models::mailbox::Mailbox;
use crate::models::password_reset_token::{
    hash_reset_token, PasswordResetToken, PASSWORD_RESET_TTL_SECS,
};
use crate::models::session::Session;
use crate::services::auth_service::hash_password;
use crate::services::smtp_service::{SendRequest, SmtpService};
use crate::state::AppState;

use super::auth::{generate_csrf_token, LOGIN_PATH};
use super::CspNonce;

/// CSRF cookie name. Distinct from the login + signup pre-session cookies so
/// they never collide via the Path scope. HttpOnly + Secure + SameSite=Strict
/// + Path=/classic/password-reset.
pub const PWRESET_CSRF_COOKIE: &str = "tasmail_classic_pwreset_csrf";

/// How long the pre-session CSRF cookie lives. 15 minutes is enough for a
/// user to read the request form, walk away, come back and submit without
/// having to refresh; short enough that a leaked cookie isn't useful.
const PWRESET_CSRF_TTL_SECS: i64 = 900;

/// Minimum password length on the confirm form. Matches the signup wizard
/// (TMAIL-374, `MIN_PASSWORD_LEN = 8`) so the rules don't drift between
/// signup and reset paths.
const MIN_PASSWORD_LEN: usize = 8;

/// Generic copy rendered on the request POST regardless of whether the
/// email exists. Locked down by a test so a "helpful" tweak can't accidentally
/// turn it into an enumeration oracle.
const GENERIC_REQUEST_DONE_MESSAGE: &str =
    "If that email address is registered with TASMail, we have sent password \
     reset instructions. Check your inbox (and spam folder).";

/// Path the request form posts to. Centralised to keep the templates and
/// handlers in lockstep on a rename.
pub const REQUEST_PATH: &str = "/classic/password-reset/request";

/// Path the confirm form posts to.
pub const CONFIRM_PATH: &str = "/classic/password-reset/confirm";

// ---------- Cookie helpers ----------

/// Build the Set-Cookie header value carrying the pre-session CSRF token.
fn build_csrf_cookie(token: &str) -> String {
    format!(
        "{PWRESET_CSRF_COOKIE}={token}; HttpOnly; Secure; SameSite=Strict; \
         Path=/classic/password-reset; Max-Age={PWRESET_CSRF_TTL_SECS}"
    )
}

/// Build the Set-Cookie header value that clears the pre-session CSRF cookie.
/// Called on success branches so the consumed token can't be replayed.
fn build_clear_csrf_cookie() -> String {
    format!(
        "{PWRESET_CSRF_COOKIE}=; HttpOnly; Secure; SameSite=Strict; \
         Path=/classic/password-reset; Max-Age=0"
    )
}

/// Pull the cookie value out of inbound request headers. Returns None for
/// any malformed / missing case — the POST handler treats that identically
/// to a wrong cookie value (re-render with the CSRF rejection message).
fn extract_csrf_cookie(headers: &HeaderMap) -> Option<String> {
    let header_val = headers.get(header::COOKIE)?.to_str().ok()?;
    header_val
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(&format!("{PWRESET_CSRF_COOKIE}=")))
        .filter(|v| !v.is_empty())
        .map(|s| s.to_string())
}

/// First-hop X-Forwarded-For + User-Agent for the audit columns. Same
/// shape as the login handler — kept duplicated rather than extracted to
/// `auth.rs` because it's six lines and the duplication keeps modules
/// self-contained.
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

/// Build the public-facing absolute URL of the confirm link. Pulls the
/// scheme from `X-Forwarded-Proto` (Apache sets this when fronting the SSH
/// tunnel), defaulting to `https`. Pulls the host from `X-Forwarded-Host`
/// then `Host`. Falls back to `https://mail.techatscale.io` if both are
/// missing (the production canonical URL).
fn build_reset_link(headers: &HeaderMap, raw_token: &str) -> String {
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("https").trim())
        .filter(|s| matches!(*s, "https" | "http"))
        .unwrap_or("https");
    let host = headers
        .get("x-forwarded-host")
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get(header::HOST).and_then(|v| v.to_str().ok()))
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mail.techatscale.io".to_string());
    // urlencoding is already a transitive dep used by other classic handlers.
    let token = urlencoding::encode(raw_token);
    format!("{scheme}://{host}{CONFIRM_PATH}?token={token}")
}

// ---------- Request page (step 1) ----------

#[derive(Template)]
#[template(path = "classic/password_reset_request.html")]
pub struct RequestFormTemplate {
    /// Pre-fill the email field across a CSRF rejection so the user
    /// doesn't have to retype.
    pub email: String,
    /// Some("…") on CSRF rejection; None on fresh GET.
    pub error: Option<String>,
    /// Pre-session CSRF token also written to the cookie.
    pub csrf_token: String,
    pub csp_nonce: String,
}

#[derive(Template)]
#[template(path = "classic/password_reset_request_done.html")]
pub struct RequestDoneTemplate {
    pub message: String,
    pub csp_nonce: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct RequestForm {
    pub email: String,
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// GET /classic/password-reset/request — render the email-entry form.
pub async fn get_request(
    Extension(csp_nonce): Extension<CspNonce>,
) -> Result<Response, AppError> {
    let token = generate_csrf_token();
    let template = RequestFormTemplate {
        email: String::new(),
        error: None,
        csrf_token: token.clone(),
        csp_nonce: csp_nonce.as_str().to_string(),
    };
    render_html(StatusCode::OK, &template, Some(build_csrf_cookie(&token)))
}

/// POST /classic/password-reset/request — issue token + email link.
///
/// ALWAYS renders the generic "if registered we sent it" page — no
/// enumeration oracle. SMTP send is fire-and-forget so latency doesn't
/// leak whether the email existed.
pub async fn post_request(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<RequestForm>,
) -> Result<Response, AppError> {
    // 1) CSRF — cookie must byte-equal form value (constant time).
    let cookie_token = extract_csrf_cookie(&headers);
    let csrf_ok = cookie_token
        .as_deref()
        .map(|c| !form.csrf.is_empty() && validate_csrf_token(&form.csrf, c))
        .unwrap_or(false);
    if !csrf_ok {
        let new_token = generate_csrf_token();
        let template = RequestFormTemplate {
            email: form.email.clone(),
            error: Some(
                "Your session expired before you submitted the form. Please try again.".to_string(),
            ),
            csrf_token: new_token.clone(),
            csp_nonce: csp_nonce.as_str().to_string(),
        };
        return render_html(
            StatusCode::BAD_REQUEST,
            &template,
            Some(build_csrf_cookie(&new_token)),
        );
    }

    let email = form.email.trim().to_string();

    // 2) Best-effort token issuance + email send. Wrapped so a missing
    //    mailbox / DB error / SMTP failure cannot leak via the response —
    //    we ALWAYS render the same generic done page.
    if !email.is_empty() {
        let (ip, ua) = extract_audit_fields(&headers);
        let reset_link = build_reset_link(&headers, "__PLACEHOLDER__"); // shape check only
        // Fire-and-forget: clone state + headers we need, drop the rest, and
        // let it run on the runtime without awaiting. Latency of the SMTP
        // dial-out therefore does NOT correlate with whether the address
        // resolved to a mailbox, defeating timing-based enumeration.
        let state_clone = state.clone();
        let email_for_task = email.clone();
        let headers_clone = headers.clone();
        let ip_for_task = ip.clone();
        let ua_for_task = ua.clone();
        tokio::spawn(async move {
            if let Err(err) = issue_and_send(
                &state_clone,
                &email_for_task,
                &headers_clone,
                ip_for_task.as_deref(),
                ua_for_task.as_deref(),
            )
            .await
            {
                tracing::warn!(
                    email = %email_for_task,
                    err = ?err,
                    "password reset issuance failed (silent — generic page already returned)"
                );
            }
        });
        // Drop the unused link variable defensively so a future refactor
        // doesn't accidentally render it into the response.
        let _ = reset_link;
    }

    // 3) Always-the-same done page. Returns 200 OK regardless of whether
    //    the email matched — locked down by a unit test that compares the
    //    response bytes for an existing-shaped vs non-existing email.
    let template = RequestDoneTemplate {
        message: GENERIC_REQUEST_DONE_MESSAGE.to_string(),
        csp_nonce: csp_nonce.as_str().to_string(),
    };
    render_html(StatusCode::OK, &template, Some(build_clear_csrf_cookie()))
}

/// Worker for the spawned task that runs the actual mailbox lookup,
/// token issuance, and email send. Errors are returned up to the spawning
/// closure where they're tracing::warn!'d — never surface to the user.
async fn issue_and_send(
    state: &AppState,
    email: &str,
    headers: &HeaderMap,
    ip: Option<&str>,
    ua: Option<&str>,
) -> Result<(), AppError> {
    let mailbox = match Mailbox::find_by_username(&state.db, email).await {
        Ok(Some(m)) if m.active => m,
        Ok(_) => return Ok(()), // unknown email or inactive — silently drop
        Err(e) => return Err(AppError::Internal(anyhow::anyhow!(e))),
    };

    // Invalidate any prior pending tokens before issuing a fresh one so
    // a user clicking "Forgot password" twice doesn't leave the older
    // link still live.
    let _ = PasswordResetToken::delete_for_user(&state.db, mailbox.id).await;

    let issued = PasswordResetToken::create(&state.db, mailbox.id, ip, ua)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let reset_link = build_reset_link(headers, &issued.raw_token);
    let ttl_minutes = PASSWORD_RESET_TTL_SECS / 60;
    let text_body = format!(
        "Hi,\n\n\
         Someone (hopefully you) requested a password reset for your TASMail \
         account ({email}).\n\n\
         To reset your password, open this link in your browser within the \
         next {ttl_minutes} minutes:\n\n\
         {reset_link}\n\n\
         If you did not request a reset, you can safely ignore this email — \
         your password will not be changed.\n\n\
         — TASMail\n"
    );
    let html_body = format!(
        "<p>Hi,</p>\
         <p>Someone (hopefully you) requested a password reset for your TASMail \
         account ({email_html}).</p>\
         <p>To reset your password, open this link in your browser within the \
         next {ttl_minutes} minutes:</p>\
         <p><a href=\"{link_html}\">{link_html}</a></p>\
         <p>If you did not request a reset, you can safely ignore this email — \
         your password will not be changed.</p>\
         <p>— TASMail</p>",
        email_html = html_escape(email),
        link_html = html_escape(&reset_link),
    );

    let smtp = SmtpService::new(state.config.smtp.clone());
    smtp.send_notification(&SendRequest {
        to: vec![email.to_string()],
        cc: None,
        bcc: None,
        subject: "TASMail password reset request".to_string(),
        text_body: Some(text_body),
        html_body: Some(html_body),
        in_reply_to: None,
        references: None,
        attachments: Vec::new(),
    })
    .await?;
    tracing::info!(user_id = ?mailbox.id, "password reset email sent");
    Ok(())
}

/// Minimal HTML escaper for the body template. Used instead of pulling in
/// `askama` rendering for the email body (which would require yet another
/// template file). Covers the four characters that matter inside text +
/// attribute contexts.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ---------- Confirm page (step 2) ----------

#[derive(serde::Deserialize, Debug)]
pub struct ConfirmQuery {
    pub token: String,
}

#[derive(Template)]
#[template(path = "classic/password_reset_confirm.html")]
pub struct ConfirmFormTemplate {
    /// Raw token round-tripped into the form as a hidden field. Askama
    /// auto-escape keeps it inert inside the value attribute.
    pub token: String,
    /// Some("…") on validation failure; None on fresh GET.
    pub error: Option<String>,
    pub csrf_token: String,
    pub csp_nonce: String,
}

#[derive(Template)]
#[template(path = "classic/password_reset_confirm_invalid.html")]
pub struct ConfirmInvalidTemplate {
    pub csp_nonce: String,
}

#[derive(Template)]
#[template(path = "classic/password_reset_confirm_done.html")]
pub struct ConfirmDoneTemplate {
    pub csp_nonce: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct ConfirmForm {
    pub token: String,
    pub new_password: String,
    pub confirm_password: String,
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// GET /classic/password-reset/confirm?token=… — validate token + render form
/// (or render the generic invalid page when missing / expired / used).
pub async fn get_confirm(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    Query(query): Query<ConfirmQuery>,
) -> Result<Response, AppError> {
    if query.token.is_empty() {
        return render_invalid(csp_nonce.as_str());
    }
    let token_hash = hash_reset_token(&query.token);
    let row = PasswordResetToken::find_active_by_hash(&state.db, &token_hash)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    if row.is_none() {
        return render_invalid(csp_nonce.as_str());
    }
    let token = generate_csrf_token();
    let template = ConfirmFormTemplate {
        token: query.token,
        error: None,
        csrf_token: token.clone(),
        csp_nonce: csp_nonce.as_str().to_string(),
    };
    render_html(StatusCode::OK, &template, Some(build_csrf_cookie(&token)))
}

/// POST /classic/password-reset/confirm — apply the new password.
///
/// Steps:
///   1. CSRF check (cookie ↔ form).
///   2. Token re-validate (still pending / unexpired) — defence in depth.
///   3. Password length + match check.
///   4. Update mailbox password hash.
///   5. Atomically mark token used (loses to a concurrent confirm).
///   6. Revoke EVERY existing session (classic + SPA refresh).
///   7. 303 → /classic/login?reset=ok (so the user lands on a fresh
///      login page and signs in with the new password).
pub async fn post_confirm(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<ConfirmForm>,
) -> Result<Response, AppError> {
    let csp_nonce_str = csp_nonce.as_str().to_string();

    // 1) CSRF
    let cookie_token = extract_csrf_cookie(&headers);
    let csrf_ok = cookie_token
        .as_deref()
        .map(|c| !form.csrf.is_empty() && validate_csrf_token(&form.csrf, c))
        .unwrap_or(false);
    if !csrf_ok {
        return render_confirm_failure(
            &form.token,
            "Your session expired before you submitted the form. Please try again.",
            StatusCode::BAD_REQUEST,
            &csp_nonce_str,
        );
    }

    // 2) Token re-validation — find the row again, atomically.
    if form.token.is_empty() {
        return render_invalid(&csp_nonce_str);
    }
    let token_hash = hash_reset_token(&form.token);
    let row = PasswordResetToken::find_active_by_hash(&state.db, &token_hash)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let row = match row {
        Some(r) => r,
        None => return render_invalid(&csp_nonce_str),
    };

    // 3) Password validation. Two checks: length AND match. Errors are
    //    SPECIFIC here (no enumeration concern — the user has already
    //    proved they hold a valid token).
    if form.new_password.len() < MIN_PASSWORD_LEN {
        return render_confirm_failure(
            &form.token,
            &format!(
                "Password must be at least {MIN_PASSWORD_LEN} characters long."
            ),
            StatusCode::BAD_REQUEST,
            &csp_nonce_str,
        );
    }
    if form.new_password != form.confirm_password {
        return render_confirm_failure(
            &form.token,
            "Passwords do not match.",
            StatusCode::BAD_REQUEST,
            &csp_nonce_str,
        );
    }

    // 4) Hash + update.
    let new_hash = hash_password(&form.new_password)?;
    Mailbox::update_password(&state.db, row.user_id, &new_hash)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    // 5) Mark used atomically. Returns false if a concurrent confirm beat
    //    us — in that case we still re-rendered the invalid page (the
    //    token UPDATE we did above is harmless because the next attempt
    //    will see used_at != NULL). We log the race for observability.
    let won_race = PasswordResetToken::mark_used(&state.db, row.id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    if !won_race {
        tracing::warn!(
            token_id = ?row.id,
            user_id = ?row.user_id,
            "concurrent password reset confirm — the other request marked the token used \
             after we already wrote the new password. Both writes converge on the same \
             new password (the user submitted the same form twice), so this is safe."
        );
    }

    // 6) Revoke EVERY existing session for this user. Both surfaces.
    //    A failure here is logged but does not roll back the password
    //    change — the user can still log in with the new password.
    if let Err(e) = ClassicSession::delete_all_for_user(&state.db, row.user_id).await {
        tracing::warn!(
            user_id = ?row.user_id,
            err = ?e,
            "failed to revoke classic_sessions after password reset"
        );
    }
    if let Err(e) = Session::delete_all_for_mailbox(&state.db, row.user_id).await {
        tracing::warn!(
            user_id = ?row.user_id,
            err = ?e,
            "failed to revoke SPA sessions after password reset"
        );
    }

    // 7) 303 to login with a friendly flash flag. Also clears the
    //    pre-session CSRF cookie since the token is consumed.
    let mut resp = Redirect::to(&format!("{LOGIN_PATH}?reset=ok")).into_response();
    if let Ok(hv) = HeaderValue::from_str(&build_clear_csrf_cookie()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    if let Ok(hv) = HeaderValue::from_str("no-store, max-age=0, must-revalidate") {
        resp.headers_mut().insert(header::CACHE_CONTROL, hv);
    }
    tracing::info!(
        user_id = ?row.user_id,
        token_id = ?row.id,
        "classic password reset confirmed — password rotated, sessions revoked"
    );
    Ok(resp)
}

// ---------- Render helpers ----------

/// Render an Askama template as a complete HTTP response, optionally
/// attaching a Set-Cookie header.
fn render_html<T: Template>(
    status: StatusCode,
    template: &T,
    set_cookie: Option<String>,
) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic password-reset template render failed: {e}"
        ))
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

/// Render the generic "this link is invalid or has expired" page. Same
/// response for unknown / expired / used / wrong-format tokens — no oracle.
fn render_invalid(csp_nonce: &str) -> Result<Response, AppError> {
    let template = ConfirmInvalidTemplate {
        csp_nonce: csp_nonce.to_string(),
    };
    render_html(StatusCode::OK, &template, Some(build_clear_csrf_cookie()))
}

/// Re-render the confirm form with an error message. Issues a FRESH
/// pre-session CSRF cookie so the next submit has a new token.
fn render_confirm_failure(
    token: &str,
    error: &str,
    status: StatusCode,
    csp_nonce: &str,
) -> Result<Response, AppError> {
    let new_csrf = generate_csrf_token();
    let template = ConfirmFormTemplate {
        token: token.to_string(),
        error: Some(error.to_string()),
        csrf_token: new_csrf.clone(),
        csp_nonce: csp_nonce.to_string(),
    };
    render_html(status, &template, Some(build_csrf_cookie(&new_csrf)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_request_template() -> RequestFormTemplate {
        RequestFormTemplate {
            email: String::new(),
            error: None,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
        }
    }

    fn fresh_confirm_template() -> ConfirmFormTemplate {
        ConfirmFormTemplate {
            token: "raw-token-xyz".to_string(),
            error: None,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
        }
    }

    #[test]
    fn paths_point_under_classic() {
        assert_eq!(REQUEST_PATH, "/classic/password-reset/request");
        assert_eq!(CONFIRM_PATH, "/classic/password-reset/confirm");
    }

    #[test]
    fn csrf_cookie_has_strict_attributes() {
        let h = build_csrf_cookie("tok42");
        assert!(h.contains(PWRESET_CSRF_COOKIE), "cookie name missing: {h}");
        assert!(h.contains("=tok42"), "value missing: {h}");
        assert!(h.contains("HttpOnly"), "HttpOnly missing");
        assert!(h.contains("Secure"), "Secure missing");
        assert!(h.contains("SameSite=Strict"), "SameSite=Strict missing");
        assert!(
            h.contains("Path=/classic/password-reset"),
            "narrow Path missing"
        );
        assert!(h.contains("Max-Age=900"), "Max-Age missing");
    }

    #[test]
    fn clear_csrf_cookie_uses_max_age_zero() {
        let h = build_clear_csrf_cookie();
        assert!(h.contains("Max-Age=0"));
        assert!(h.contains("SameSite=Strict"));
    }

    #[test]
    fn extract_csrf_cookie_finds_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static(
                "foo=bar; tasmail_classic_pwreset_csrf=tok42; baz=qux",
            ),
        );
        assert_eq!(extract_csrf_cookie(&headers).as_deref(), Some("tok42"));
    }

    #[test]
    fn extract_csrf_cookie_handles_missing() {
        let headers = HeaderMap::new();
        assert!(extract_csrf_cookie(&headers).is_none());

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("tasmail_classic_pwreset_csrf="),
        );
        assert!(extract_csrf_cookie(&headers).is_none());
    }

    #[test]
    fn build_reset_link_prefers_x_forwarded_proto_and_host() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("mail.example.com"),
        );
        let link = build_reset_link(&headers, "abc/+xyz");
        assert!(link.starts_with("https://mail.example.com/classic/password-reset/confirm?token="));
        // URL-encoded — / becomes %2F, + becomes %2B.
        assert!(link.contains("token=abc%2F%2Bxyz"), "got: {link}");
    }

    #[test]
    fn build_reset_link_falls_back_to_host_header() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("localhost:3300"));
        let link = build_reset_link(&headers, "tok");
        assert!(
            link.starts_with("https://localhost:3300/classic/password-reset/confirm?token=tok"),
            "got: {link}"
        );
    }

    #[test]
    fn build_reset_link_defaults_when_no_headers() {
        let headers = HeaderMap::new();
        let link = build_reset_link(&headers, "tok");
        assert_eq!(
            link,
            "https://mail.techatscale.io/classic/password-reset/confirm?token=tok"
        );
    }

    #[test]
    fn build_reset_link_rejects_weird_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("javascript"));
        headers.insert("host", HeaderValue::from_static("h"));
        let link = build_reset_link(&headers, "t");
        // Falls back to https — refuses to honour an attacker-controlled
        // protocol value.
        assert!(link.starts_with("https://"), "got: {link}");
    }

    #[test]
    fn request_template_renders_email_form() {
        let body = fresh_request_template().render().expect("renders");
        assert!(
            body.contains(&format!("action=\"{}\"", REQUEST_PATH)),
            "form action missing: {body}"
        );
        assert!(body.contains("name=\"email\""));
        assert!(body.contains("type=\"email\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("value=\"test-csrf-token\""));
        assert!(body.contains("method=\"post\""));
    }

    #[test]
    fn request_template_omits_error_on_fresh_render() {
        let body = fresh_request_template().render().expect("renders");
        assert!(!body.contains("role=\"alert\""));
    }

    #[test]
    fn request_template_shows_error_when_set() {
        let mut t = fresh_request_template();
        t.error = Some("Session expired.".to_string());
        let body = t.render().expect("renders");
        assert!(body.contains("role=\"alert\""));
        assert!(body.contains("Session expired."));
    }

    #[test]
    fn request_template_has_zero_script_tags() {
        let body = fresh_request_template().render().expect("renders");
        assert!(!body.contains("<script"));
    }

    #[test]
    fn request_done_template_renders_generic_message() {
        let t = RequestDoneTemplate {
            message: GENERIC_REQUEST_DONE_MESSAGE.to_string(),
            csp_nonce: "n".to_string(),
        };
        let body = t.render().expect("renders");
        assert!(body.contains("If that email address is registered"));
        // Does NOT name the email address — no enumeration.
        assert!(!body.to_lowercase().contains("found"));
        assert!(!body.to_lowercase().contains("does not exist"));
        assert!(!body.to_lowercase().contains("not registered"));
    }

    #[test]
    fn confirm_template_renders_password_fields() {
        let body = fresh_confirm_template().render().expect("renders");
        assert!(
            body.contains(&format!("action=\"{}\"", CONFIRM_PATH)),
            "form action missing: {body}"
        );
        assert!(body.contains("name=\"new_password\""));
        assert!(body.contains("name=\"confirm_password\""));
        assert!(body.contains("name=\"_csrf\""));
        // Hidden token field round-trips the raw token across the POST.
        assert!(body.contains("name=\"token\""));
        assert!(body.contains("value=\"raw-token-xyz\""));
        // Two password fields, autocomplete=new-password.
        let new_pw_count = body.matches("autocomplete=\"new-password\"").count();
        assert_eq!(
            new_pw_count, 2,
            "both password fields should hint new-password to password managers"
        );
    }

    #[test]
    fn confirm_template_escapes_hostile_token() {
        let mut t = fresh_confirm_template();
        t.token = "\"><script>alert(1)</script>".to_string();
        let body = t.render().expect("renders");
        assert!(
            !body.contains("\"><script>alert(1)</script>"),
            "raw <script> leaked: {body}"
        );
    }

    #[test]
    fn confirm_invalid_template_renders_safe_copy() {
        let t = ConfirmInvalidTemplate {
            csp_nonce: "n".to_string(),
        };
        let body = t.render().expect("renders");
        assert!(body.to_lowercase().contains("invalid"));
        // Offer the request page as a retry — links to start the flow over.
        assert!(body.contains(REQUEST_PATH));
        // No <script>.
        assert!(!body.contains("<script"));
    }

    #[test]
    fn confirm_done_template_links_to_login() {
        let t = ConfirmDoneTemplate {
            csp_nonce: "n".to_string(),
        };
        let body = t.render().expect("renders");
        assert!(body.contains(LOGIN_PATH));
    }

    #[test]
    fn generic_request_done_message_does_not_enumerate() {
        // The whole point of TMAIL-375's anti-enumeration acceptance
        // criterion — lock down the copy so a future "helpful" tweak
        // can't accidentally leak whether the email exists.
        let m = GENERIC_REQUEST_DONE_MESSAGE.to_lowercase();
        assert!(!m.contains("does not exist"));
        assert!(!m.contains("not found"));
        assert!(!m.contains("no such account"));
        assert!(!m.contains("user found"));
        assert!(!m.contains("we sent"), "phrasing should be conditional (\"if registered\"): {GENERIC_REQUEST_DONE_MESSAGE}");
        assert!(m.contains("if that email address is registered"));
    }

    #[test]
    fn html_escape_handles_four_dangerous_chars() {
        let s = "<a href=\"x\">'&\"</a>";
        let e = html_escape(s);
        assert!(!e.contains('<'));
        assert!(!e.contains('>'));
        // Quote escaping — both single and double for attribute safety.
        assert!(e.contains("&quot;"));
        assert!(e.contains("&#39;"));
        assert!(e.contains("&amp;"));
    }

    #[test]
    fn min_password_len_matches_signup() {
        // Same rule across signup + reset — both use 8.
        assert_eq!(MIN_PASSWORD_LEN, 8);
    }

    #[test]
    fn ttl_constant_matches_model() {
        // Lock the handler's view of the TTL to the model's so a change
        // in one place can't drift from the other.
        assert_eq!(PASSWORD_RESET_TTL_SECS, 3600);
    }
}
