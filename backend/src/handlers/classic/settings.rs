// Added (TMAIL-376): /classic/settings/* handlers for the no-JS surface.
//
// First inhabitant: GET / POST `/classic/settings/password` — the
// change-password form mandated by the gap analysis (`docs/gap-analysis/
// classic-ui.md` P1 #22). The handler:
//
//   1. Pulls the verified `ClassicSession` row + `Claims` out of request
//      extensions (the `classic_session_middleware` did the cookie ↔ session
//      lookup before the request reached us).
//   2. On GET: renders `templates/classic/settings/password.html` with the
//      session-scoped CSRF token threaded into the hidden `_csrf` field.
//   3. On POST: validates the form, re-verifies the *current* password via
//      `auth_service::evaluate_password_login` (so the per-account lockout
//      / counter book-keeping is exactly the same as a fresh login attempt
//      — typing the wrong password into the change-password screen counts
//      toward the brute-force threshold), runs `auth_service::change_password`,
//      then revokes every *other* classic session and every SPA refresh
//      token for the user. The current browser stays signed in (the
//      session row we are riding on is preserved) so the redirect can
//      land on the success page without bouncing the user back to login.
//
// Why not also revoke the current classic session
// -----------------------------------------------
// The user just typed their current password to prove they hold the
// account. Forcing them to log in again on the same browser would be
// hostile UX with no security benefit — a single round-trip later the
// new session would be back. The acceptance criteria explicitly call
// out "keeping the current Classic session alive".
//
// Why revoke EVERY SPA refresh token (not "every other")
// ------------------------------------------------------
// The SPA refresh token does not carry a session-id we can pin to "the
// one the user is currently on". From the change-password form's
// perspective every SPA session is *another* surface — the user is in
// the Classic UI right now. Wiping all of them forces every SPA / mobile
// app to re-auth, which is the whole point of changing the password
// after a suspected leak.
//
// CSP + CSRF
// ----------
// The route lives on `authenticated_router(state)` in `handlers::classic::mod`,
// so both `classic_session_middleware` (cookie → session + extensions)
// AND `classic_csrf_middleware` (validates `_csrf` form field against
// the session row's csrf_token, constant-time compare) wrap us
// transparently. No per-handler CSRF code lives here.

use askama::Template;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::classic_session::ClassicSession;
use crate::models::session::Session;
use crate::services::auth_service::{change_password, evaluate_password_login, Claims};
use crate::state::AppState;

use super::CspNonce;

/// Path the change-password form posts to. Single source of truth so a
/// future rename doesn't drift between handler, template, and router.
pub const PASSWORD_PATH: &str = "/classic/settings/password";

/// Minimum password length. Matches the signup wizard (TMAIL-374) and the
/// password-reset confirm page (TMAIL-375) so the rules don't drift across
/// the three surfaces a user can land a new password through.
const MIN_PASSWORD_LEN: usize = 8;

/// Inbox path used as the post-success "back to inbox" CTA target on the
/// success template. Kept here (rather than re-using `auth::INBOX_PATH`) so
/// a future settings landing page can quietly redirect somewhere else
/// without touching auth.rs.
#[allow(dead_code)]
const INBOX_PATH: &str = "/classic/folders/INBOX";

// ---------- Template structs ----------

#[derive(Template)]
#[template(path = "classic/settings/password.html")]
pub struct PasswordFormTemplate {
    /// Some("…") on validation failure (CSRF would never reach us — the
    /// middleware short-circuits); None on a fresh GET render.
    pub error: Option<String>,
    /// Per-session CSRF token. Threaded into both the hidden `_csrf`
    /// input on this form AND the included `_logout_form.html` partial.
    pub csrf_token: String,
    pub csp_nonce: String,
}

#[derive(Template)]
#[template(path = "classic/settings/password_done.html")]
pub struct PasswordDoneTemplate {
    pub revoked_classic: u64,
    pub revoked_spa: u64,
    /// Carries the (still-valid, same id) session token so the success
    /// page's logout button works without a refresh.
    pub csrf_token: String,
    pub csp_nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
    /// Validated by `classic_csrf_middleware` before this handler runs,
    /// but axum's `Form` extractor still needs the field on the struct
    /// so deserialisation doesn't fail with "missing field" when the
    /// middleware forwards the body to us.
    #[serde(rename = "_csrf")]
    #[allow(dead_code)]
    pub csrf: String,
}

// ---------- Handlers ----------

/// GET /classic/settings/password — render the empty form.
pub async fn get_password(
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
) -> Result<Response, AppError> {
    let template = PasswordFormTemplate {
        error: None,
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
    };
    render_html(StatusCode::OK, &template)
}

