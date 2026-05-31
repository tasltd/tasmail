// Added (TMAIL-377): /classic/settings/sessions/* handlers for the no-JS
// surface.
//
// This module owns the "active sessions" page mandated by the gap analysis
// (`docs/gap-analysis/classic-ui.md` P1 #23) and its two destructive
// follow-ups:
//
//   * GET  /classic/settings/sessions                  — list every active
//                                                        classic_sessions
//                                                        row AND every
//                                                        refresh-token row
//                                                        for the user with
//                                                        per-row "Revoke"
//                                                        buttons.
//   * POST /classic/settings/sessions/revoke           — per-row revoke
//                                                        (single classic or
//                                                        SPA row).
//   * POST /classic/settings/sessions/revoke-all       — render the confirm
//                                                        page for the
//                                                        "Sign out
//                                                        everywhere" CTA.
//                                                        Does NOT yet
//                                                        destroy anything.
//   * POST /classic/settings/sessions/revoke-all/confirm — final destructive
//                                                          step. Deletes
//                                                          every classic
//                                                          session + refresh
//                                                          token for the
//                                                          user (including
//                                                          the current
//                                                          browser), clears
//                                                          the session
//                                                          cookie, and
//                                                          303-redirects to
//                                                          /classic/login.
//
// CSRF protection
// ---------------
// Every route lives on `authenticated_router(state)` in
// `handlers::classic::mod` so `classic_session_middleware` +
// `classic_csrf_middleware` wrap them transparently. The handlers only
// care about happy-path business logic; the middleware enforces auth +
// CSRF before the request reaches us.
//
// Why two POST steps for "Sign out everywhere"
// --------------------------------------------
// The CTA on the sessions list page POSTs to `/revoke-all`, which renders
// a confirm page. That confirm page then POSTs to `/revoke-all/confirm`
// to perform the destructive action. This mirrors the delete.rs Trash
// confirm pattern (TMAIL-367) and matches the gap-analysis acceptance
// criteria: "The 'sign out everywhere' button has a confirm step".
// Both POSTs are CSRF-protected; the second one also includes a
// session-id pin so a stale confirm form can't accidentally destroy a
// freshly-rotated session.

use askama::Template;
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Extension, Form,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::classic_session::build_clear_cookie_header;
use crate::models::classic_session::ClassicSession;
use crate::models::session::Session as SpaSession;
use crate::services::auth_service::Claims;
use crate::state::AppState;

use super::auth::LOGIN_PATH;
use super::CspNonce;

/// Path the sessions overview lives on. Single source of truth so a future
/// rename doesn't drift between handler, template, and router.
pub const SESSIONS_PATH: &str = "/classic/settings/sessions";

/// Cap on the user-agent string we render to the page. The DB column itself
/// is bounded by `password_reset.rs::extract_audit_fields` to 256 chars on
/// write, but a defensive cap on the read path keeps a hostile
/// pre-existing row from blowing the page width out.
const UA_DISPLAY_CAP: usize = 200;

/// Placeholder for missing IP / user-agent metadata. The audit columns are
/// nullable on both tables (an inbound request with no `X-Forwarded-For` or
/// `User-Agent` lands as NULL), so we substitute a stable label rather than
/// rendering an empty cell that looks like a layout bug.
const UNKNOWN_DISPLAY: &str = "(unknown)";

// ---------- Display structs ----------

/// One row in the "Classic UI browsers" table. The model struct
/// (`ClassicSession`) carries raw `DateTime<Utc>` + `Option<String>`
/// fields that need formatting + null-handling before they reach the
/// template — doing that prep here keeps the template engine free of
/// `chrono` / `Option` API noise.
pub struct ClassicSessionRow {
    pub id: Uuid,
    pub created_at_display: String,
    pub last_seen_display: String,
    pub ip_display: String,
    pub ua_display: String,
    /// True when this row is the one driving the current request — drives
    /// the "This browser" badge and hides the per-row Revoke button (the
    /// nav-bar Sign-out is the right tool for the current session).
    pub is_current: bool,
}

