// Added (TMAIL-380): GET + POST handlers for /classic/settings/byok and
// POST handler for /classic/settings/byok/test — the BYOK IMAP / SMTP
// edit form for the no-JS Classic UI surface (gap-analysis P1 #26).
//
// Surface
// -------
//   * GET  /classic/settings/byok
//       Renders the form prefilled from the user's default
//       `imap_configurations` + `smtp_configurations` rows (if any).
//       Passwords NEVER round-trip back to the rendered form — every
//       re-render leaves the password fields blank. The form carries a
//       provider-preset picker (re-used from the signup wizard) so a user
//       switching providers can auto-fill host/port/encryption with one
//       click.
//
//   * POST /classic/settings/byok/test     (form-urlencoded)
//       Runs IMAP TCP+LOGIN + SMTP transport build against the submitted
//       values, re-renders the form with the test result banners
//       (success / failure per-protocol). On a successful test, the
//       Save button gets unlocked: a hidden `tested_ok` field flips to
//       `1` so the next POST /classic/settings/byok with action=save
//       knows it can write through. The submitted values are preserved
//       in the form so the user doesn't lose typing.
//
//   * POST /classic/settings/byok          (form-urlencoded)
//       Action dispatch on the `action` form field:
//         * action=save — write through ONLY if `tested_ok=1` is also
//           present (i.e. the user pressed Test first and both probes
//           passed). Otherwise re-render with an error banner.
//         * action=save_no_test — write through immediately, skipping
//           the connection test. This is the "save without testing"
//           power-user link called out in the gap analysis.
//
// Behaviour on an unchanged password
// ----------------------------------
// `<input type="password">` with no value submitted means "the user
// didn't retype the password — keep the encrypted_password row column as
// it is". This is the standard pattern for edit forms on credentialed
// rows. For CREATE (the user has no IMAP / SMTP row yet), the password
// is required and an empty field produces a validation error.
//
// Why a SEPARATE /test endpoint (rather than action=test on the main
// POST)
// ------------------------------------------------------------------
// The gap analysis spec literally calls for `POST /classic/settings/byok/test`
// as the verify endpoint. Splitting the path keeps the two endpoints'
// router-level audit log + future rate-limiting policies independently
// expressible (test endpoints are far more abuse-prone than save).
//
// CSRF + CSP
// ----------
// Both routes live on `authenticated_router(state)` in
// `handlers::classic::mod`, so both `classic_session_middleware` and
// `classic_csrf_middleware` wrap them transparently. No per-handler CSRF
// code lives here.

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
use crate::models::ai_config::{derive_encryption_key, encrypt_api_key};
use crate::models::classic_session::ClassicSession;
use crate::models::imap_config::{
    provider_presets, CreateImapConfigRequest, ImapConfiguration, ImapEncryption,
};
use crate::models::smtp_config::{SmtpConfiguration, SmtpEncryption};
use crate::services::auth_service::Claims;
use crate::state::AppState;

use super::CspNonce;

/// Path the form posts to. Single source of truth so a future rename
/// doesn't drift between handler, template, and router.
pub const BYOK_PATH: &str = "/classic/settings/byok";

/// Test-only endpoint path. The "Test" button submits the form here,
/// the handler runs probes against IMAP + SMTP, and renders the form
/// back with inline pass/fail banners.
pub const BYOK_TEST_PATH: &str = "/classic/settings/byok/test";

/// Default name to give a freshly-created config row when the user is
/// landing on the BYOK page without anything saved yet. Picks the host
/// out of the form so a user who saves Gmail then later switches to
/// Outlook sees a sensible label on the SPA settings page too.
const DEFAULT_CONFIG_LABEL: &str = "Default";

/// Maximum length we accept for any host / username field on the form.
/// Captures both DNS host limits (253 octets) and email address local-
/// part limits (64 octets local + @ + 253 octets domain ≈ 320 total)
/// with comfortable headroom for non-email usernames.
const MAX_FIELD_LEN: usize = 512;

/// Hard cap on the password we accept on the form. Generous (covers
/// 100-char generated app passwords + headroom) without giving a hostile
/// caller a free Argon2id-grade DoS surface.
const MAX_PASSWORD_LEN: usize = 1024;

// ---------- Template struct ----------

#[derive(Template)]
#[template(path = "classic/settings/byok.html")]
pub struct ByokFormTemplate {
    /// Whether the IMAP / SMTP rows already exist. Drives copy ("Update
    /// your servers" vs "Connect your servers") and whether a blank
    /// password is interpreted as "keep existing" (true) vs "missing
    /// field" (false).
    pub imap_existing: bool,
    pub smtp_existing: bool,

    /// IMAP form fields. Always echoed back so a re-render preserves
    /// typing across validation failures or test results.
    pub imap_host: String,
    pub imap_port: String,
    pub imap_username: String,
    pub imap_encryption: String,

    /// SMTP form fields. Same pattern as IMAP above.
    pub smtp_host: String,
    pub smtp_port: String,
    pub smtp_username: String,
    pub smtp_encryption: String,
    pub smtp_from_address: String,

    /// Provider preset picker. Mirrors `signup_servers.html` so users
    /// switching providers can auto-fill in one click.
    pub presets: Vec<PresetView>,
    pub picked_name: String,
    pub picked_hint: String,

    /// Test-result banners. Some(true, "…") on a pass, Some(false, "…")
    /// on a fail, None when the user hasn't pressed Test yet.
    pub imap_test: Option<(bool, String)>,
    pub smtp_test: Option<(bool, String)>,

    /// Top-of-form non-field error (validation, "you must test first",
    /// internal). Some("…") or None.
    pub error: Option<String>,
    /// Top-of-form success banner. Some("…") immediately after a save.
    pub success: Option<String>,

    /// Hidden form field — flipped to "1" by the test endpoint when
    /// BOTH probes pass. The save handler refuses `action=save` unless
    /// this is "1". The "save without testing" link bypasses this gate
    /// via `action=save_no_test`.
    pub tested_ok: String,

    /// Session-scoped CSRF token. Threaded into the hidden `_csrf` field
    /// on the form AND the logout partial.
    pub csrf_token: String,
    /// Per-request CSP nonce required by base.html.
    pub csp_nonce: String,
}

