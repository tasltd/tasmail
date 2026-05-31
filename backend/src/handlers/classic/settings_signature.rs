// Added (TMAIL-378): GET + POST /classic/settings/signature — single-signature
// settings page for the no-JS Classic UI surface (gap-analysis P1 #24).
//
// Surface
// -------
//   * GET  /classic/settings/signature
//       Renders the form with the user's current default signature (if any)
//       prefilled into the textarea, a "Set as default" checkbox (default
//       on), and a sanitised HTML preview block.
//
//   * POST /classic/settings/signature  (form-urlencoded)
//       Dispatches on the `action` form field:
//         action=save   → upsert the user's default signature. Single
//                         signature per user — if a default row exists we
//                         UPDATE it, otherwise we CREATE one named
//                         "Default". `text_body` stores the raw textarea
//                         content (consumed by compose auto-append).
//                         `html_body` stores the sanitiser output.
//         action=remove → DELETE the user's default signature.
//
// Why "single signature, named 'Default'"
// ---------------------------------------
// The gap analysis explicitly scopes Classic UI to "Single signature form
// sufficient for v1" (row 105: `Signature CRUD + default`). The full
// multi-signature CRUD continues to live on the SPA's existing
// `/api/signatures` surface — this page is the no-JS minimum that lets a
// beta customer set + edit one default from a stuck-on-2G phone, which is
// what auto-appends on compose.
//
// HTML allowed, sanitised
// -----------------------
// The textarea content goes through `services::html_sanitizer::sanitize_email_html`
// (the same strict-allowlist sanitiser the message-read view uses for
// untrusted inbound mail). That means:
//   * `<script>`, `<style>`, `<iframe>`, `<object>`, `<embed>`, `<form>`,
//     `<base>`, `<meta>` are stripped (ammonia default allowlist).
//   * Inline event handlers (`onclick=`, `onerror=`, …) are stripped.
//   * `javascript:` / `vbscript:` URLs are stripped.
//   * Remote `<img src=>` URLs are rewritten to a 1×1 blocked-image
//     placeholder (so a hostile draft signature can't act as a tracking
//     pixel on every outbound mail).
//   * Safe text formatting (`<b>`, `<i>`, `<a href>`, `<br>`, `<p>`, …)
//     survives.
// The sanitised output is what goes into `html_body` and into the preview
// block on the next render — never the raw user input.
//
// `text_body` is the raw textarea content (capped at MAX_SIGNATURE_LEN to
// keep an outbound mail's per-recipient overhead bounded). That field is
// what `compose.rs::append_signature` suffixes onto the body before the
// user starts typing.
//
// CSRF + CSP
// ----------
// The route lives on `authenticated_router(state)` in `handlers::classic::mod`,
// so both `classic_session_middleware` (cookie → session + extensions) and
// `classic_csrf_middleware` (validates `_csrf` form field) wrap us
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
use crate::models::signature::{CreateSignature, Signature, UpdateSignature};
use crate::services::auth_service::Claims;
use crate::services::html_sanitizer::sanitize_email_html;
use crate::state::AppState;

use super::CspNonce;

/// Path the form posts to. Single source of truth so a future rename
/// doesn't drift between handler, template, and router.
pub const SIGNATURE_PATH: &str = "/classic/settings/signature";

/// Name used for the auto-created classic-UI signature row. The SPA's
/// signature manager renders this in the list; "Default" makes the
/// classic-UI signature obvious there too.
pub const DEFAULT_SIGNATURE_NAME: &str = "Default";

/// Hard cap on signature length. 8 KiB is comfortably larger than any
/// realistic plain-text or HTML signature (corporate disclaimer + logo
/// link + addresses + 3 phone numbers fits well under 4 KiB) and small
/// enough that a hostile draft can't bloat every outbound mail.
pub const MAX_SIGNATURE_LEN: usize = 8 * 1024;

/// Flash message kinds. Drives the success/error banner colour on the
/// next render after a POST + 303 round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashKind {
    Saved,
    Removed,
    Error,
}