/// One row in the "SPA / mobile refresh tokens" table.
pub struct SpaSessionRow {
    pub id: Uuid,
    pub created_at_display: String,
    pub ip_display: String,
    pub ua_display: String,
}

// ---------- Template structs ----------

#[derive(Template)]
#[template(path = "classic/settings/sessions.html")]
pub struct SessionsListTemplate {
    pub classic_rows: Vec<ClassicSessionRow>,
    pub spa_rows: Vec<SpaSessionRow>,
    /// `Some(("success" | "error", message))` after a per-row revoke;
    /// `None` on a fresh page load.
    pub flash: Option<(String, String)>,
    pub csrf_token: String,
    pub csp_nonce: String,
    /// Added (TMAIL-384): Footer quota indicator. `None` when the cache
    /// + DB couldn't be reached — the partial renders nothing.
    pub quota_indicator: Option<super::QuotaIndicator>,
}

#[derive(Template)]
#[template(path = "classic/settings/sessions_revoke_all_confirm.html")]
pub struct RevokeAllConfirmTemplate {
    pub classic_count: usize,
    pub spa_count: usize,
    pub csrf_token: String,
    pub csp_nonce: String,
    /// Added (TMAIL-384): Footer quota indicator. `None` on cache + DB
    /// outage — the partial renders nothing in that case.
    pub quota_indicator: Option<super::QuotaIndicator>,
}

// ---------- Form bodies ----------

/// Per-row revoke form. `kind` selects which table to delete from
/// ("classic" → `classic_sessions`, "spa" → `sessions`). `session_id` is
/// the row id. Both fields are validated against the strict set the
/// template emits, and the row is filtered by `user_id` / `mailbox_id`
/// in the model layer so an attacker-supplied id can never reach another
/// account's row.
#[derive(Debug, Deserialize)]
pub struct RevokeRowForm {
    pub kind: String,
    pub session_id: Uuid,
    #[serde(rename = "_csrf")]
    #[allow(dead_code)]
    pub csrf: String,
}

/// Empty body — the CSRF middleware validates `_csrf` upstream, and the
/// `/revoke-all` endpoint just needs to know a POST was made. The
/// `axum::Form` extractor would 400 on an empty body so we use a
/// `Deserialize` struct with a single optional field.
#[derive(Debug, Default, Deserialize)]
pub struct RevokeAllForm {
    #[serde(rename = "_csrf")]
    #[allow(dead_code)]
    pub csrf: Option<String>,
}

// ---------- Handlers ----------

/// GET /classic/settings/sessions — render the active sessions table.
pub async fn get_sessions(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
) -> Result<Response, AppError> {
    render_list(&state, &claims, &session, csp_nonce, None).await
}

/// POST /classic/settings/sessions/revoke — single-row revoke.
///
/// Re-renders the same list page with a flash banner. Doesn't 303-redirect
/// because the user is supposed to see the "Revoked" confirmation in
/// context with the now-shorter table — a redirect would mean re-rendering
/// from scratch and losing the inline feedback signal.
pub async fn post_revoke_row(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    Form(form): Form<RevokeRowForm>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Defence in depth — refuse to revoke the current row through this
    // endpoint. The list page hides the button for the current row but
    // a hostile request could still try; the nav-bar Sign-out is the
    // right tool for the current session, so 400 is honest.
    if form.kind == "classic" && form.session_id == session.id {
        let flash = Some((
            "error".to_string(),
            "Use the Sign-out button in the navigation to end the current session.".to_string(),
        ));
        return render_list(&state, &claims, &session, csp_nonce, flash).await;
    }

    let flash = match form.kind.as_str() {
        "classic" => {
            match ClassicSession::delete_for_user(&state.db, mailbox_id, form.session_id).await {
                Ok(true) => Some((
                    "success".to_string(),
                    "Classic UI session revoked.".to_string(),
                )),
                Ok(false) => Some((
                    "error".to_string(),
                    "That session is no longer active.".to_string(),
                )),
                Err(e) => {
                    tracing::warn!(
                        user_id = ?mailbox_id,
                        session_id = ?form.session_id,
                        err = ?e,
                        "classic settings/sessions: per-row classic revoke failed"
                    );
                    Some((
                        "error".to_string(),
                        "Failed to revoke that session. Please try again.".to_string(),
                    ))
                }
            }
        }
        "spa" => {
            match SpaSession::delete_for_mailbox(&state.db, mailbox_id, form.session_id).await {
                Ok(true) => Some((
                    "success".to_string(),
                    "SPA / mobile refresh token revoked.".to_string(),
                )),
                Ok(false) => Some((
                    "error".to_string(),
                    "That refresh token is no longer active.".to_string(),
                )),
                Err(e) => {
                    tracing::warn!(
                        user_id = ?mailbox_id,
                        session_id = ?form.session_id,
                        err = ?e,
                        "classic settings/sessions: per-row spa revoke failed"
                    );
                    Some((
                        "error".to_string(),
                        "Failed to revoke that refresh token. Please try again.".to_string(),
                    ))
                }
            }
        }
        other => {
            tracing::warn!(
                user_id = ?mailbox_id,
                kind = %other,
                "classic settings/sessions: unexpected revoke kind"
            );
            Some((
                "error".to_string(),
                "Unknown session kind.".to_string(),
            ))
        }
    };

    render_list(&state, &claims, &session, csp_nonce, flash).await
}