/// Trimmed view of `provider_presets()` for the template. The raw
/// `serde_json::Value` blob the model returns isn't directly Askama-
/// renderable — we lower it once at GET time into struct fields the
/// template can index without `.get(...)` gymnastics.
#[derive(Debug, Clone)]
pub struct PresetView {
    pub name: String,
    pub imap_host: String,
    pub imap_port: i64,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: i64,
    pub smtp_encryption: String,
    pub hint: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ByokForm {
    #[serde(default)]
    pub imap_host: Option<String>,
    #[serde(default)]
    pub imap_port: Option<String>,
    #[serde(default)]
    pub imap_username: Option<String>,
    #[serde(default)]
    pub imap_password: Option<String>,
    #[serde(default)]
    pub imap_encryption: Option<String>,

    #[serde(default)]
    pub smtp_host: Option<String>,
    #[serde(default)]
    pub smtp_port: Option<String>,
    #[serde(default)]
    pub smtp_username: Option<String>,
    #[serde(default)]
    pub smtp_password: Option<String>,
    #[serde(default)]
    pub smtp_encryption: Option<String>,
    #[serde(default)]
    pub smtp_from_address: Option<String>,

    /// `save` / `save_no_test` on the main POST endpoint. Unused (and
    /// ignored) on the /test endpoint.
    #[serde(default)]
    pub action: Option<String>,

    /// Flipped to `"1"` by the test endpoint when both probes pass. The
    /// save endpoint validates this against `action=save`.
    #[serde(default)]
    pub tested_ok: Option<String>,

    /// Provider preset name carried across re-renders so the highlight
    /// on the picker survives a test/save round-trip.
    #[serde(default)]
    pub picked_name: Option<String>,

    /// Validated by `classic_csrf_middleware` before this handler runs,
    /// but axum's `Form` extractor still needs the field on the struct.
    #[serde(rename = "_csrf")]
    #[allow(dead_code)]
    pub csrf: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ByokQuery {
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub flash: Option<String>,
}

// ---------- Handlers ----------

/// GET /classic/settings/byok — render the form prefilled from the
/// user's default IMAP + SMTP rows (if any).
pub async fn get_byok(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    axum::extract::Query(query): axum::extract::Query<ByokQuery>,
) -> Result<Response, AppError> {
    let user_id = parse_user_id(&claims)?;

    let imap = ImapConfiguration::default_for_user(&state.db, user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default IMAP: {e}")))?;
    let smtp = SmtpConfiguration::find_default(&state.db, user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default SMTP: {e}")))?;

    let presets = build_preset_views();
    let (picked_name, picked_hint) = resolve_preset(&presets, query.preset.as_deref(), &imap);

    // Apply preset overrides when the user picked one via the query
    // string. This lets the link list at the top of the page act as a
    // one-click auto-fill without needing JS.
    let preset = presets.iter().find(|p| p.name == picked_name).cloned();

    let template = ByokFormTemplate {
        imap_existing: imap.is_some(),
        smtp_existing: smtp.is_some(),

        imap_host: preset
            .as_ref()
            .map(|p| p.imap_host.clone())
            .unwrap_or_else(|| imap.as_ref().map(|c| c.host.clone()).unwrap_or_default()),
        imap_port: preset
            .as_ref()
            .map(|p| p.imap_port.to_string())
            .unwrap_or_else(|| imap.as_ref().map(|c| c.port.to_string()).unwrap_or_else(|| "993".to_string())),
        imap_username: imap.as_ref().map(|c| c.username.clone()).unwrap_or_default(),
        imap_encryption: preset
            .as_ref()
            .map(|p| p.imap_encryption.clone())
            .unwrap_or_else(|| imap.as_ref().map(|c| c.encryption.clone()).unwrap_or_else(|| "ssl".to_string())),

        smtp_host: preset
            .as_ref()
            .map(|p| p.smtp_host.clone())
            .unwrap_or_else(|| smtp.as_ref().map(|c| c.host.clone()).unwrap_or_default()),
        smtp_port: preset
            .as_ref()
            .map(|p| p.smtp_port.to_string())
            .unwrap_or_else(|| smtp.as_ref().map(|c| c.port.to_string()).unwrap_or_else(|| "587".to_string())),
        smtp_username: smtp.as_ref().map(|c| c.username.clone()).unwrap_or_default(),
        smtp_encryption: preset
            .as_ref()
            .map(|p| p.smtp_encryption.clone())
            .unwrap_or_else(|| smtp.as_ref().map(|c| c.encryption.clone()).unwrap_or_else(|| "starttls".to_string())),
        smtp_from_address: smtp.as_ref().and_then(|c| c.from_address.clone()).unwrap_or_default(),

        presets,
        picked_name,
        picked_hint,

        imap_test: None,
        smtp_test: None,
        error: None,
        success: build_success_flash(&query),
        tested_ok: String::new(),

        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
    };

    render_html(StatusCode::OK, &template)
}

/// POST /classic/settings/byok/test — run the IMAP TCP+LOGIN + SMTP
/// transport-build probes and re-render the form with the results.
pub async fn post_byok_test(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    axum::Form(form): axum::Form<ByokForm>,
) -> Result<Response, AppError> {
    let user_id = parse_user_id(&claims)?;
    let csrf_token = session.csrf_token.clone();
    let csp_nonce_str = csp_nonce.into_string();

    let imap_existing = ImapConfiguration::default_for_user(&state.db, user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default IMAP: {e}")))?;
    let smtp_existing = SmtpConfiguration::find_default(&state.db, user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default SMTP: {e}")))?;

    let inputs = match validate_inputs(&form, imap_existing.is_some(), smtp_existing.is_some()) {
        Ok(v) => v,
        Err(msg) => {
            return re_render_form(
                &state,
                &form,
                imap_existing.is_some(),
                smtp_existing.is_some(),
                None,
                None,
                Some(msg),
                None,
                /*tested_ok=*/ false,
                &csrf_token,
                &csp_nonce_str,
                user_id,
                StatusCode::BAD_REQUEST,
            )
            .await;
        }
    };

    // Resolve the password we'll probe with — submitted plaintext if
    // present, otherwise decrypt the saved row. If neither is available
    // (brand-new user + blank field), the test fails with a clear
    // error.
    let key = derive_encryption_key(&state.config.jwt.secret);
    let imap_password = match resolve_password_for_probe(
        inputs.imap_password.as_deref(),
        imap_existing.as_ref().map(|c| c.encrypted_password.as_str()),
        &key,
    ) {
        Ok(pw) => pw,
        Err(msg) => {
            return re_render_form(
                &state,
                &form,
                imap_existing.is_some(),
                smtp_existing.is_some(),
                Some((false, format!("IMAP password: {msg}"))),
                None,
                None,
                None,
                /*tested_ok=*/ false,
                &csrf_token,
                &csp_nonce_str,
                user_id,
                StatusCode::OK,
            )
            .await;
        }
    };
    let smtp_password = match resolve_password_for_probe(
        inputs.smtp_password.as_deref(),
        smtp_existing.as_ref().map(|c| c.encrypted_password.as_str()),
        &key,
    ) {
        Ok(pw) => pw,
        Err(msg) => {
            return re_render_form(
                &state,
                &form,
                imap_existing.is_some(),
                smtp_existing.is_some(),
                None,
                Some((false, format!("SMTP password: {msg}"))),
                None,
                None,
                /*tested_ok=*/ false,
                &csrf_token,
                &csp_nonce_str,
                user_id,
                StatusCode::OK,
            )
            .await;
        }
    };

    let imap_result = probe_imap(
        &inputs.imap_host,
        inputs.imap_port,
        &inputs.imap_username,
        &imap_password,
        &inputs.imap_encryption,
    )
    .await;
    let smtp_result = probe_smtp(
        &inputs.smtp_host,
        inputs.smtp_port,
        &inputs.smtp_username,
        &smtp_password,
        &inputs.smtp_encryption,
    )
    .await;

    let tested_ok = imap_result.0 && smtp_result.0;

    re_render_form(
        &state,
        &form,
        imap_existing.is_some(),
        smtp_existing.is_some(),
        Some(imap_result),
        Some(smtp_result),
        None,
        None,
        tested_ok,
        &csrf_token,
        &csp_nonce_str,
        user_id,
        StatusCode::OK,
    )
    .await
}

/// POST /classic/settings/byok — save (with or without prior test).
pub async fn post_byok(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    axum::Form(form): axum::Form<ByokForm>,
) -> Result<Response, AppError> {
    let user_id = parse_user_id(&claims)?;
    let csrf_token = session.csrf_token.clone();
    let csp_nonce_str = csp_nonce.into_string();

    let action = form.action.as_deref().unwrap_or("").to_string();
    let force_save = action == "save_no_test";
    let want_save = action == "save" || force_save;
    if !want_save {
        return re_render_form(
            &state,
            &form,
            false,
            false,
            None,
            None,
            Some("Unknown action — use the Test or Save buttons on the form.".to_string()),
            None,
            /*tested_ok=*/ false,
            &csrf_token,
            &csp_nonce_str,
            user_id,
            StatusCode::BAD_REQUEST,
        )
        .await;
    }

    let imap_existing = ImapConfiguration::default_for_user(&state.db, user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default IMAP: {e}")))?;
    let smtp_existing = SmtpConfiguration::find_default(&state.db, user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("load default SMTP: {e}")))?;

    // Refuse `action=save` unless the user pressed Test first and both
    // probes passed. The hidden `tested_ok=1` field is the proof.
    if !force_save && form.tested_ok.as_deref() != Some("1") {
        return re_render_form(
            &state,
            &form,
            imap_existing.is_some(),
            smtp_existing.is_some(),
            None,
            None,
            Some(
                "Test the connection before saving, or use the \"Save without testing\" link if you're sure."
                    .to_string(),
            ),
            None,
            /*tested_ok=*/ false,
            &csrf_token,
            &csp_nonce_str,
            user_id,
            StatusCode::BAD_REQUEST,
        )
        .await;
    }

    let inputs = match validate_inputs(&form, imap_existing.is_some(), smtp_existing.is_some()) {
        Ok(v) => v,
        Err(msg) => {
            return re_render_form(
                &state,
                &form,
                imap_existing.is_some(),
                smtp_existing.is_some(),
                None,
                None,
                Some(msg),
                None,
                /*tested_ok=*/ false,
                &csrf_token,
                &csp_nonce_str,
                user_id,
                StatusCode::BAD_REQUEST,
            )
            .await;
        }
    };

    let key = derive_encryption_key(&state.config.jwt.secret);

    // Upsert IMAP first, then SMTP. Each call is INDEPENDENT — a partial
    // save (IMAP succeeded, SMTP failed) is preferable to throwing the
    // user back at a blank form on a transient DB hiccup.
    let imap_encrypted = match inputs.imap_password.as_deref() {
        Some(pw) => Some(
            encrypt_api_key(pw, &key)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("encrypt IMAP password: {e}")))?,
        ),
        None => None,
    };

    match imap_existing.as_ref() {
        Some(row) => {
            ImapConfiguration::update(
                &state.db,
                row.id,
                user_id,
                Some(&inputs.imap_host),
                Some(inputs.imap_port),
                Some(&inputs.imap_username),
                imap_encrypted.as_deref(),
                Some(&inputs.imap_encryption),
            )
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("update IMAP: {e}")))?;
        }
        None => {
            // CREATE path — password must be present (validate_inputs
            // already enforced this when imap_existing == false).
            let pw = inputs
                .imap_password
                .clone()
                .expect("validate_inputs enforces IMAP password on create");
            let create_req = CreateImapConfigRequest {
                name: DEFAULT_CONFIG_LABEL.to_string(),
                host: inputs.imap_host.clone(),
                port: inputs.imap_port,
                username: inputs.imap_username.clone(),
                password: pw,
                encryption: ImapEncryption::from_str(&inputs.imap_encryption)
                    .unwrap_or(ImapEncryption::Ssl),
                sent_folder: None,
                drafts_folder: None,
                trash_folder: None,
                spam_folder: None,
                archive_folder: None,
                is_default: true,
            };
            ImapConfiguration::create(&state.db, user_id, &create_req, &key)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("create IMAP: {e}")))?;
        }
    }