impl FlashKind {
    fn as_str(self) -> &'static str {
        match self {
            FlashKind::Saved => "saved",
            FlashKind::Removed => "removed",
            FlashKind::Error => "error",
        }
    }
}

/// Query-string carrier for the post-POST flash banner. Lives in the URL
/// (not a server-side flash store) so the surface stays stateless and a
/// page reload is idempotent.
#[derive(Debug, Default, Deserialize)]
pub struct SignatureQuery {
    #[serde(default)]
    pub flash: Option<String>,
    #[serde(default)]
    pub msg: Option<String>,
}

// ---------- Template struct ----------

#[derive(Template)]
#[template(path = "classic/settings/signature.html")]
pub struct SignatureFormTemplate {
    /// Raw textarea content. On a fresh render this is the user's saved
    /// `text_body` (or empty); on a validation failure re-render this is
    /// the submitted value so the user doesn't lose their typing.
    pub body: String,
    /// `<input type="checkbox" checked>` when true. Default-on on the
    /// initial render so a brand-new signature is set as default unless
    /// the user explicitly unticks.
    pub is_default: bool,
    /// True when a signature row currently exists for the user — used to
    /// decide whether the "Remove signature" button renders. (Brand-new
    /// users without a saved signature shouldn't see Remove.)
    pub has_existing_signature: bool,
    /// Sanitised HTML preview, rendered with `{{ preview_html|safe }}`.
    /// Built from the SAME sanitised HTML that gets persisted in
    /// `signatures.html_body`, so the user sees exactly what their
    /// recipients will see.
    pub preview_html: String,
    /// Optional banner (success / error) above the form. Some(("kind", "msg")).
    /// `kind` is rendered into the alert class so a CSS rename lands in
    /// one place.
    pub flash: Option<(String, String)>,
    /// Session-scoped CSRF token. Threaded into the hidden `_csrf` field
    /// on both action forms (save + remove) AND the logout partial.
    pub csrf_token: String,
    /// Per-request CSP nonce required by base.html (TMAIL-356).
    pub csp_nonce: String,
    /// Added (TMAIL-384): Footer quota indicator. `None` on cache + DB
    /// outage — the partial renders nothing.
    pub quota_indicator: Option<super::QuotaIndicator>,
}

#[derive(Debug, Deserialize)]
pub struct SignatureForm {
    /// `save` (default) or `remove`. Anything else surfaces as a 400.
    #[serde(default)]
    pub action: Option<String>,
    /// Textarea content. Up to MAX_SIGNATURE_LEN bytes after trim.
    #[serde(default)]
    pub body: Option<String>,
    /// Checkbox encodes as `is_default=on` when ticked, absent when not.
    /// (HTML forms don't post unchecked checkboxes — that's why this is
    /// Option<String> not bool.)
    #[serde(default)]
    pub is_default: Option<String>,
    /// Validated by `classic_csrf_middleware` before this handler runs;
    /// the field is on the struct so Form deserialisation doesn't 400
    /// with "missing field".
    #[serde(rename = "_csrf")]
    #[allow(dead_code)]
    pub csrf: String,
}

// ---------- Handlers ----------

/// GET /classic/settings/signature — render the form prefilled from the
/// user's saved default signature (if any).
pub async fn get_signature(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    axum::extract::Query(query): axum::extract::Query<SignatureQuery>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let existing = Signature::find_default(&state.db, mailbox_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default signature: {e}")))?;

    let (body, preview_html, has_existing, is_default) = match &existing {
        Some(sig) => (
            sig.text_body.clone(),
            sig.html_body.clone(),
            true,
            true,
        ),
        None => (String::new(), String::new(), false, true),
    };

    // Added (TMAIL-384): hydrate the footer quota indicator. Loaded
    // once per GET so the indicator stays in sync with every other
    // authenticated page.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;

    let template = SignatureFormTemplate {
        body,
        is_default,
        has_existing_signature: has_existing,
        preview_html,
        flash: build_flash(&query),
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
        quota_indicator,
    };
    render_html(StatusCode::OK, &template)
}

/// POST /classic/settings/signature — save / remove dispatch.
pub async fn post_signature(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    axum::Form(form): axum::Form<SignatureForm>,
) -> Result<Response, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let action = form.action.as_deref().unwrap_or("save");
    let csrf_token = session.csrf_token.clone();
    let csp_nonce_str = csp_nonce.into_string();

    match action {
        "remove" => handle_remove(&state, mailbox_id).await,
        "save" => {
            handle_save(&state, mailbox_id, &form, &csrf_token, &csp_nonce_str).await
        }
        other => Err(AppError::BadRequest(format!(
            "Unknown action '{other}'. Expected 'save' or 'remove'."
        ))),
    }
}