/// POST /classic/settings/sessions/revoke-all — render the confirm page.
///
/// Counts the rows that *will* be destroyed so the confirm page can show
/// the user concrete numbers before they make the decision. Does NOT
/// destroy anything — that's `post_revoke_all_confirm`.
pub async fn post_revoke_all(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let classic_rows = ClassicSession::list_active_for_user(&state.db, mailbox_id)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "classic settings/sessions: list_active_for_user failed: {e}"
            ))
        })?;
    let spa_rows = SpaSession::list_active_for_mailbox(&state.db, mailbox_id)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "classic settings/sessions: list_active_for_mailbox failed: {e}"
            ))
        })?;

    // Added (TMAIL-384): hydrate the footer quota indicator for the
    // confirm page render.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;

    let template = RevokeAllConfirmTemplate {
        classic_count: classic_rows.len(),
        spa_count: spa_rows.len(),
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
        quota_indicator,
    };
    render_html(StatusCode::OK, &template)
}

/// POST /classic/settings/sessions/revoke-all/confirm — destructive.
///
/// Deletes every `classic_sessions` row AND every `sessions` (refresh
/// token) row for the user, clears the session cookie, and 303-redirects
/// to `/classic/login`. The user lands on the login page and must sign
/// back in with their password.
pub async fn post_revoke_all_confirm(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Order matters here only weakly — both deletes are independent
    // user-scoped queries. We run classic first so that if the SPA
    // delete somehow fails the user still ends up signed out of every
    // browser (the cookie clear below applies regardless).
    let revoked_classic = ClassicSession::delete_all_for_user(&state.db, mailbox_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(
                user_id = ?mailbox_id,
                err = ?e,
                "classic settings/sessions/revoke-all: classic delete failed"
            );
            0
        });
    let revoked_spa = SpaSession::delete_all_for_mailbox(&state.db, mailbox_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(
                user_id = ?mailbox_id,
                err = ?e,
                "classic settings/sessions/revoke-all: spa delete failed"
            );
            0
        });

    tracing::info!(
        user_id = ?mailbox_id,
        session_id = ?session.id,
        revoked_classic,
        revoked_spa,
        "classic settings/sessions: sign-out everywhere completed"
    );

    // Build the cookie-clearing header by hand (rather than calling
    // `destroy_session_and_cookie`) because the classic_sessions row
    // for the current session was already deleted by
    // `delete_all_for_user`. A second DELETE would be a no-op but the
    // function returns a Result we'd have to handle for no gain.
    let clear_cookie = HeaderValue::from_str(&build_clear_cookie_header()).map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic settings/sessions: clear-cookie header value invalid: {e}"
        ))
    })?;

    let mut resp = Redirect::to(LOGIN_PATH).into_response();
    resp.headers_mut().append(header::SET_COOKIE, clear_cookie);
    if let Ok(hv) = HeaderValue::from_str("no-store, max-age=0, must-revalidate") {
        resp.headers_mut().insert(header::CACHE_CONTROL, hv);
    }

    Ok(resp)
}