    let smtp_encrypted = match inputs.smtp_password.as_deref() {
        Some(pw) => Some(
            encrypt_api_key(pw, &key)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("encrypt SMTP password: {e}")))?,
        ),
        None => None,
    };

    match smtp_existing.as_ref() {
        Some(row) => {
            SmtpConfiguration::update(
                &state.db,
                row.id,
                user_id,
                None, // name unchanged
                Some(&inputs.smtp_host),
                Some(inputs.smtp_port),
                Some(&inputs.smtp_username),
                smtp_encrypted.as_deref(),
                Some(&inputs.smtp_encryption),
                Some(inputs.smtp_from_address.as_deref()),
            )
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("update SMTP: {e}")))?;
        }
        None => {
            let pw = inputs
                .smtp_password
                .clone()
                .expect("validate_inputs enforces SMTP password on create");
            let encrypted = encrypt_api_key(&pw, &key)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("encrypt SMTP password: {e}")))?;
            let created = SmtpConfiguration::create(
                &state.db,
                user_id,
                DEFAULT_CONFIG_LABEL,
                &inputs.smtp_host,
                inputs.smtp_port,
                &inputs.smtp_username,
                &encrypted,
                &inputs.smtp_encryption,
                inputs.smtp_from_address.as_deref(),
            )
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("create SMTP: {e}")))?;
            // Promote the brand-new row to default — `create` doesn't
            // touch is_default, so without this the user's first save
            // would land a row that the resolve helper ignores.
            let _ = SmtpConfiguration::set_default(&state.db, created.id, user_id).await;
        }
    }

    // Drop the per-user caches so the next /api request picks up the new
    // defaults without waiting for the TTL.
    let _ = state.cache.invalidate_user_imap_config(&user_id.to_string()).await;
    let _ = state.cache.invalidate_user_smtp_config(&user_id.to_string()).await;

    tracing::info!(
        user_id = ?user_id,
        force_save,
        "classic BYOK settings saved"
    );

    // 303 to self with a flash banner. POST-Redirect-Get keeps a reload
    // from re-submitting the form.
    let target = format!(
        "{}?flash={}",
        BYOK_PATH,
        if force_save {
            "saved_no_test"
        } else {
            "saved"
        }
    );
    Ok((StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response())
}