/// POST /classic/settings/password — verify current, set new, revoke peers.
pub async fn post_password(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: axum::http::HeaderMap,
    axum::Form(form): axum::Form<PasswordForm>,
) -> Result<Response, AppError> {
    let csrf_token = session.csrf_token.clone();
    let csp_nonce_str = csp_nonce.into_string();

    // 1) Cheap form validation up front — same shape (and same error
    //    copy) as the password reset confirm page so the user sees the
    //    same message whichever path they took.
    if form.new_password.len() < MIN_PASSWORD_LEN {
        return render_form_error(
            &format!("Password must be at least {MIN_PASSWORD_LEN} characters long."),
            StatusCode::BAD_REQUEST,
            &csrf_token,
            &csp_nonce_str,
        );
    }
    if form.new_password != form.confirm_password {
        return render_form_error(
            "New passwords do not match.",
            StatusCode::BAD_REQUEST,
            &csrf_token,
            &csp_nonce_str,
        );
    }
    if form.new_password == form.current_password {
        // Defence-in-depth — refusing the no-op rotation avoids the
        // confusing "I changed it but it didn't change" support
        // ticket, and discourages users from satisfying a future
        // password-policy reminder by re-typing their existing
        // password.
        return render_form_error(
            "New password must be different from your current password.",
            StatusCode::BAD_REQUEST,
            &csrf_token,
            &csp_nonce_str,
        );
    }

    // 2) Re-verify the current password through the SAME helper the
    //    login form uses. This:
    //      * Honours the per-account lockout window (a brute-force
    //        attacker who somehow got hold of a live session cookie
    //        can't grind passwords against this endpoint either — the
    //        same threshold + duration + counter that protects /login
    //        protects us here).
    //      * Audits "auth.locked_attempt" / "auth.locked" / "auth.lockout_cleared"
    //        rows so the audit-log trail matches the login surface.
    //      * Produces an `AccountLocked` error (rendered by the global
    //        error layer with HTTP 423) on a just-locked or
    //        already-locked transition.
    let (ip, ua) = extract_audit_fields(&headers);
    let verify = evaluate_password_login(
        &state.db,
        &state.config.lockout,
        &claims.username,
        &form.current_password,
        ip.as_deref(),
        ua.as_deref(),
    )
    .await;
    let mailbox = match verify {
        Ok(m) => m,
        Err(AppError::Unauthorized(_)) => {
            // Re-render the form with a 400 (NOT a 401) so the user
            // stays on the page instead of being bounced to login —
            // they ARE authenticated, they just typed the wrong
            // current password.
            return render_form_error(
                "The current password you entered is incorrect.",
                StatusCode::BAD_REQUEST,
                &csrf_token,
                &csp_nonce_str,
            );
        }
        Err(other) => return Err(other), // AccountLocked / Internal — bubble up.
    };

    // 3) Defensive: the verified mailbox's id MUST match the claims sub
    //    (the session cookie's subject). A mismatch means the cookie
    //    points at one user but the username field on the JWT resolves
    //    to a different mailbox — which would indicate either a stale
    //    claims cache or a malicious crafted cookie. Refuse loudly.
    let mailbox_id: Uuid = claims.sub.parse().map_err(|_| {
        AppError::Internal(anyhow::anyhow!(
            "classic settings/password: claims.sub is not a UUID — {}",
            claims.sub
        ))
    })?;
    if mailbox.id != mailbox_id {
        tracing::error!(
            claims_sub = %claims.sub,
            resolved_mailbox = %mailbox.id,
            "classic settings/password: claims.sub != resolved mailbox.id — refusing"
        );
        return Err(AppError::Unauthorized(
            "session does not match resolved mailbox".to_string(),
        ));
    }

    // 4) Hash + persist the new password. A failure here is fatal
    //    (rolling back would not be possible anyway since
    //    `change_password` does its own commit) so we surface the
    //    error.
    let updated = change_password(&state.db, mailbox.id, &form.new_password).await?;
    if !updated {
        return Err(AppError::Internal(anyhow::anyhow!(
            "classic settings/password: update_password returned no-rows-affected for mailbox {}",
            mailbox.id
        )));
    }

    // 5) Revoke every OTHER classic session for this user, and EVERY
    //    SPA refresh token. Failures are logged but do NOT roll back
    //    the password change — the user can still sign in with the
    //    new password, and the leftover sessions still hold a hashed
    //    refresh token that becomes useless once their next refresh
    //    rotates (or the next cleanup sweep deletes them).
    let revoked_classic = match ClassicSession::delete_others_for_user(
        &state.db,
        mailbox.id,
        session.id,
    )
    .await
    {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(
                user_id = ?mailbox.id,
                err = ?e,
                "failed to revoke other classic_sessions after password change"
            );
            0
        }
    };
    let revoked_spa = match Session::delete_all_for_mailbox(&state.db, mailbox.id).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(
                user_id = ?mailbox.id,
                err = ?e,
                "failed to revoke SPA refresh sessions after password change"
            );
            0
        }
    };

    tracing::info!(
        user_id = ?mailbox.id,
        session_id = ?session.id,
        revoked_classic,
        revoked_spa,
        "classic password change accepted — current session kept alive, peers revoked"
    );

    // 6) Render the success page. We intentionally render inline (a 200
    //    OK) rather than 303-redirecting because:
    //      * The `revoked_classic` / `revoked_spa` counts are
    //        per-request and would be lost across a redirect (or
    //        require a `?revoked_classic=N` query string, which is
    //        information leakage if the URL ends up in
    //        browser history / referer headers).
    //      * The current browser is still authenticated — there is no
    //        cookie or session state to clear from the response, so a
    //        plain 200 is honest.
    let template = PasswordDoneTemplate {
        revoked_classic,
        revoked_spa,
        csrf_token,
        csp_nonce: csp_nonce_str,
    };
    render_html(StatusCode::OK, &template)
}