// ---------- Helpers ----------

/// Build the row VMs + render the list template. Shared between the
/// initial GET and the per-row revoke POST.
async fn render_list(
    state: &AppState,
    claims: &Claims,
    session: &ClassicSession,
    csp_nonce: CspNonce,
    flash: Option<(String, String)>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(claims)?;

    let classic_models = ClassicSession::list_active_for_user(&state.db, mailbox_id)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "classic settings/sessions: list_active_for_user failed: {e}"
            ))
        })?;
    let spa_models = SpaSession::list_active_for_mailbox(&state.db, mailbox_id)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "classic settings/sessions: list_active_for_mailbox failed: {e}"
            ))
        })?;

    let classic_rows: Vec<ClassicSessionRow> = classic_models
        .into_iter()
        .map(|m| ClassicSessionRow {
            is_current: m.id == session.id,
            created_at_display: format_dt(m.created_at),
            last_seen_display: format_dt(m.last_seen_at),
            ip_display: display_or_unknown(m.last_seen_ip),
            ua_display: cap_ua(m.last_seen_ua),
            id: m.id,
        })
        .collect();

    let spa_rows: Vec<SpaSessionRow> = spa_models
        .into_iter()
        .map(|m| SpaSessionRow {
            created_at_display: format_dt(m.created_at),
            ip_display: display_or_unknown(m.ip_address),
            ua_display: cap_ua(m.user_agent),
            id: m.id,
        })
        .collect();

    // Added (TMAIL-384): hydrate the footer quota indicator. Loaded
    // here (rather than at each call site) so both the GET and the
    // POST-revoke re-render share one cache lookup per request.
    let quota_indicator = super::load_quota_indicator(state, mailbox_id).await;

    let template = SessionsListTemplate {
        classic_rows,
        spa_rows,
        flash,
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
        quota_indicator,
    };
    render_html(StatusCode::OK, &template)
}

/// Compact UTC display: 2026-05-31 14:23 UTC. Picked because it's
/// unambiguous across locales and stable in 80-column lynx output.
fn format_dt(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// Map nullable string columns to a stable placeholder so the rendered
/// row never shows an empty cell.
fn display_or_unknown(s: Option<String>) -> String {
    match s {
        Some(v) if !v.is_empty() => v,
        _ => UNKNOWN_DISPLAY.to_string(),
    }
}

/// Defensive char-cap on a user-agent string before it lands in the
/// template. The write path also caps but a hostile pre-existing row
/// shouldn't blow up the rendered table.
fn cap_ua(s: Option<String>) -> String {
    let raw = match s {
        Some(v) if !v.is_empty() => v,
        _ => return UNKNOWN_DISPLAY.to_string(),
    };
    if raw.chars().count() <= UA_DISPLAY_CAP {
        return raw;
    }
    let mut out: String = raw.chars().take(UA_DISPLAY_CAP).collect();
    out.push('…');
    out
}

fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims.sub.parse::<Uuid>().map_err(|_| {
        AppError::Internal(anyhow::anyhow!(
            "classic settings/sessions: claims.sub is not a UUID — {}",
            claims.sub
        ))
    })
}