// ---------- Validation ----------

#[derive(Debug)]
struct ValidatedInputs {
    imap_host: String,
    imap_port: i32,
    imap_username: String,
    /// Some when the user typed a new password; None when the field was
    /// blank AND a saved row exists (i.e. "keep existing").
    imap_password: Option<String>,
    imap_encryption: String,

    smtp_host: String,
    smtp_port: i32,
    smtp_username: String,
    smtp_password: Option<String>,
    smtp_encryption: String,
    smtp_from_address: Option<String>,
}

fn validate_inputs(
    form: &ByokForm,
    imap_existing: bool,
    smtp_existing: bool,
) -> Result<ValidatedInputs, String> {
    let imap_host = form.imap_host.as_deref().unwrap_or("").trim().to_string();
    let imap_username = form.imap_username.as_deref().unwrap_or("").trim().to_string();
    let imap_password_raw = form.imap_password.as_deref().unwrap_or("");
    let imap_encryption = form.imap_encryption.as_deref().unwrap_or("").trim().to_string();

    let smtp_host = form.smtp_host.as_deref().unwrap_or("").trim().to_string();
    let smtp_username = form.smtp_username.as_deref().unwrap_or("").trim().to_string();
    let smtp_password_raw = form.smtp_password.as_deref().unwrap_or("");
    let smtp_encryption = form.smtp_encryption.as_deref().unwrap_or("").trim().to_string();
    let smtp_from_address_raw = form.smtp_from_address.as_deref().unwrap_or("").trim().to_string();

    if imap_host.is_empty() {
        return Err("IMAP host cannot be empty.".to_string());
    }
    if imap_host.len() > MAX_FIELD_LEN {
        return Err("IMAP host is too long.".to_string());
    }
    if imap_username.is_empty() {
        return Err("IMAP username cannot be empty.".to_string());
    }
    if imap_username.len() > MAX_FIELD_LEN {
        return Err("IMAP username is too long.".to_string());
    }
    if smtp_host.is_empty() {
        return Err("SMTP host cannot be empty.".to_string());
    }
    if smtp_host.len() > MAX_FIELD_LEN {
        return Err("SMTP host is too long.".to_string());
    }
    if smtp_username.is_empty() {
        return Err("SMTP username cannot be empty.".to_string());
    }
    if smtp_username.len() > MAX_FIELD_LEN {
        return Err("SMTP username is too long.".to_string());
    }
    if smtp_from_address_raw.len() > MAX_FIELD_LEN {
        return Err("SMTP from address is too long.".to_string());
    }

    let imap_port = parse_port(form.imap_port.as_deref().unwrap_or(""), "IMAP")?;
    let smtp_port = parse_port(form.smtp_port.as_deref().unwrap_or(""), "SMTP")?;

    if ImapEncryption::from_str(&imap_encryption).is_err() {
        return Err("IMAP encryption must be SSL/TLS, STARTTLS, or None.".to_string());
    }
    if SmtpEncryption::from_str(&smtp_encryption).is_err() {
        return Err("SMTP encryption must be SSL/TLS, STARTTLS, or None.".to_string());
    }

    if imap_password_raw.len() > MAX_PASSWORD_LEN {
        return Err("IMAP password is too long.".to_string());
    }
    if smtp_password_raw.len() > MAX_PASSWORD_LEN {
        return Err("SMTP password is too long.".to_string());
    }

    let imap_password = match imap_password_raw {
        "" if imap_existing => None,
        "" => return Err("IMAP password is required for the first save.".to_string()),
        pw => Some(pw.to_string()),
    };
    let smtp_password = match smtp_password_raw {
        "" if smtp_existing => None,
        "" => return Err("SMTP password is required for the first save.".to_string()),
        pw => Some(pw.to_string()),
    };

    Ok(ValidatedInputs {
        imap_host,
        imap_port,
        imap_username,
        imap_password,
        imap_encryption,

        smtp_host,
        smtp_port,
        smtp_username,
        smtp_password,
        smtp_encryption,
        smtp_from_address: if smtp_from_address_raw.is_empty() {
            None
        } else {
            Some(smtp_from_address_raw)
        },
    })
}

fn parse_port(raw: &str, label: &str) -> Result<i32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} port is required."));
    }
    let n: i64 = trimmed
        .parse()
        .map_err(|_| format!("{label} port must be a whole number."))?;
    if !(1..=65535).contains(&n) {
        return Err(format!("{label} port must be between 1 and 65535."));
    }
    Ok(n as i32)
}