// ---------- Helpers ----------

/// Render an Askama template as an HTML response.
fn render_html<T: Template>(status: StatusCode, template: &T) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic settings/password template render failed: {e}"
        ))
    })?;
    Ok((
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

/// Re-render the form with an error banner. Re-uses the session-scoped
/// CSRF token because the session row itself is unchanged across a
/// validation failure (only the password fields are rejected, the auth
/// state is fine).
fn render_form_error(
    error: &str,
    status: StatusCode,
    csrf_token: &str,
    csp_nonce: &str,
) -> Result<Response, AppError> {
    let template = PasswordFormTemplate {
        error: Some(error.to_string()),
        csrf_token: csrf_token.to_string(),
        csp_nonce: csp_nonce.to_string(),
    };
    render_html(status, &template)
}

/// First-hop X-Forwarded-For + truncated User-Agent for the audit columns.
/// Same shape `password_reset.rs::extract_audit_fields` produces — kept
/// duplicated rather than extracted to a shared util because each module's
/// "what counts as an audit field" rule could plausibly diverge in the
/// future (e.g. a settings POST audited slightly differently from a
/// login).
fn extract_audit_fields(
    headers: &axum::http::HeaderMap,
) -> (Option<String>, Option<String>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_form_template() -> PasswordFormTemplate {
        PasswordFormTemplate {
            error: None,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
        }
    }

    fn fresh_done_template() -> PasswordDoneTemplate {
        PasswordDoneTemplate {
            revoked_classic: 2,
            revoked_spa: 5,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
        }
    }

    #[test]
    fn password_path_points_under_classic() {
        // Locks in the URL — a typo would render a form that posts to a
        // 404 endpoint and silently fail.
        assert_eq!(PASSWORD_PATH, "/classic/settings/password");
    }

    #[test]
    fn min_password_len_matches_signup_and_reset() {
        // Same rule across signup + reset + change — all three use 8.
        // Drift here would mean the user could pick a password on signup
        // that the change-password form refuses, which is confusing.
        // The matching constants in handlers::classic::{signup,password_reset}
        // are private to those modules, so this assertion only locks
        // down the local copy; the constant in each module is independently
        // pinned to 8 by its own test (`min_password_len_matches_signup`
        // / `min_password_len_matches_signup` in those modules).
        assert_eq!(MIN_PASSWORD_LEN, 8);
    }

    #[test]
    fn form_template_renders_three_password_fields() {
        let body = fresh_form_template().render().expect("renders");
        assert!(
            body.contains(&format!("action=\"{}\"", PASSWORD_PATH)),
            "form action missing: {body}"
        );
        assert!(body.contains("name=\"current_password\""));
        assert!(body.contains("name=\"new_password\""));
        assert!(body.contains("name=\"confirm_password\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("value=\"test-csrf-token\""));
        assert!(body.contains("method=\"post\""));

        // Autocomplete hints: current-password for the existing one, then
        // new-password TWICE for the new + confirm pair. Wrong values
        // here would break password manager integration on touch
        // devices where typing a 12+ character random password is
        // miserable.
        assert!(
            body.contains("autocomplete=\"current-password\""),
            "current_password must hint current-password to managers"
        );
        let new_pw_count = body.matches("autocomplete=\"new-password\"").count();
        assert_eq!(
            new_pw_count, 2,
            "both new + confirm password fields should hint new-password"
        );
    }

    #[test]
    fn form_template_omits_error_on_fresh_render() {
        let body = fresh_form_template().render().expect("renders");
        // No `role="alert"` means no banner — sanity check that the
        // `{% if %}` branch in the template actually gates on `error`.
        assert!(!body.contains("role=\"alert\""));
    }

    #[test]
    fn form_template_shows_error_when_set() {
        let mut t = fresh_form_template();
        t.error = Some("Current password is incorrect.".to_string());
        let body = t.render().expect("renders");
        assert!(body.contains("role=\"alert\""));
        assert!(body.contains("Current password is incorrect."));
    }

    #[test]
    fn form_template_has_zero_script_tags() {
        // Hard rule across the whole Classic UI surface — TMAIL-368
        // CSP would block any inline script anyway, but lock the
        // markup down too.
        let body = fresh_form_template().render().expect("renders");
        assert!(!body.contains("<script"));
    }

    #[test]
    fn form_template_includes_logout_partial_inside_nav() {
        // The logout partial overrides base.html's `logout_form` block
        // so the Sign-out button travels with this page like every
        // other authenticated template. Without this assertion, a
        // future refactor that drops the override would silently
        // strand the user with no obvious way out.
        let body = fresh_form_template().render().expect("renders");
        assert!(
            body.contains("action=\"/classic/logout\""),
            "logout partial must render its POST form on the change-password page"
        );
    }

    #[test]
    fn form_template_html_escapes_hostile_csrf_token() {
        // Defence-in-depth: a hostile csrf_token containing HTML chars
        // MUST be inert in the rendered output (Askama auto-escapes
        // for the .html extension, but lock it down).
        let mut t = fresh_form_template();
        t.csrf_token = "\"><script>alert(1)</script>".to_string();
        let body = t.render().expect("renders");
        assert!(
            !body.contains("\"><script>alert(1)</script>"),
            "raw <script> leaked into the value attribute: {body}"
        );
    }

    #[test]
    fn done_template_renders_revoked_counts() {
        let body = fresh_done_template().render().expect("renders");
        // Show both numbers so the user has a concrete signal of what
        // got cleaned up. "2" + "5" from fresh_done_template above.
        assert!(body.contains("signed out: 2"));
        assert!(body.contains("revoked: 5"));
        // Success banner so it doesn't look like a validation failure.
        assert!(body.contains("alert-success"));
        assert!(body.contains("role=\"status\""));
        // Back-to-inbox CTA.
        assert!(body.contains("href=\"/classic/folders/INBOX\""));
    }

    #[test]
    fn done_template_zero_counts_still_render() {
        // When the user had no other classic sessions and no SPA
        // sessions, the template should still render cleanly — a
        // panic-on-empty bug would leave the user staring at a 500
        // after a successful password change.
        let mut t = fresh_done_template();
        t.revoked_classic = 0;
        t.revoked_spa = 0;
        let body = t.render().expect("renders");
        assert!(body.contains("signed out: 0"));
        assert!(body.contains("revoked: 0"));
    }

    #[test]
    fn done_template_includes_logout_partial() {
        // The success page is still authenticated, so the nav logout
        // button must remain present.
        let body = fresh_done_template().render().expect("renders");
        assert!(body.contains("action=\"/classic/logout\""));
    }

    #[test]
    fn done_template_has_zero_script_tags() {
        let body = fresh_done_template().render().expect("renders");
        assert!(!body.contains("<script"));
    }

    #[test]
    fn extract_audit_fields_pulls_first_forwarded_ip() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            axum::http::HeaderValue::from_static("10.0.0.1, 10.0.0.2"),
        );
        headers.insert(
            header::USER_AGENT,
            axum::http::HeaderValue::from_static("Mozilla/5.0 test"),
        );
        let (ip, ua) = extract_audit_fields(&headers);
        assert_eq!(ip.as_deref(), Some("10.0.0.1"));
        assert_eq!(ua.as_deref(), Some("Mozilla/5.0 test"));
    }

    #[test]
    fn extract_audit_fields_truncates_long_ua() {
        // 256-char cap so a hostile UA can't fill the audit columns.
        let mut headers = axum::http::HeaderMap::new();
        let long_ua = "A".repeat(1024);
        headers.insert(
            header::USER_AGENT,
            axum::http::HeaderValue::from_str(&long_ua).unwrap(),
        );
        let (_, ua) = extract_audit_fields(&headers);
        assert_eq!(ua.as_deref().map(|s| s.len()), Some(256));
    }

    #[test]
    fn extract_audit_fields_handles_missing_headers() {
        let headers = axum::http::HeaderMap::new();
        let (ip, ua) = extract_audit_fields(&headers);
        assert!(ip.is_none());
        assert!(ua.is_none());
    }
}