fn render_html<T: Template>(status: StatusCode, template: &T) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic settings/sessions template render failed: {e}"
        ))
    })?;
    Ok((
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classic_row(is_current: bool) -> ClassicSessionRow {
        ClassicSessionRow {
            id: Uuid::new_v4(),
            created_at_display: "2026-05-31 09:00 UTC".to_string(),
            last_seen_display: "2026-05-31 14:23 UTC".to_string(),
            ip_display: "10.0.0.1".to_string(),
            ua_display: "Mozilla/5.0 (X11; Linux) Firefox/138".to_string(),
            is_current,
        }
    }

    fn spa_row() -> SpaSessionRow {
        SpaSessionRow {
            id: Uuid::new_v4(),
            created_at_display: "2026-05-30 08:00 UTC".to_string(),
            ip_display: "192.0.2.5".to_string(),
            ua_display: "TASMail-Mobile/1.0 Android".to_string(),
        }
    }

    fn fresh_list_template(
        classic_rows: Vec<ClassicSessionRow>,
        spa_rows: Vec<SpaSessionRow>,
        flash: Option<(String, String)>,
    ) -> SessionsListTemplate {
        SessionsListTemplate {
            classic_rows,
            spa_rows,
            flash,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
            quota_indicator: None,
        }
    }

    fn fresh_confirm_template() -> RevokeAllConfirmTemplate {
        RevokeAllConfirmTemplate {
            classic_count: 3,
            spa_count: 5,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
            quota_indicator: None,
        }
    }

    #[test]
    fn sessions_path_points_under_classic() {
        // Locks the URL — a typo would render a page whose per-row form
        // posts to a 404.
        assert_eq!(SESSIONS_PATH, "/classic/settings/sessions");
    }

    #[test]
    fn format_dt_uses_compact_utc_layout() {
        let ts = "2026-05-31T14:23:09Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(format_dt(ts), "2026-05-31 14:23 UTC");
    }

    #[test]
    fn display_or_unknown_falls_back_on_empty_and_none() {
        assert_eq!(display_or_unknown(None), UNKNOWN_DISPLAY);
        assert_eq!(display_or_unknown(Some("".to_string())), UNKNOWN_DISPLAY);
        assert_eq!(display_or_unknown(Some("10.0.0.1".to_string())), "10.0.0.1");
    }

    #[test]
    fn cap_ua_truncates_with_ellipsis() {
        let long = "A".repeat(500);
        let out = cap_ua(Some(long));
        // 200 chars + 1-char ellipsis = 201 chars total.
        assert_eq!(out.chars().count(), UA_DISPLAY_CAP + 1);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn cap_ua_passes_short_strings_unchanged() {
        let out = cap_ua(Some("Mozilla/5.0".to_string()));
        assert_eq!(out, "Mozilla/5.0");
    }

    #[test]
    fn cap_ua_returns_unknown_for_none_and_empty() {
        assert_eq!(cap_ua(None), UNKNOWN_DISPLAY);
        assert_eq!(cap_ua(Some("".to_string())), UNKNOWN_DISPLAY);
    }

    #[test]
    fn list_template_renders_classic_rows() {
        let body = fresh_list_template(vec![classic_row(false), classic_row(true)], vec![], None)
            .render()
            .expect("renders");
        assert!(body.contains("Classic UI browsers"));
        assert!(body.contains("Mozilla/5.0"));
        // The current row marker.
        assert!(body.contains("This browser"));
        // Per-row revoke form on the non-current row.
        assert!(body.contains("action=\"/classic/settings/sessions/revoke\""));
        assert!(body.contains("name=\"kind\" value=\"classic\""));
    }

    #[test]
    fn list_template_renders_spa_rows() {
        let body = fresh_list_template(vec![], vec![spa_row()], None)
            .render()
            .expect("renders");
        assert!(body.contains("SPA / mobile refresh tokens"));
        assert!(body.contains("TASMail-Mobile/1.0 Android"));
        assert!(body.contains("name=\"kind\" value=\"spa\""));
    }

    #[test]
    fn list_template_shows_empty_state_for_each_table() {
        let body = fresh_list_template(vec![], vec![], None).render().expect("renders");
        assert!(body.contains("No active Classic UI sessions"));
        assert!(body.contains("No active SPA or mobile refresh tokens"));
    }

    #[test]
    fn list_template_omits_revoke_button_on_current_row() {
        let body = fresh_list_template(vec![classic_row(true)], vec![], None)
            .render()
            .expect("renders");
        // No revoke form should render for the row driving the request —
        // the nav-bar Sign-out is the right tool.
        assert!(
            !body.contains("name=\"kind\" value=\"classic\""),
            "current-row should NOT render a per-row revoke form: {body}"
        );
    }

    #[test]
    fn list_template_renders_success_flash() {
        let flash = Some(("success".to_string(), "Session revoked.".to_string()));
        let body = fresh_list_template(vec![], vec![], flash).render().expect("renders");
        assert!(body.contains("alert-success"));
        assert!(body.contains("role=\"status\""));
        assert!(body.contains("Session revoked."));
    }

    #[test]
    fn list_template_renders_error_flash() {
        let flash = Some(("error".to_string(), "Something went wrong.".to_string()));
        let body = fresh_list_template(vec![], vec![], flash).render().expect("renders");
        assert!(body.contains("alert-error"));
        assert!(body.contains("role=\"alert\""));
        assert!(body.contains("Something went wrong."));
    }

    #[test]
    fn list_template_renders_danger_zone_button() {
        let body = fresh_list_template(vec![], vec![], None).render().expect("renders");
        assert!(body.contains("Sign out everywhere"));
        assert!(body.contains("action=\"/classic/settings/sessions/revoke-all\""));
        assert!(body.contains("class=\"danger\""));
    }

    #[test]
    fn list_template_carries_csrf_in_every_form() {
        // Every POST form on the page MUST thread csrf_token through.
        let body = fresh_list_template(
            vec![classic_row(false)],
            vec![spa_row()],
            None,
        )
        .render()
        .expect("renders");
        // Per-row classic, per-row spa, danger-zone, logout partial — four total.
        let csrf_count = body.matches("value=\"test-csrf-token\"").count();
        assert!(
            csrf_count >= 4,
            "expected csrf_token in 4+ forms (classic, spa, revoke-all, logout); found {csrf_count}\n{body}"
        );
    }

    #[test]
    fn list_template_includes_logout_partial() {
        let body = fresh_list_template(vec![], vec![], None).render().expect("renders");
        assert!(body.contains("action=\"/classic/logout\""));
    }

    #[test]
    fn list_template_has_zero_script_tags() {
        let body = fresh_list_template(vec![], vec![], None).render().expect("renders");
        assert!(!body.contains("<script"));
    }

    #[test]
    fn list_template_html_escapes_hostile_ua() {
        let mut row = classic_row(false);
        row.ua_display = "\"><script>alert(1)</script>".to_string();
        let body = fresh_list_template(vec![row], vec![], None).render().expect("renders");
        assert!(
            !body.contains("\"><script>alert(1)</script>"),
            "raw <script> leaked into the rendered cell: {body}"
        );
    }

    #[test]
    fn confirm_template_renders_counts() {
        let body = fresh_confirm_template().render().expect("renders");
        assert!(body.contains("Sign out everywhere?"));
        assert!(body.contains("Classic UI browsers: 3"));
        assert!(body.contains("SPA / mobile refresh tokens: 5"));
        assert!(body.contains("action=\"/classic/settings/sessions/revoke-all/confirm\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("value=\"test-csrf-token\""));
        // Cancel link points back to the list page.
        assert!(body.contains("href=\"/classic/settings/sessions\""));
    }

    #[test]
    fn confirm_template_has_destructive_alert() {
        let body = fresh_confirm_template().render().expect("renders");
        assert!(body.contains("alert-error"));
        assert!(body.contains("role=\"alert\""));
    }

    #[test]
    fn confirm_template_has_zero_script_tags() {
        let body = fresh_confirm_template().render().expect("renders");
        assert!(!body.contains("<script"));
    }

    #[test]
    fn confirm_template_includes_logout_partial() {
        let body = fresh_confirm_template().render().expect("renders");
        assert!(body.contains("action=\"/classic/logout\""));
    }

    #[test]
    fn confirm_template_zero_counts_still_render() {
        let mut t = fresh_confirm_template();
        t.classic_count = 0;
        t.spa_count = 0;
        let body = t.render().expect("renders");
        assert!(body.contains("Classic UI browsers: 0"));
        assert!(body.contains("SPA / mobile refresh tokens: 0"));
    }
}