/// Resolve which password to use for an actual TCP probe — submitted
/// plaintext if the user typed something, otherwise the decrypted saved
/// password. Errors when neither is available.
fn resolve_password_for_probe(
    submitted: Option<&str>,
    existing_encrypted: Option<&str>,
    key: &[u8; 32],
) -> Result<String, String> {
    match submitted {
        Some(pw) if !pw.is_empty() => Ok(pw.to_string()),
        _ => match existing_encrypted {
            Some(enc) => crate::models::ai_config::decrypt_api_key(enc, key)
                .map_err(|e| format!("could not decrypt saved password ({e})")),
            None => Err("type the password to test (no saved value yet)".to_string()),
        },
    }
}

// ---------- Probes ----------

/// IMAP TCP-connect + LOGIN probe. Mirrors `handlers::imap_config::test_imap_connection`
/// so a future fix to either path can be lifted to a shared helper without
/// changing the wire protocol.
async fn probe_imap(
    host: &str,
    port: i32,
    username: &str,
    password: &str,
    encryption: &str,
) -> (bool, String) {
    use async_imap::Client;
    use tokio::net::TcpStream;
    use tokio_util::compat::TokioAsyncReadCompatExt;

    let port_u16 = port as u16;
    let tcp = match TcpStream::connect((host, port_u16)).await {
        Ok(s) => s,
        Err(e) => return (false, format!("TCP connect failed: {e}")),
    };

    match encryption {
        "ssl" => {
            let tls = async_native_tls::TlsConnector::new();
            let tls_stream = match tls.connect(host, tcp.compat()).await {
                Ok(s) => s,
                Err(e) => return (false, format!("TLS handshake failed: {e}")),
            };
            let client = Client::new(tls_stream);
            match client.login(username, password).await {
                Ok(mut s) => {
                    let _ = s.logout().await;
                    (true, "IMAP login succeeded.".to_string())
                }
                Err((e, _)) => (false, format!("LOGIN failed: {e}")),
            }
        }
        "starttls" | "none" => {
            let client = Client::new(tcp.compat());
            match client.login(username, password).await {
                Ok(mut s) => {
                    let _ = s.logout().await;
                    (true, "IMAP login succeeded.".to_string())
                }
                Err((e, _)) => (false, format!("LOGIN failed: {e}")),
            }
        }
        other => (false, format!("unknown IMAP encryption: {other}")),
    }
}

/// SMTP connection probe. Builds a lettre transport and runs a no-op
/// connectivity check (`test_connection`) rather than the full send-self
/// probe `smtp_tester::test_smtp_connection` does. Sending self-mail on
/// every settings page save would amplify volume against the user's
/// own MTA — `test_connection` checks the TCP + TLS + AUTH handshake
/// without dispatching a real message.
async fn probe_smtp(
    host: &str,
    port: i32,
    username: &str,
    password: &str,
    encryption: &str,
) -> (bool, String) {
    use lettre::{transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor};

    let creds = Credentials::new(username.to_string(), password.to_string());
    let port_u16 = port as u16;

    let transport_result: Result<AsyncSmtpTransport<Tokio1Executor>, String> = match encryption {
        "ssl" => AsyncSmtpTransport::<Tokio1Executor>::relay(host)
            .map(|b| b.port(port_u16).credentials(creds).build())
            .map_err(|e| format!("SSL relay setup failed: {e}")),
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map(|b| b.port(port_u16).credentials(creds).build())
            .map_err(|e| format!("STARTTLS relay setup failed: {e}")),
        "none" => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port_u16)
            .credentials(creds)
            .build()),
        other => Err(format!("unknown SMTP encryption: {other}")),
    };

    let transport = match transport_result {
        Ok(t) => t,
        Err(msg) => return (false, msg),
    };

    match transport.test_connection().await {
        Ok(true) => (true, "SMTP handshake succeeded.".to_string()),
        Ok(false) => (false, "SMTP server rejected the test connection.".to_string()),
        Err(e) => (false, format!("SMTP connection failed: {e}")),
    }
}

// ---------- Helpers ----------

fn parse_user_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims.sub.parse().map_err(|_| {
        AppError::Internal(anyhow::anyhow!(
            "classic settings/byok: invalid mailbox id in claims"
        ))
    })
}