async fn handle_save(
    state: &AppState,
    mailbox_id: Uuid,
    form: &SignatureForm,
    csrf_token: &str,
    csp_nonce: &str,
) -> Result<Response, AppError> {
    let raw_body = form.body.as_deref().unwrap_or("").trim().to_string();
    let is_default = form.is_default.is_some();

    // Empty body on a save is a validation error — the user should use
    // the Remove button to clear the signature. We re-render the form
    // (NOT 303-redirect) so the error banner sits over the form they
    // just submitted.
    if raw_body.is_empty() {
        return render_form_error(
            "Signature body cannot be empty. Use \"Remove signature\" to clear it.",
            StatusCode::BAD_REQUEST,
            state,
            mailbox_id,
            String::new(),
            is_default,
            csrf_token,
            csp_nonce,
        )
        .await;
    }
    if raw_body.len() > MAX_SIGNATURE_LEN {
        return render_form_error(
            &format!(
                "Signature is too long ({} bytes — max {} bytes).",
                raw_body.len(),
                MAX_SIGNATURE_LEN
            ),
            StatusCode::BAD_REQUEST,
            state,
            mailbox_id,
            raw_body,
            is_default,
            csrf_token,
            csp_nonce,
        )
        .await;
    }

    let html_body = sanitize_email_html(&raw_body);

    // Look up existing default. If present → UPDATE; else → CREATE.
    let existing = Signature::find_default(&state.db, mailbox_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default for upsert: {e}")))?;

    match existing {
        Some(sig) => {
            let update = UpdateSignature {
                name: None,
                html_body: Some(html_body),
                text_body: Some(raw_body),
                is_default: Some(is_default),
            };
            Signature::update(&state.db, sig.id, mailbox_id, &update)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("update signature: {e}")))?
                .ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "signature row vanished between lookup and update"
                    ))
                })?;
        }
        None => {
            let create = CreateSignature {
                name: DEFAULT_SIGNATURE_NAME.to_string(),
                html_body,
                text_body: raw_body,
                is_default: Some(is_default),
            };
            Signature::create(&state.db, mailbox_id, &create)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("create signature: {e}")))?;
        }
    };

    tracing::info!(
        user_id = ?mailbox_id,
        is_default,
        "classic signature saved"
    );

    Ok(redirect_with_flash(FlashKind::Saved, "Signature saved."))
}

async fn handle_remove(state: &AppState, mailbox_id: Uuid) -> Result<Response, AppError> {
    let existing = Signature::find_default(&state.db, mailbox_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default for delete: {e}")))?;

    match existing {
        Some(sig) => {
            let deleted = Signature::delete(&state.db, sig.id, mailbox_id)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("delete signature: {e}")))?;
            if !deleted {
                tracing::warn!(
                    user_id = ?mailbox_id,
                    sig_id = ?sig.id,
                    "delete reported no rows affected — race with another tab?"
                );
            } else {
                tracing::info!(user_id = ?mailbox_id, "classic signature removed");
            }
            Ok(redirect_with_flash(
                FlashKind::Removed,
                "Signature removed. New messages will be sent without a signature.",
            ))
        }
        None => Ok(redirect_with_flash(
            FlashKind::Error,
            "No signature to remove.",
        )),
    }
}

// ---------- Helpers ----------

fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("classic settings/signature: invalid mailbox id in claims")))
}

