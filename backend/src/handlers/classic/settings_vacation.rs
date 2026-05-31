// Added (TMAIL-379): GET + POST /classic/settings/vacation — vacation
// responder / auto-reply settings page for the no-JS Classic UI surface
// (gap-analysis P1 #25).
//
// Surface
// -------
//   * GET  /classic/settings/vacation
//       Renders the form prefilled from the user's current `auto_reply_rules`
//       row (if any). Fields: enable toggle, subject, body (plain text),
//       start_date, end_date, "external senders only" checkbox.
//
//   * POST /classic/settings/vacation   (form-urlencoded)
//       Upserts the user's auto_reply_rules row through the existing
//       `models::auto_reply::AutoReplyRule::upsert` path (the same model
//       backing /api/auto-reply). Validates:
//         - subject  trim, 1..=MAX_SUBJECT_LEN bytes
//         - body     trim, 1..=MAX_BODY_LEN bytes
//         - dates    end >= start (when both present)
//         - dates    within DATE_LOWER_BOUND_YEAR..=DATE_UPPER_BOUND_YEAR
//       On any validation failure: re-renders the form with the submitted
//       values + an inline error banner (no 303). On success: 303 → self
//       with `?flash=saved`.
//
// Model mapping
// -------------
// The classic form intentionally exposes a SUBSET of the SPA's
// `VacationResponder` component:
//
// | Form field           | DB column              | Notes                          |
// | -------------------- | ---------------------- | ------------------------------ |
// | enabled (checkbox)   | enabled                | "Enable vacation responder"    |
// | subject              | subject                | TEXT, default "Out of Office"  |
// | body                 | body_text              | TEXT, plain text only          |
// | start_date           | start_date             | YYYY-MM-DD → 00:00:00 UTC      |
// | end_date             | end_date               | YYYY-MM-DD → 23:59:59 UTC      |
// | external_only        | exclude_lists          | "skip mailing lists / bulk"    |
//
// `body_html` is left NULL (plain-text only on the classic surface), and
// `reply_to_all` is left at whatever's already on the row (default false
// on a brand-new row). This keeps the no-JS form scoped to what the gap
// analysis spec calls out ("enable + start + end + subject + body +
// external-only checkbox") without adding fields a phone-on-2G user would
// have to scroll past.
//
// Dates
// -----
// The form uses `<input type="date">` so the user picks a calendar day
// without having to think about timezones. The handler treats the picked
// day as UTC: start_date → that day at 00:00:00Z, end_date → that day at
// 23:59:59Z. That keeps the cross-timezone semantics simple and matches
// the "out of office Mon–Fri" mental model a beta customer has — close
// enough for vacation responders; anyone needing minute-precision can
// still hit the SPA.
//
// CSRF + CSP
// ----------
// The route lives on `authenticated_router(state)` in
// `handlers::classic::mod`, so both `classic_session_middleware` and
// `classic_csrf_middleware` wrap us transparently. No per-handler CSRF
// code lives here.

use askama::Template;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::auto_reply::{AutoReplyRule, UpsertAutoReply};
use crate::models::classic_session::ClassicSession;
use crate::services::auth_service::Claims;
use crate::state::AppState;

use super::CspNonce;

/// Path the form posts to. Single source of truth so a future rename
/// doesn't drift between handler, template, and router.
pub const VACATION_PATH: &str = "/classic/settings/vacation";

/// Cap on the subject line. RFC 5322 doesn't fix a hard limit but most
/// MTAs choke past ~998 octets; 256 bytes is comfortably under that and
/// past any realistic "Out of office until 2026-09-01" string.
pub const MAX_SUBJECT_LEN: usize = 256;

/// Cap on the auto-reply body. The auto-replier ships this on every
/// matching inbound mail; 8 KiB matches the signature cap (see
/// settings_signature) and keeps a hostile draft from amplifying mailbox
/// IO per replied-to sender.
pub const MAX_BODY_LEN: usize = 8 * 1024;

/// Lower bound for picked dates. A vacation "start date" in the 1970s
/// is a typo, not a vacation — reject early with a clear error rather
/// than silently storing the value.
pub const DATE_LOWER_BOUND_YEAR: i32 = 2020;

/// Upper bound for picked dates. 50-year horizon is far enough that no
/// realistic vacation hits it; anything past is a typo (or year-1970-
/// rollover from a malformed `<input type="date">`).
pub const DATE_UPPER_BOUND_YEAR: i32 = 2075;