fn render_html<T: Template>(status: StatusCode, template: &T) -> Result<Response, AppError> {
    let body = template
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic byok template render: {e}")))?;
    Ok((
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

/// Re-render the form with the submitted values (so a validation failure
/// or test result doesn't make the user retype everything). Passwords
/// are NEVER echoed back — the fields render blank.
#[allow(clippy::too_many_arguments)]
async fn re_render_form(
    _state: &AppState,
    form: &ByokForm,
    imap_existing: bool,
    smtp_existing: bool,
    imap_test: Option<(bool, String)>,
    smtp_test: Option<(bool, String)>,
    error: Option<String>,
    success: Option<String>,
    tested_ok: bool,
    csrf_token: &str,
    csp_nonce: &str,
    _user_id: Uuid,
    status: StatusCode,
) -> Result<Response, AppError> {
    let presets = build_preset_views();
    let picked_name = form.picked_name.clone().unwrap_or_default();
    let picked_hint = presets
        .iter()
        .find(|p| p.name == picked_name)
        .map(|p| p.hint.clone())
        .unwrap_or_default();

    let template = ByokFormTemplate {
        imap_existing,
        smtp_existing,

        imap_host: form.imap_host.clone().unwrap_or_default(),
        imap_port: form.imap_port.clone().unwrap_or_default(),
        imap_username: form.imap_username.clone().unwrap_or_default(),
        imap_encryption: form
            .imap_encryption
            .clone()
            .unwrap_or_else(|| "ssl".to_string()),

        smtp_host: form.smtp_host.clone().unwrap_or_default(),
        smtp_port: form.smtp_port.clone().unwrap_or_default(),
        smtp_username: form.smtp_username.clone().unwrap_or_default(),
        smtp_encryption: form
            .smtp_encryption
            .clone()
            .unwrap_or_else(|| "starttls".to_string()),
        smtp_from_address: form.smtp_from_address.clone().unwrap_or_default(),

        presets,
        picked_name,
        picked_hint,

        imap_test,
        smtp_test,
        error,
        success,
        tested_ok: if tested_ok { "1".to_string() } else { String::new() },

        csrf_token: csrf_token.to_string(),
        csp_nonce: csp_nonce.to_string(),
    };

    render_html(status, &template)
}

/// Lower `provider_presets()` (the shared `serde_json::Value` source)
/// into the strongly-typed `PresetView`s the template indexes.
pub(crate) fn build_preset_views() -> Vec<PresetView> {
    provider_presets()
        .into_iter()
        .filter_map(|v| {
            let name = v.get("name")?.as_str()?.to_string();
            let imap = v.get("imap")?;
            let smtp = v.get("smtp")?;
            Some(PresetView {
                name,
                imap_host: imap.get("host")?.as_str()?.to_string(),
                imap_port: imap.get("port")?.as_i64()?,
                imap_encryption: imap.get("encryption")?.as_str()?.to_string(),
                smtp_host: smtp.get("host")?.as_str()?.to_string(),
                smtp_port: smtp.get("port")?.as_i64()?,
                smtp_encryption: smtp.get("encryption")?.as_str()?.to_string(),
                hint: v
                    .get("hint")
                    .and_then(|h| h.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

/// Pick which preset (if any) the GET-side picker should highlight, and
/// what hint to show under it.
fn resolve_preset(
    presets: &[PresetView],
    picked_query: Option<&str>,
    imap: &Option<ImapConfiguration>,
) -> (String, String) {
    if let Some(name) = picked_query {
        if let Some(p) = presets.iter().find(|p| p.name == name) {
            return (p.name.clone(), p.hint.clone());
        }
        return (String::new(), String::new());
    }
    // No query — match the saved IMAP host to a known preset so the
    // picker highlights "Gmail" when the user already saved Gmail's
    // servers from the signup wizard.
    if let Some(cfg) = imap {
        if let Some(p) = presets.iter().find(|p| p.imap_host == cfg.host) {
            return (p.name.clone(), p.hint.clone());
        }
    }
    (String::new(), String::new())
}

fn build_success_flash(query: &ByokQuery) -> Option<String> {
    match query.flash.as_deref()? {
        "saved" => Some("Mail server settings saved.".to_string()),
        "saved_no_test" => Some(
            "Mail server settings saved without a connection test. You can test later."
                .to_string(),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_template() -> ByokFormTemplate {
        ByokFormTemplate {
            imap_existing: true,
            smtp_existing: true,
            imap_host: "imap.gmail.com".to_string(),
            imap_port: "993".to_string(),
            imap_username: "alice@gmail.com".to_string(),
            imap_encryption: "ssl".to_string(),
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: "587".to_string(),
            smtp_username: "alice@gmail.com".to_string(),
            smtp_encryption: "starttls".to_string(),
            smtp_from_address: String::new(),
            presets: build_preset_views(),
            picked_name: "Gmail".to_string(),
            picked_hint: "Use a Google App Password, not your account password.".to_string(),
            imap_test: None,
            smtp_test: None,
            error: None,
            success: None,
            tested_ok: String::new(),
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce".to_string(),
        }
    }

    fn empty_form() -> ByokForm {
        ByokForm {
            imap_host: Some("imap.gmail.com".to_string()),
            imap_port: Some("993".to_string()),
            imap_username: Some("alice@gmail.com".to_string()),
            imap_password: Some(String::new()),
            imap_encryption: Some("ssl".to_string()),
            smtp_host: Some("smtp.gmail.com".to_string()),
            smtp_port: Some("587".to_string()),
            smtp_username: Some("alice@gmail.com".to_string()),
            smtp_password: Some(String::new()),
            smtp_encryption: Some("starttls".to_string()),
            smtp_from_address: Some(String::new()),
            action: Some("save".to_string()),
            tested_ok: Some("1".to_string()),
            picked_name: Some(String::new()),
            csrf: "t".to_string(),
        }
    }

    // ----- Constants -----

    #[test]
    fn byok_path_locked() {
        assert_eq!(BYOK_PATH, "/classic/settings/byok");
    }

    #[test]
    fn byok_test_path_locked() {
        assert_eq!(BYOK_TEST_PATH, "/classic/settings/byok/test");
    }

    #[test]
    fn max_field_len_generous_but_bounded() {
        assert!(MAX_FIELD_LEN >= 256);
        assert!(MAX_FIELD_LEN <= 4096);
    }

    #[test]
    fn max_password_len_handles_app_passwords() {
        // Google app passwords are 16 chars, but a hostile caller could
        // submit a 1MB password to grind Argon2id (NB: BYOK doesn't
        // Argon2id, but the encryption path still copies the buffer).
        assert!(MAX_PASSWORD_LEN >= 64);
        assert!(MAX_PASSWORD_LEN <= 16 * 1024);
    }

    // ----- Template -----

    #[test]
    fn template_renders_form_action() {
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains(&format!("action=\"{}\"", BYOK_PATH)),
            "form action missing: {body}"
        );
        assert!(body.contains("method=\"post\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("value=\"test-csrf-token\""));
    }

    #[test]
    fn template_renders_imap_and_smtp_fields() {
        let body = fresh_template().render().expect("renders");
        for name in [
            "name=\"imap_host\"",
            "name=\"imap_port\"",
            "name=\"imap_username\"",
            "name=\"imap_password\"",
            "name=\"imap_encryption\"",
            "name=\"smtp_host\"",
            "name=\"smtp_port\"",
            "name=\"smtp_username\"",
            "name=\"smtp_password\"",
            "name=\"smtp_encryption\"",
            "name=\"smtp_from_address\"",
        ] {
            assert!(body.contains(name), "missing form field {name}");
        }
    }

    #[test]
    fn template_passwords_render_as_type_password() {
        // Spec: "Password field uses type=password".
        let body = fresh_template().render().expect("renders");
        // Two `type="password"` inputs (IMAP + SMTP).
        assert!(
            body.matches("type=\"password\"").count() >= 2,
            "expected two type=\"password\" inputs"
        );
    }

    #[test]
    fn template_passwords_never_carry_a_value_attribute() {
        // Defence-in-depth: even if a future refactor wires a `password`
        // field on the template struct, the HTML MUST NOT echo it back.
        let body = fresh_template().render().expect("renders");
        let pw_open = body.find("name=\"imap_password\"").expect("imap_password present");
        let window_start = body[..pw_open].rfind("<input").unwrap_or(0);
        let window_end = body[pw_open..]
            .find('>')
            .map(|rel| pw_open + rel)
            .unwrap_or(body.len());
        let window = &body[window_start..window_end];
        assert!(
            !window.contains("value=\""),
            "imap_password input must NOT render a value= attribute, got: {window}"
        );
    }

    #[test]
    fn template_echoes_imap_and_smtp_non_secret_values() {
        let body = fresh_template().render().expect("renders");
        assert!(body.contains("imap.gmail.com"));
        assert!(body.contains("smtp.gmail.com"));
        assert!(body.contains("value=\"993\""));
        assert!(body.contains("value=\"587\""));
        assert!(body.contains("alice@gmail.com"));
    }

    #[test]
    fn template_renders_test_and_save_buttons() {
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains("value=\"test\""),
            "Test button (action=test) missing from form"
        );
        assert!(
            body.contains("value=\"save\""),
            "Save button (action=save) missing from form"
        );
        assert!(
            body.contains("value=\"save_no_test\""),
            "Save-without-testing button (action=save_no_test) missing"
        );
    }

    #[test]
    fn template_test_button_posts_to_test_endpoint() {
        // The Test form posts to /classic/settings/byok/test per the spec.
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains(&format!("action=\"{}\"", BYOK_TEST_PATH)),
            "test endpoint missing as form action: {body}"
        );
    }

    #[test]
    fn template_renders_imap_test_success_banner() {
        let mut t = fresh_template();
        t.imap_test = Some((true, "IMAP login succeeded.".to_string()));
        let body = t.render().expect("renders");
        assert!(body.contains("alert-success"));
        assert!(body.contains("IMAP login succeeded."));
    }

    #[test]
    fn template_renders_imap_test_failure_banner() {
        let mut t = fresh_template();
        t.imap_test = Some((false, "TCP connect failed: timeout".to_string()));
        let body = t.render().expect("renders");
        assert!(body.contains("alert-error"));
        assert!(body.contains("TCP connect failed: timeout"));
    }

    #[test]
    fn template_renders_smtp_test_banners() {
        let mut t = fresh_template();
        t.smtp_test = Some((true, "SMTP handshake succeeded.".to_string()));
        let body = t.render().expect("renders");
        assert!(body.contains("SMTP handshake succeeded."));
    }

    #[test]
    fn template_renders_top_level_error_banner() {
        let mut t = fresh_template();
        t.error = Some("IMAP host cannot be empty.".to_string());
        let body = t.render().expect("renders");
        assert!(body.contains("role=\"alert\""));
        assert!(body.contains("IMAP host cannot be empty."));
    }

    #[test]
    fn template_renders_success_flash() {
        let mut t = fresh_template();
        t.success = Some("Mail server settings saved.".to_string());
        let body = t.render().expect("renders");
        assert!(body.contains("alert-success"));
        assert!(body.contains("role=\"status\""));
        assert!(body.contains("Mail server settings saved."));
    }

    #[test]
    fn template_renders_provider_picker_links() {
        let body = fresh_template().render().expect("renders");
        assert!(body.contains("Gmail"));
        assert!(body.contains("Outlook"));
    }

    #[test]
    fn template_includes_logout_partial() {
        let body = fresh_template().render().expect("renders");
        assert!(
            body.contains("action=\"/classic/logout\""),
            "logout partial must render on settings/byok"
        );
    }

    #[test]
    fn template_has_zero_script_tags() {
        let body = fresh_template().render().expect("renders");
        assert!(!body.contains("<script"));
    }

    #[test]
    fn template_html_escapes_hostile_host() {
        let mut t = fresh_template();
        t.imap_host = "\"><script>alert(1)</script>".to_string();
        let body = t.render().expect("renders");
        assert!(
            !body.contains("\"><script>alert(1)</script>"),
            "raw <script> leaked from imap_host: {body}"
        );
    }

    #[test]
    fn template_renders_cross_links_to_other_settings_pages() {
        let body = fresh_template().render().expect("renders");
        for link in [
            "/classic/settings/signature",
            "/classic/settings/password",
            "/classic/settings/vacation",
            "/classic/settings/sessions",
        ] {
            assert!(body.contains(link), "missing settings crosslink {link}");
        }
    }

    #[test]
    fn template_carries_tested_ok_hidden_input() {
        let mut t = fresh_template();
        t.tested_ok = "1".to_string();
        let body = t.render().expect("renders");
        assert!(
            body.contains("name=\"tested_ok\""),
            "tested_ok hidden input missing"
        );
        assert!(body.contains("value=\"1\""));
    }

    // ----- validate_inputs -----

    #[test]
    fn validate_inputs_accepts_complete_form() {
        let mut form = empty_form();
        form.imap_password = Some("secret".to_string());
        form.smtp_password = Some("secret2".to_string());
        let v = validate_inputs(&form, false, false).expect("ok");
        assert_eq!(v.imap_host, "imap.gmail.com");
        assert_eq!(v.imap_port, 993);
        assert_eq!(v.smtp_port, 587);
        assert_eq!(v.imap_password.as_deref(), Some("secret"));
        assert_eq!(v.smtp_password.as_deref(), Some("secret2"));
    }

    #[test]
    fn validate_inputs_keep_existing_password_on_edit() {
        // Blank password + existing row → password=None (means "keep
        // existing").
        let form = empty_form();
        let v = validate_inputs(&form, true, true).expect("ok");
        assert!(v.imap_password.is_none());
        assert!(v.smtp_password.is_none());
    }

    #[test]
    fn validate_inputs_rejects_blank_password_on_create() {
        let form = empty_form();
        let err = validate_inputs(&form, false, true).unwrap_err();
        assert!(err.contains("IMAP password is required"), "err: {err}");
    }

    #[test]
    fn validate_inputs_rejects_empty_imap_host() {
        let mut form = empty_form();
        form.imap_host = Some(String::new());
        let err = validate_inputs(&form, true, true).unwrap_err();
        assert!(err.contains("IMAP host"), "err: {err}");
    }

    #[test]
    fn validate_inputs_rejects_empty_smtp_username() {
        let mut form = empty_form();
        form.smtp_username = Some(String::new());
        let err = validate_inputs(&form, true, true).unwrap_err();
        assert!(err.contains("SMTP username"), "err: {err}");
    }

    #[test]
    fn validate_inputs_rejects_invalid_port() {
        let mut form = empty_form();
        form.imap_port = Some("hello".to_string());
        let err = validate_inputs(&form, true, true).unwrap_err();
        assert!(err.contains("IMAP port"), "err: {err}");
    }

    #[test]
    fn validate_inputs_rejects_out_of_range_port() {
        let mut form = empty_form();
        form.smtp_port = Some("70000".to_string());
        let err = validate_inputs(&form, true, true).unwrap_err();
        assert!(err.contains("SMTP port"), "err: {err}");
        assert!(err.contains("65535"));
    }

    #[test]
    fn validate_inputs_rejects_unknown_imap_encryption() {
        let mut form = empty_form();
        form.imap_encryption = Some("rot13".to_string());
        let err = validate_inputs(&form, true, true).unwrap_err();
        assert!(err.contains("IMAP encryption"));
    }

    #[test]
    fn validate_inputs_rejects_overly_long_host() {
        let mut form = empty_form();
        form.imap_host = Some("a".repeat(MAX_FIELD_LEN + 1));
        let err = validate_inputs(&form, true, true).unwrap_err();
        assert!(err.contains("IMAP host"));
    }

    #[test]
    fn validate_inputs_rejects_overly_long_password() {
        let mut form = empty_form();
        form.imap_password = Some("a".repeat(MAX_PASSWORD_LEN + 1));
        let err = validate_inputs(&form, true, true).unwrap_err();
        assert!(err.contains("IMAP password"));
    }

    #[test]
    fn validate_inputs_strips_blank_from_address() {
        let mut form = empty_form();
        form.smtp_from_address = Some("   ".to_string());
        let v = validate_inputs(&form, true, true).expect("ok");
        assert!(v.smtp_from_address.is_none());
    }

    #[test]
    fn validate_inputs_keeps_non_blank_from_address() {
        let mut form = empty_form();
        form.smtp_from_address = Some("alice@example.com".to_string());
        let v = validate_inputs(&form, true, true).expect("ok");
        assert_eq!(v.smtp_from_address.as_deref(), Some("alice@example.com"));
    }

    // ----- parse_port -----

    #[test]
    fn parse_port_accepts_993() {
        assert_eq!(parse_port("993", "IMAP").unwrap(), 993);
    }

    #[test]
    fn parse_port_rejects_empty() {
        assert!(parse_port("", "IMAP").is_err());
    }

    #[test]
    fn parse_port_rejects_garbage() {
        assert!(parse_port("not-a-port", "IMAP").is_err());
    }

    #[test]
    fn parse_port_rejects_zero() {
        assert!(parse_port("0", "SMTP").is_err());
    }

    #[test]
    fn parse_port_rejects_above_65535() {
        assert!(parse_port("65536", "SMTP").is_err());
    }

    // ----- build_preset_views -----

    #[test]
    fn build_preset_views_includes_gmail() {
        let presets = build_preset_views();
        let gmail = presets
            .iter()
            .find(|p| p.name == "Gmail")
            .expect("Gmail preset present");
        assert_eq!(gmail.imap_host, "imap.gmail.com");
        assert_eq!(gmail.smtp_host, "smtp.gmail.com");
        assert_eq!(gmail.imap_port, 993);
    }

    #[test]
    fn build_preset_views_at_least_five_providers() {
        // The signup wizard's preset list ships with 11 entries; a regression
        // that drops most of them would silently degrade BYOK auto-fill UX
        // too. We check >=5 to leave headroom for future curation without
        // making this assertion brittle.
        assert!(build_preset_views().len() >= 5);
    }

    // ----- resolve_preset -----

    #[test]
    fn resolve_preset_picks_query_when_present() {
        let presets = build_preset_views();
        let (name, hint) = resolve_preset(&presets, Some("Gmail"), &None);
        assert_eq!(name, "Gmail");
        assert!(!hint.is_empty());
    }

    #[test]
    fn resolve_preset_falls_back_to_saved_imap_host() {
        let presets = build_preset_views();
        let saved = Some(ImapConfiguration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Default".to_string(),
            host: "imap.gmail.com".to_string(),
            port: 993,
            username: "alice@gmail.com".to_string(),
            encrypted_password: "".to_string(),
            encryption: "ssl".to_string(),
            sent_folder: None,
            drafts_folder: None,
            trash_folder: None,
            spam_folder: None,
            archive_folder: None,
            is_default: true,
            verified: false,
            last_tested_at: None,
            last_error: None,
            created_at: None,
            updated_at: None,
        });
        let (name, _hint) = resolve_preset(&presets, None, &saved);
        assert_eq!(name, "Gmail");
    }

    #[test]
    fn resolve_preset_returns_empty_when_unmatched() {
        let presets = build_preset_views();
        let (name, hint) = resolve_preset(&presets, Some("UnknownProvider"), &None);
        assert!(name.is_empty());
        assert!(hint.is_empty());
    }

    // ----- build_success_flash -----

    #[test]
    fn build_success_flash_maps_saved() {
        let q = ByokQuery {
            preset: None,
            flash: Some("saved".to_string()),
        };
        assert!(build_success_flash(&q).is_some());
    }

    #[test]
    fn build_success_flash_maps_saved_no_test() {
        let q = ByokQuery {
            preset: None,
            flash: Some("saved_no_test".to_string()),
        };
        let msg = build_success_flash(&q).expect("flash present");
        assert!(msg.contains("without"));
    }

    #[test]
    fn build_success_flash_ignores_hostile_flash() {
        let q = ByokQuery {
            preset: None,
            flash: Some("danger".to_string()),
        };
        assert!(build_success_flash(&q).is_none());
    }

    // ----- resolve_password_for_probe -----

    #[test]
    fn resolve_password_uses_submitted_when_present() {
        let key = derive_encryption_key("test-secret");
        let pw = resolve_password_for_probe(Some("typed-password"), None, &key).unwrap();
        assert_eq!(pw, "typed-password");
    }

    #[test]
    fn resolve_password_falls_back_to_encrypted() {
        let key = derive_encryption_key("test-secret");
        let encrypted = encrypt_api_key("saved-password", &key).unwrap();
        let pw = resolve_password_for_probe(Some(""), Some(&encrypted), &key).unwrap();
        assert_eq!(pw, "saved-password");
    }

    #[test]
    fn resolve_password_errors_when_no_value_anywhere() {
        let key = derive_encryption_key("test-secret");
        let err = resolve_password_for_probe(Some(""), None, &key).unwrap_err();
        assert!(err.contains("no saved value"), "err: {err}");
    }
}