fn render_html<T: Template>(status: StatusCode, template: &T) -> Result<Response, AppError> {
    let body = template
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic signature template render: {e}")))?;
    Ok((
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

/// Re-render the form with an inline error banner. Recomputes the preview
/// from the (still un-saved) submitted body so what the user sees matches
/// what would have been saved.
#[allow(clippy::too_many_arguments)]
async fn render_form_error(
    error: &str,
    status: StatusCode,
    state: &AppState,
    mailbox_id: Uuid,
    body: String,
    is_default: bool,
    csrf_token: &str,
    csp_nonce: &str,
) -> Result<Response, AppError> {
    // `has_existing_signature` reflects the DB state, NOT the form state —
    // a validation failure shouldn't make the Remove button vanish just
    // because the user emptied the textarea.
    let has_existing = Signature::find_default(&state.db, mailbox_id)
        .await
        .map(|opt| opt.is_some())
        .unwrap_or(false);

    let preview_html = if body.is_empty() {
        String::new()
    } else {
        sanitize_email_html(&body)
    };

    // Added (TMAIL-384): hydrate the footer quota indicator for the
    // error re-render so it carries the same indicator as the GET.
    let quota_indicator = super::load_quota_indicator(state, mailbox_id).await;

    let template = SignatureFormTemplate {
        body,
        is_default,
        has_existing_signature: has_existing,
        preview_html,
        flash: Some(("error".to_string(), error.to_string())),
        csrf_token: csrf_token.to_string(),
        csp_nonce: csp_nonce.to_string(),
        quota_indicator,
    };
    render_html(status, &template)
}

/// Build the 303-redirect response carrying the flash banner in the URL.
/// We use 303 (not 302) so a browser reload of the landing page is a GET
/// — POST-Redirect-Get is the canonical no-JS pattern for form-edit pages.
fn redirect_with_flash(kind: FlashKind, msg: &str) -> Response {
    let target = format!(
        "{}?flash={}&msg={}",
        SIGNATURE_PATH,
        kind.as_str(),
        urlencoding::encode(msg),
    );
    (StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response()
}

/// Decode the `?flash=&msg=` query into the template's `(kind, msg)`
/// tuple. Unknown kinds become None so a hostile bookmark can't inject
/// arbitrary banner copy without a visible class.
fn build_flash(query: &SignatureQuery) -> Option<(String, String)> {
    let kind = query.flash.as_deref()?;
    let msg = query.msg.clone().unwrap_or_default();
    if msg.is_empty() {
        return None;
    }
    // Cap the displayed message so a runaway / hostile `?msg=` query
    // string can't make the banner OOM the renderer.
    let safe_msg = msg.chars().take(512).collect::<String>();
    match kind {
        "saved" => Some(("success".to_string(), safe_msg)),
        "removed" => Some(("success".to_string(), safe_msg)),
        "error" => Some(("error".to_string(), safe_msg)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_template() -> SignatureFormTemplate {
        SignatureFormTemplate {
            body: String::new(),
            is_default: true,
            has_existing_signature: false,
            preview_html: String::new(),
            flash: None,
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
            quota_indicator: None,
        }
    }

    // ----- Constants -----

    #[test]
    fn signature_path_locked() {
        assert_eq!(SIGNATURE_PATH, "/classic/settings/signature");
    }

    #[test]
    fn default_signature_name_is_human_readable() {
        // The SPA signature manager lists rows by name — "Default" makes it
        // obvious which one the classic UI owns.
        assert_eq!(DEFAULT_SIGNATURE_NAME, "Default");
    }

    #[test]
    fn max_signature_len_bounded() {
        // 8 KiB is the documented cap. Larger values mean we lost the
        // intent of the rule; smaller means we'd reject realistic
        // corporate disclaimers.
        assert_eq!(MAX_SIGNATURE_LEN, 8 * 1024);
    }

    // ----- Template -----

    #[test]
    fn template_renders_form_action_and_method() {
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains(&format!("action=\"{}\"", SIGNATURE_PATH)),
            "form action missing: {body}"
        );
        assert!(body.contains("method=\"post\""));
        assert!(body.contains("name=\"body\""));
        assert!(body.contains("name=\"is_default\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("value=\"test-csrf-token\""));
    }

    #[test]
    fn template_save_button_named_action_save() {
        // The dispatch hinges on the submit button's `name=action`
        // `value=save` attribute. Without these, every POST defaults to
        // "save" (which is the right fallback) — but the explicit button
        // value makes the test for the remove branch sharp.
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains("name=\"action\""),
            "save submit button missing name=action: {body}"
        );
        assert!(
            body.contains("value=\"save\""),
            "save submit button missing value=save: {body}"
        );
    }

    #[test]
    fn template_remove_button_renders_only_when_signature_exists() {
        let mut t = fresh_template();
        t.has_existing_signature = false;
        let no_existing = t.render().expect("renders");
        assert!(
            !no_existing.contains("value=\"remove\""),
            "remove button should be hidden when no signature exists: {no_existing}"
        );

        let mut t = fresh_template();
        t.has_existing_signature = true;
        let with_existing = t.render().expect("renders");
        assert!(
            with_existing.contains("value=\"remove\""),
            "remove button must render when a signature exists: {with_existing}"
        );
    }

    #[test]
    fn template_is_default_checkbox_reflects_state() {
        let mut t = fresh_template();
        t.is_default = true;
        let on = t.render().expect("renders");
        assert!(on.contains("checked"), "checkbox missing checked attr: {on}");

        let mut t = fresh_template();
        t.is_default = false;
        let off = t.render().expect("renders");
        // The string "checked" appears only on the checkbox element.
        assert!(
            !off.contains("checked"),
            "checkbox unexpectedly checked when is_default=false: {off}"
        );
    }

    #[test]
    fn template_renders_preview_block_with_safe_html() {
        let mut t = fresh_template();
        t.preview_html = "<p>Best,<br><b>Kwame</b></p>".to_string();
        let body = t.render().expect("renders");
        // `|safe` lets sanitised HTML survive — the preview is the WHOLE
        // point of the page.
        assert!(
            body.contains("<p>Best,<br><b>Kwame</b></p>"),
            "preview block should render sanitised HTML literally: {body}"
        );
    }

    #[test]
    fn template_omits_preview_when_empty() {
        let body = fresh_template().render().expect("renders");
        // The CSS class name `.signature-preview` lives in the inline
        // `<style>` block regardless, so assert against the actual
        // `<section class="signature-preview">` ELEMENT, not the substring.
        assert!(
            !body.contains("<section class=\"signature-preview\""),
            "preview <section> should be hidden on empty body: {body}"
        );
        // Also assert that the preview HEADING isn't emitted — that's
        // proof the `{% if !preview_html.is_empty() %}` branch was skipped.
        assert!(
            !body.contains("id=\"signature-preview-heading\""),
            "preview heading should be hidden on empty body: {body}"
        );
    }

    #[test]
    fn template_echoes_textarea_body() {
        let mut t = fresh_template();
        t.body = "Best regards,\nKwame".to_string();
        let body = t.render().expect("renders");
        // textarea content lives between the open + close tags, NOT in a
        // value=… attribute.
        assert!(
            body.contains("Best regards,\nKwame"),
            "textarea body not echoed: {body}"
        );
    }

    #[test]
    fn template_renders_success_flash() {
        let mut t = fresh_template();
        t.flash = Some(("success".to_string(), "Signature saved.".to_string()));
        let body = t.render().expect("renders");
        assert!(body.contains("alert-success"));
        assert!(body.contains("role=\"status\""));
        assert!(body.contains("Signature saved."));
    }

    #[test]
    fn template_renders_error_flash() {
        let mut t = fresh_template();
        t.flash = Some(("error".to_string(), "Too long.".to_string()));
        let body = t.render().expect("renders");
        assert!(body.contains("alert-error"));
        assert!(body.contains("role=\"alert\""));
        assert!(body.contains("Too long."));
    }

    #[test]
    fn template_omits_flash_when_none() {
        let body = fresh_template().render().expect("renders");
        // The CSS classes appear in the inline `<style>` even when no
        // banner is emitted; assert against the actual ELEMENT markup.
        assert!(
            !body.contains("class=\"alert alert-success\""),
            "no success banner should render when flash=None: {body}"
        );
        assert!(
            !body.contains("class=\"alert alert-error\""),
            "no error banner should render when flash=None: {body}"
        );
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

    // ----- FlashKind -----

    #[test]
    fn flash_kind_serialises_to_url_safe_strings() {
        assert_eq!(FlashKind::Saved.as_str(), "saved");
        assert_eq!(FlashKind::Removed.as_str(), "removed");
        assert_eq!(FlashKind::Error.as_str(), "error");
    }

    // ----- build_flash -----

    #[test]
    fn build_flash_maps_saved_to_success() {
        let q = SignatureQuery {
            flash: Some("saved".to_string()),
            msg: Some("Signature saved.".to_string()),
        };
        assert_eq!(
            build_flash(&q),
            Some(("success".to_string(), "Signature saved.".to_string()))
        );
    }

    #[test]
    fn build_flash_maps_removed_to_success() {
        let q = SignatureQuery {
            flash: Some("removed".to_string()),
            msg: Some("Signature removed.".to_string()),
        };
        let result = build_flash(&q);
        assert_eq!(result.as_ref().map(|(k, _)| k.as_str()), Some("success"));
    }

    #[test]
    fn build_flash_maps_error_to_error() {
        let q = SignatureQuery {
            flash: Some("error".to_string()),
            msg: Some("Too long.".to_string()),
        };
        let result = build_flash(&q);
        assert_eq!(result.as_ref().map(|(k, _)| k.as_str()), Some("error"));
    }

    #[test]
    fn build_flash_returns_none_without_flash_param() {
        let q = SignatureQuery::default();
        assert_eq!(build_flash(&q), None);
    }

    #[test]
    fn build_flash_returns_none_without_msg() {
        let q = SignatureQuery {
            flash: Some("saved".to_string()),
            msg: None,
        };
        assert_eq!(build_flash(&q), None);
    }

    #[test]
    fn build_flash_returns_none_for_unknown_kind() {
        // Defence-in-depth — a hostile `?flash=danger&msg=…` URL must NOT
        // render with arbitrary banner class.
        let q = SignatureQuery {
            flash: Some("danger".to_string()),
            msg: Some("Boom".to_string()),
        };
        assert_eq!(build_flash(&q), None);
    }

    #[test]
    fn build_flash_truncates_oversized_msg() {
        let q = SignatureQuery {
            flash: Some("saved".to_string()),
            msg: Some("a".repeat(2048)),
        };
        let (_, msg) = build_flash(&q).expect("flash present");
        assert!(
            msg.len() <= 512,
            "msg should be capped at 512 chars, got {}",
            msg.len()
        );
    }

    // ----- redirect_with_flash -----

    #[test]
    fn redirect_carries_flash_query() {
        let resp = redirect_with_flash(FlashKind::Saved, "Signature saved.");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp.headers().get(header::LOCATION).expect("Location header");
        let location = location.to_str().unwrap();
        assert!(location.starts_with(SIGNATURE_PATH), "location: {location}");
        assert!(location.contains("flash=saved"), "location: {location}");
        // URL encoding turns the space into %20 and the period stays.
        assert!(
            location.contains("Signature%20saved") || location.contains("Signature+saved"),
            "msg should be URL-encoded into Location: {location}"
        );
    }

    #[test]
    fn redirect_url_encodes_msg_with_special_chars() {
        let resp = redirect_with_flash(FlashKind::Error, "Too long & messy <foo>");
        let location = resp.headers().get(header::LOCATION).expect("Location header");
        let location = location.to_str().unwrap();
        // `<` and `>` MUST be percent-encoded — they would otherwise break
        // out of an HTML attribute on the next render.
        assert!(
            !location.contains('<') && !location.contains('>'),
            "raw < or > leaked into Location: {location}"
        );
        assert!(location.contains("%3C") || location.contains("%3c"));
        assert!(location.contains("%3E") || location.contains("%3e"));
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