/// Flash kinds carried in the post-POST `?flash=` query. Lives in the URL
/// (no server-side flash store) so the surface stays stateless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashKind {
    Saved,
    Error,
}

impl FlashKind {
    fn as_str(self) -> &'static str {
        match self {
            FlashKind::Saved => "saved",
            FlashKind::Error => "error",
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct VacationQuery {
    #[serde(default)]
    pub flash: Option<String>,
    #[serde(default)]
    pub msg: Option<String>,
}

// ---------- Template struct ----------

#[derive(Template)]
#[template(path = "classic/settings/vacation.html")]
pub struct VacationFormTemplate {
    pub enabled: bool,
    pub subject: String,
    pub body: String,
    /// YYYY-MM-DD or empty string. `<input type="date">` accepts both
    /// shapes — empty means "no date picked".
    pub start_date: String,
    pub end_date: String,
    pub external_only: bool,
    /// Optional banner above the form. Some(("success"|"error", msg)).
    pub flash: Option<(String, String)>,
    /// Session-scoped CSRF token. Threaded into the hidden `_csrf` field
    /// on the form AND the logout partial.
    pub csrf_token: String,
    /// Per-request CSP nonce required by base.html (TMAIL-356).
    pub csp_nonce: String,
    /// Added (TMAIL-384): Footer quota indicator. `None` on cache + DB
    /// outage — the partial renders nothing.
    pub quota_indicator: Option<super::QuotaIndicator>,
}

#[derive(Debug, Deserialize)]
pub struct VacationForm {
    /// Checkbox: `enabled=on` when ticked, absent when not.
    #[serde(default)]
    pub enabled: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    /// YYYY-MM-DD from `<input type="date">`. Empty when unset.
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    /// Checkbox: `external_only=on` when ticked, absent when not.
    #[serde(default)]
    pub external_only: Option<String>,
    /// Validated by `classic_csrf_middleware` before this handler runs.
    #[serde(rename = "_csrf")]
    #[allow(dead_code)]
    pub csrf: String,
}

// ---------- Handlers ----------

/// GET /classic/settings/vacation — render the form prefilled from the
/// user's saved auto-reply rule (if any).
pub async fn get_vacation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    axum::extract::Query(query): axum::extract::Query<VacationQuery>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let existing = AutoReplyRule::find_by_mailbox(&state.db, mailbox_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load auto-reply rule: {e}")))?;

    // Added (TMAIL-384): hydrate the footer quota indicator. Loaded once
    // per GET; cache-first.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;
    let csp_nonce_str = csp_nonce.into_string();

    let template = match existing {
        Some(rule) => VacationFormTemplate {
            enabled: rule.enabled,
            subject: rule.subject,
            body: rule.body_text,
            start_date: rule.start_date.map(format_date_for_input).unwrap_or_default(),
            end_date: rule.end_date.map(format_date_for_input).unwrap_or_default(),
            external_only: rule.exclude_lists,
            flash: build_flash(&query),
            csrf_token: session.csrf_token.clone(),
            csp_nonce: csp_nonce_str,
            quota_indicator,
        },
        None => VacationFormTemplate {
            enabled: false,
            subject: "Out of Office".to_string(),
            body: String::new(),
            start_date: String::new(),
            end_date: String::new(),
            // Default for a brand-new responder: don't reply to mailing
            // lists. Matches the SPA component's default.
            external_only: true,
            flash: build_flash(&query),
            csrf_token: session.csrf_token.clone(),
            csp_nonce: csp_nonce_str,
            quota_indicator,
        },
    };

    render_html(StatusCode::OK, &template)
}

/// POST /classic/settings/vacation — validate + upsert the row.
pub async fn post_vacation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    axum::Form(form): axum::Form<VacationForm>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let csrf_token = session.csrf_token.clone();
    let csp_nonce_str = csp_nonce.into_string();

    // Added (TMAIL-384): hydrate the footer quota indicator once at the
    // top of post_vacation so every render_error branch (and the success
    // redirect — no template body) carries the same indicator the GET
    // path would have shown.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;

    let enabled = form.enabled.is_some();
    let external_only = form.external_only.is_some();
    let subject = form.subject.as_deref().unwrap_or("").trim().to_string();
    let body = form.body.as_deref().unwrap_or("").trim().to_string();
    let start_date_raw = form.start_date.as_deref().unwrap_or("").trim().to_string();
    let end_date_raw = form.end_date.as_deref().unwrap_or("").trim().to_string();

    // PURPOSE: every validation branch re-renders the form with the
    // submitted values so the user doesn't lose their typing. Build the
    // template-state-on-error helper closure once to keep the bodies tight.
    let render_error = |status: StatusCode, msg: &str| -> Result<Response, AppError> {
        let tpl = VacationFormTemplate {
            enabled,
            subject: subject.clone(),
            body: body.clone(),
            start_date: start_date_raw.clone(),
            end_date: end_date_raw.clone(),
            external_only,
            flash: Some(("error".to_string(), msg.to_string())),
            csrf_token: csrf_token.clone(),
            csp_nonce: csp_nonce_str.clone(),
            quota_indicator: quota_indicator.clone(),
        };
        render_html(status, &tpl)
    };

    if subject.is_empty() {
        return render_error(StatusCode::BAD_REQUEST, "Subject cannot be empty.");
    }
    if subject.len() > MAX_SUBJECT_LEN {
        return render_error(
            StatusCode::BAD_REQUEST,
            &format!(
                "Subject is too long ({} bytes — max {} bytes).",
                subject.len(),
                MAX_SUBJECT_LEN
            ),
        );
    }
    if body.is_empty() {
        return render_error(StatusCode::BAD_REQUEST, "Message body cannot be empty.");
    }
    if body.len() > MAX_BODY_LEN {
        return render_error(
            StatusCode::BAD_REQUEST,
            &format!(
                "Message body is too long ({} bytes — max {} bytes).",
                body.len(),
                MAX_BODY_LEN
            ),
        );
    }

    let start_date = match parse_optional_date(&start_date_raw, /*end_of_day=*/ false) {
        Ok(opt) => opt,
        Err(msg) => return render_error(StatusCode::BAD_REQUEST, &format!("Start date: {msg}")),
    };
    let end_date = match parse_optional_date(&end_date_raw, /*end_of_day=*/ true) {
        Ok(opt) => opt,
        Err(msg) => return render_error(StatusCode::BAD_REQUEST, &format!("End date: {msg}")),
    };
    if let (Some(start), Some(end)) = (start_date, end_date)
        && end < start
    {
        return render_error(
            StatusCode::BAD_REQUEST,
            "End date must be on or after the start date.",
        );
    }

    // Preserve the existing `reply_to_all` so the no-JS form doesn't
    // silently un-tick an option the SPA owns. The SPA's component
    // controls reply_to_all separately; this form only owns the subset
    // called out in the spec.
    let existing_reply_to_all = AutoReplyRule::find_by_mailbox(&state.db, mailbox_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load existing rule: {e}")))?
        .map(|r| r.reply_to_all)
        .unwrap_or(false);

    let upsert = UpsertAutoReply {
        enabled,
        subject,
        body_text: body,
        body_html: None,
        start_date,
        end_date,
        reply_to_all: Some(existing_reply_to_all),
        exclude_lists: Some(external_only),
    };

    AutoReplyRule::upsert(&state.db, mailbox_id, &upsert)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("upsert auto-reply: {e}")))?;

    tracing::info!(
        user_id = ?mailbox_id,
        enabled,
        has_start = start_date.is_some(),
        has_end = end_date.is_some(),
        external_only,
        "classic vacation responder saved"
    );

    let msg = if enabled {
        "Vacation responder saved and enabled."
    } else {
        "Vacation responder saved."
    };
    Ok(redirect_with_flash(FlashKind::Saved, msg))
}

// ---------- Helpers ----------

fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims.sub.parse().map_err(|_| {
        AppError::Internal(anyhow::anyhow!(
            "classic settings/vacation: invalid mailbox id in claims"
        ))
    })
}

fn render_html<T: Template>(status: StatusCode, template: &T) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!("classic vacation template render: {e}"))
    })?;
    Ok((
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

/// 303-redirect carrying the flash banner in the URL. POST-Redirect-Get
/// keeps a reload from re-submitting the form.
fn redirect_with_flash(kind: FlashKind, msg: &str) -> Response {
    let target = format!(
        "{}?flash={}&msg={}",
        VACATION_PATH,
        kind.as_str(),
        urlencoding::encode(msg),
    );
    (StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response()
}

/// Decode the `?flash=&msg=` query into the template's `(kind, msg)`
/// tuple. Unknown kinds become None so a hostile bookmark can't inject
/// arbitrary banner copy without a visible class.
fn build_flash(query: &VacationQuery) -> Option<(String, String)> {
    let kind = query.flash.as_deref()?;
    let msg = query.msg.clone().unwrap_or_default();
    if msg.is_empty() {
        return None;
    }
    let safe_msg = msg.chars().take(512).collect::<String>();
    match kind {
        "saved" => Some(("success".to_string(), safe_msg)),
        "error" => Some(("error".to_string(), safe_msg)),
        _ => None,
    }
}

/// Format a stored `DateTime<Utc>` back into the YYYY-MM-DD string an
/// `<input type="date">` will accept. The picker doesn't display tz, so
/// the user sees the same date they typed in (modulo the UTC convention
/// the handler applies on POST).
fn format_date_for_input(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

/// Parse a YYYY-MM-DD string from `<input type="date">` into an optional
/// `DateTime<Utc>`. Empty → Ok(None). Invalid → Err with a user-facing
/// message. `end_of_day=true` snaps the time to 23:59:59 UTC so the full
/// day is included in the rule's active window.
fn parse_optional_date(raw: &str, end_of_day: bool) -> Result<Option<DateTime<Utc>>, String> {
    if raw.is_empty() {
        return Ok(None);
    }
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| "Use the date picker (YYYY-MM-DD).".to_string())?;
    if date.year() < DATE_LOWER_BOUND_YEAR || date.year() > DATE_UPPER_BOUND_YEAR {
        return Err(format!(
            "Year must be between {DATE_LOWER_BOUND_YEAR} and {DATE_UPPER_BOUND_YEAR}."
        ));
    }
    let (h, m, s) = if end_of_day { (23, 59, 59) } else { (0, 0, 0) };
    let naive = date
        .and_hms_opt(h, m, s)
        .ok_or_else(|| "Invalid time of day.".to_string())?;
    Ok(Some(Utc.from_utc_datetime(&naive)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_template() -> VacationFormTemplate {
        VacationFormTemplate {
            enabled: false,
            subject: "Out of Office".to_string(),
            body: String::new(),
            start_date: String::new(),
            end_date: String::new(),
            external_only: true,
            flash: None,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
            quota_indicator: None,
        }
    }

    // ----- Constants -----

    #[test]
    fn vacation_path_locked() {
        // Lock the route so a rename has to touch the test (and therefore
        // every cross-link in other settings pages).
        assert_eq!(VACATION_PATH, "/classic/settings/vacation");
    }

    #[test]
    fn max_subject_len_bounded() {
        assert_eq!(MAX_SUBJECT_LEN, 256);
    }

    #[test]
    fn max_body_len_matches_signature_cap() {
        // Same cap as classic signature so a beta customer's mental
        // model ("how much can I type?") is consistent.
        assert_eq!(MAX_BODY_LEN, 8 * 1024);
    }

    #[test]
    fn date_bounds_sane() {
        assert!(DATE_LOWER_BOUND_YEAR < DATE_UPPER_BOUND_YEAR);
        assert!(DATE_LOWER_BOUND_YEAR >= 2000);
        assert!(DATE_UPPER_BOUND_YEAR <= 2100);
    }

    // ----- Template -----

    #[test]
    fn template_renders_form_action_and_method() {
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains(&format!("action=\"{}\"", VACATION_PATH)),
            "form action missing: {body}"
        );
        assert!(body.contains("method=\"post\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("value=\"test-csrf-token\""));
    }

    #[test]
    fn template_renders_all_field_names() {
        let body = fresh_template().render().expect("renders");
        for name in [
            "name=\"enabled\"",
            "name=\"subject\"",
            "name=\"body\"",
            "name=\"start_date\"",
            "name=\"end_date\"",
            "name=\"external_only\"",
        ] {
            assert!(body.contains(name), "missing field {name}: {body}");
        }
    }

    #[test]
    fn template_date_inputs_are_type_date() {
        // The spec calls for date pickers, not datetime-local. type="date"
        // gives the cross-browser native calendar UI and parses cleanly
        // to NaiveDate on the server.
        let body = fresh_template().render().expect("renders");
        assert!(
            body.matches("type=\"date\"").count() >= 2,
            "expected two type=\"date\" inputs (start + end): {body}"
        );
    }

    #[test]
    fn template_enabled_checkbox_reflects_state() {
        let mut t = fresh_template();
        t.enabled = true;
        let on = t.render().expect("renders");
        // The "checked" attribute appears on whichever checkbox is on.
        // We just need at least one (the enabled toggle).
        assert!(on.contains("checked"), "enabled checkbox missing checked: {on}");
    }

    #[test]
    fn template_external_only_default_on() {
        // Brand-new responder defaults external_only=true; this is the
        // sensible behaviour because it prevents the auto-reply from
        // amplifying mailing list traffic.
        let body = fresh_template().render().expect("renders");
        // Locate the external_only input and verify it carries `checked`.
        let ext_at = body
            .find("name=\"external_only\"")
            .expect("external_only input present");
        // Look in a small window around the input tag for "checked".
        let window_start = body[..ext_at].rfind("<input").unwrap_or(0);
        let window_end = body[ext_at..]
            .find('>')
            .map(|rel| ext_at + rel)
            .unwrap_or(body.len());
        let window = &body[window_start..window_end];
        assert!(
            window.contains("checked"),
            "external_only should default to checked: {window}"
        );
    }

    #[test]
    fn template_echoes_subject_and_body() {
        let mut t = fresh_template();
        t.subject = "Back on Monday".to_string();
        // No apostrophes — Askama HTML-escapes them to &#39;. The
        // round-trip on the browser side is fine (the textarea decodes
        // the entity back to `'`), but the in-memory string we assert
        // against here needs to match the rendered (escaped) bytes.
        t.body = "Back soon.\nKwame".to_string();
        let body = t.render().expect("renders");
        assert!(body.contains("Back on Monday"), "subject not echoed: {body}");
        assert!(
            body.contains("Back soon.\nKwame"),
            "body not echoed: {body}"
        );
    }

    #[test]
    fn template_echoes_dates() {
        let mut t = fresh_template();
        t.start_date = "2026-06-01".to_string();
        t.end_date = "2026-06-07".to_string();
        let body = t.render().expect("renders");
        assert!(body.contains("2026-06-01"));
        assert!(body.contains("2026-06-07"));
    }

    #[test]
    fn template_renders_success_flash() {
        let mut t = fresh_template();
        t.flash = Some(("success".to_string(), "Vacation responder saved.".to_string()));
        let body = t.render().expect("renders");
        assert!(body.contains("alert-success"));
        assert!(body.contains("role=\"status\""));
        assert!(body.contains("Vacation responder saved."));
    }

    #[test]
    fn template_renders_error_flash() {
        let mut t = fresh_template();
        t.flash = Some(("error".to_string(), "End date must be on or after the start date.".to_string()));
        let body = t.render().expect("renders");
        assert!(body.contains("alert-error"));
        assert!(body.contains("role=\"alert\""));
        assert!(body.contains("End date must be on or after the start date."));
    }

    #[test]
    fn template_renders_logout_partial_inside_nav() {
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains("action=\"/classic/logout\""),
            "logout partial must render: {body}"
        );
    }

    #[test]
    fn template_has_zero_script_tags() {
        // Hard rule across the whole Classic UI surface.
        let body = fresh_template().render().expect("renders");
        assert!(!body.contains("<script"));
    }

    #[test]
    fn template_html_escapes_hostile_csrf_token() {
        let mut t = fresh_template();
        t.csrf_token = "\"><script>alert(1)</script>".to_string();
        let body = t.render().expect("renders");
        assert!(
            !body.contains("\"><script>alert(1)</script>"),
            "raw <script> leaked into value attribute: {body}"
        );
    }

    #[test]
    fn template_html_escapes_hostile_subject() {
        let mut t = fresh_template();
        t.subject = "<script>alert(1)</script>".to_string();
        let body = t.render().expect("renders");
        assert!(
            !body.contains("<script>alert(1)</script>"),
            "raw <script> leaked from subject: {body}"
        );
    }

    // ----- FlashKind -----

    #[test]
    fn flash_kind_serialises_to_url_safe_strings() {
        assert_eq!(FlashKind::Saved.as_str(), "saved");
        assert_eq!(FlashKind::Error.as_str(), "error");
    }

    // ----- build_flash -----

    #[test]
    fn build_flash_maps_saved_to_success() {
        let q = VacationQuery {
            flash: Some("saved".to_string()),
            msg: Some("ok".to_string()),
        };
        assert_eq!(
            build_flash(&q),
            Some(("success".to_string(), "ok".to_string()))
        );
    }

    #[test]
    fn build_flash_maps_error_to_error() {
        let q = VacationQuery {
            flash: Some("error".to_string()),
            msg: Some("nope".to_string()),
        };
        let result = build_flash(&q);
        assert_eq!(result.as_ref().map(|(k, _)| k.as_str()), Some("error"));
    }

    #[test]
    fn build_flash_returns_none_without_flash_param() {
        assert_eq!(build_flash(&VacationQuery::default()), None);
    }

    #[test]
    fn build_flash_returns_none_for_unknown_kind() {
        // Hostile `?flash=danger&msg=…` URL must NOT render with arbitrary
        // banner class.
        let q = VacationQuery {
            flash: Some("danger".to_string()),
            msg: Some("Boom".to_string()),
        };
        assert_eq!(build_flash(&q), None);
    }

    #[test]
    fn build_flash_truncates_oversized_msg() {
        let q = VacationQuery {
            flash: Some("saved".to_string()),
            msg: Some("a".repeat(2048)),
        };
        let (_, msg) = build_flash(&q).expect("flash present");
        assert!(msg.len() <= 512, "msg should be capped: {}", msg.len());
    }

    // ----- redirect_with_flash -----

    #[test]
    fn redirect_carries_flash_query() {
        let resp = redirect_with_flash(FlashKind::Saved, "Vacation responder saved.");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp.headers().get(header::LOCATION).expect("Location");
        let location = location.to_str().unwrap();
        assert!(location.starts_with(VACATION_PATH), "location: {location}");
        assert!(location.contains("flash=saved"));
    }

    #[test]
    fn redirect_url_encodes_msg_with_special_chars() {
        let resp = redirect_with_flash(FlashKind::Error, "Bad <input> & stuff");
        let location = resp.headers().get(header::LOCATION).unwrap().to_str().unwrap();
        assert!(
            !location.contains('<') && !location.contains('>'),
            "raw <> leaked: {location}"
        );
        assert!(location.contains("%3C") || location.contains("%3c"));
    }

    // ----- format_date_for_input -----

    #[test]
    fn format_date_for_input_strips_time() {
        let dt = Utc.with_ymd_and_hms(2026, 6, 7, 23, 59, 59).unwrap();
        assert_eq!(format_date_for_input(dt), "2026-06-07");
    }

    #[test]
    fn format_date_for_input_pads_month_and_day() {
        let dt = Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0).unwrap();
        assert_eq!(format_date_for_input(dt), "2026-01-03");
    }

    // ----- parse_optional_date -----

    #[test]
    fn parse_optional_date_empty_is_none() {
        assert_eq!(parse_optional_date("", false), Ok(None));
        assert_eq!(parse_optional_date("", true), Ok(None));
    }

    #[test]
    fn parse_optional_date_start_is_midnight_utc() {
        let got = parse_optional_date("2026-06-01", /*end_of_day=*/ false).unwrap().unwrap();
        let want = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn parse_optional_date_end_is_last_second_utc() {
        let got = parse_optional_date("2026-06-07", /*end_of_day=*/ true).unwrap().unwrap();
        let want = Utc.with_ymd_and_hms(2026, 6, 7, 23, 59, 59).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn parse_optional_date_rejects_garbage() {
        assert!(parse_optional_date("not-a-date", false).is_err());
        assert!(parse_optional_date("06/01/2026", false).is_err());
        assert!(parse_optional_date("2026-13-01", false).is_err());
    }

    #[test]
    fn parse_optional_date_rejects_year_below_lower_bound() {
        let err = parse_optional_date("1999-06-01", false).unwrap_err();
        assert!(err.contains("Year"), "err: {err}");
    }

    #[test]
    fn parse_optional_date_rejects_year_above_upper_bound() {
        let err = parse_optional_date("2099-06-01", false).unwrap_err();
        assert!(err.contains("Year"), "err: {err}");
    }

    // ----- parse_mailbox_id -----

    #[test]
    fn parse_mailbox_id_accepts_valid_uuid() {
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            username: "u".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_ok());
    }

    #[test]
    fn parse_mailbox_id_rejects_garbage() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "u".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_err());
    }
}
