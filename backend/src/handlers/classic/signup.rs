// Added (TMAIL-374): 3-step server-rendered signup wizard for the `/classic`
// no-JS surface. Mirrors the SPA `frontend/src/components/onboarding/OnboardingWizard.tsx`
// for the BYOK path.
//
// Step 1 — GET/POST `/classic/signup`
//   * GET renders the account form (email + password + optional display name)
//     and issues a fresh `tasmail_classic_signup_draft` cookie pointing at a
//     freshly-created `classic_signup_drafts` row.
//   * POST validates the inputs, creates a Mailbox row (Argon2id via
//     auth_service::hash_password), attaches the mailbox to the draft, advances
//     the draft to step "servers", and 303-redirects to Step 2.
//
// Step 2 — GET/POST `/classic/signup/imap`
//   * GET renders a single combined form: provider preset picker (optional),
//     IMAP host/port/username/password/encryption, SMTP host/port/username/
//     password/encryption. Anything pre-filled from the picker is rendered
//     as a query-string round-trip — no state is persisted between GETs.
//   * POST runs `imap_service::test_connection` AND `smtp_service::test_connection`
//     in parallel. If BOTH pass, the encrypted-at-rest IMAP+SMTP rows are
//     created and the draft advances to "done"; otherwise the form re-renders
//     with inline success/failure markers + a "fix and retry" link.
//
// Step 3 — GET/POST `/classic/signup/done`
//   * GET renders a summary + "Go to inbox" button.
//   * POST creates the real `classic_sessions` row, sets the session cookie,
//     deletes the draft, and 303-redirects to `/classic/folders/INBOX`.
//
// State carried via a signed cookie (`tasmail_classic_signup_draft=<uuid>.<hmac>`)
// pointing at a server-side `classic_signup_drafts` row. NEVER via URL params.
// CSRF on every POST via the same OWASP double-submit-cookie pattern login
// uses — the row's csrf_token also lives in a sibling cookie + the form's
// hidden `_csrf` input.

use askama::Template;
use axum::{
    extract::{Query, State},
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
use crate::models::ai_config::{derive_encryption_key, encrypt_api_key};
use crate::models::classic_signup_draft::{ClassicSignupDraft, SIGNUP_DRAFT_TTL_SECS};
use crate::models::imap_config::{provider_presets, CreateImapConfigRequest, ImapConfiguration, ImapEncryption};
use crate::models::mailbox::Mailbox;
use crate::models::smtp_config::SmtpConfiguration;
use crate::services::auth_service;
use crate::state::AppState;

use super::auth::{create_session_and_cookie, generate_csrf_token, INBOX_PATH};
use super::CspNonce;

type HmacSha256 = Hmac<Sha256>;

/// Cookie carrying the draft id + HMAC signature. Same shape as
/// `tasmail_classic_sid` but scoped to the wizard so it doesn't collide.
pub const SIGNUP_DRAFT_COOKIE: &str = "tasmail_classic_signup_draft";

/// Route prefixes — centralised so a future rename touches one place.
pub const SIGNUP_STEP1_PATH: &str = "/classic/signup";
pub const SIGNUP_STEP2_PATH: &str = "/classic/signup/imap";
pub const SIGNUP_STEP3_PATH: &str = "/classic/signup/done";

/// Default quota for a freshly-signed-up mailbox. TASMail itself doesn't store
/// mail (it's a webmail UI) so this is a generous placeholder — mirrors the
/// SPA signup handler's value in `handlers::auth::signup`.
const DEFAULT_QUOTA_BYTES: i64 = 1_073_741_824;

/// Minimum password length. Mirrors the SPA signup check + auth_service guard
/// so the two surfaces fail symmetrically.
const MIN_PASSWORD_LEN: usize = 8;

// ---------------------------------------------------------------------------
// Cookie HMAC helpers — same shape as classic_session::sign_session_id.
// ---------------------------------------------------------------------------

fn sign_draft_id(jwt_secret: &str, draft_id: Uuid) -> String {
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes())
        .expect("HMAC-SHA256 accepts keys of any length");
    mac.update(draft_id.as_bytes());
    B64URL.encode(mac.finalize().into_bytes())
}

fn verify_draft_signature(jwt_secret: &str, draft_id: Uuid, supplied_sig: &str) -> bool {
    let expected = sign_draft_id(jwt_secret, draft_id);
    if expected.len() != supplied_sig.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in expected.as_bytes().iter().zip(supplied_sig.as_bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Compose the cookie body `<uuid_simple>.<sig_b64url>`. Public for tests.
pub fn build_draft_cookie_value(jwt_secret: &str, draft_id: Uuid) -> String {
    format!("{}.{}", draft_id.as_simple(), sign_draft_id(jwt_secret, draft_id))
}

/// Full Set-Cookie header value for a fresh draft cookie.
///
/// Attributes:
///   * HttpOnly + Secure + SameSite=Strict — same as the pending-2FA cookie;
///     the wizard's POSTs are same-origin so Strict is the safer default.
///   * Path=/classic/signup — narrow scope so it can't leak to other classic
///     routes and can't collide with the post-login session cookie's Path=/
///     scope. Note: this means every step's POST/GET shares the path prefix,
///     which all three steps do.
fn build_set_draft_cookie_header(jwt_secret: &str, draft_id: Uuid) -> String {
    format!(
        "{SIGNUP_DRAFT_COOKIE}={}; HttpOnly; Secure; SameSite=Strict; Path=/classic/signup; Max-Age={SIGNUP_DRAFT_TTL_SECS}",
        build_draft_cookie_value(jwt_secret, draft_id)
    )
}

/// Cookie-clearing header — used after the wizard graduates into a real
/// session and when the handler detects a stale draft.
fn build_clear_draft_cookie_header() -> String {
    format!(
        "{SIGNUP_DRAFT_COOKIE}=; HttpOnly; Secure; SameSite=Strict; Path=/classic/signup; Max-Age=0"
    )
}

/// Parse the request's Cookie header and pull `(draft_id, sig)`.
fn extract_draft_cookie(headers: &HeaderMap) -> Option<(Uuid, String)> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    let raw = cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix(&format!("{SIGNUP_DRAFT_COOKIE}=")))?;
    let (id_part, sig_part) = raw.split_once('.')?;
    let id = Uuid::parse_str(id_part).ok()?;
    Some((id, sig_part.to_string()))
}

/// First-hop X-Forwarded-For + UA. Same shape as the login handler's helper.
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

/// Load the active draft from a verified cookie. Returns None for missing,
/// malformed, sig-failed, expired, or row-not-found cookies — the handlers
/// treat all of those identically (start over).
async fn load_active_draft(
    state: &AppState,
    headers: &HeaderMap,
) -> Option<ClassicSignupDraft> {
    let (id, sig) = extract_draft_cookie(headers)?;
    if !verify_draft_signature(&state.config.jwt.secret, id, &sig) {
        tracing::warn!(?id, "classic signup draft cookie signature mismatch — possible tampering");
        return None;
    }
    ClassicSignupDraft::find_active(&state.db, id).await.ok().flatten()
}

/// Bounce response used when the draft is missing/stale and we need to
/// restart the wizard. Clears the cookie + 303s to Step 1.
fn bounce_to_step1() -> Response {
    let mut resp = Redirect::to(SIGNUP_STEP1_PATH).into_response();
    if let Ok(hv) = HeaderValue::from_str(&build_clear_draft_cookie_header()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    resp
}

// ---------------------------------------------------------------------------
// Step 1: Account
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "classic/signup_account.html")]
pub struct SignupAccountTemplate {
    /// Pre-fill across a failed POST so the user doesn't retype the address.
    pub email: String,
    /// Optional display name; preserved across re-renders.
    pub display_name: String,
    /// `Some("…")` on any failure mode (validation, CSRF, conflict). Rendered
    /// inside a `role="alert"` block.
    pub error: Option<String>,
    /// CSRF token bound to the draft row. Identical bytes appear in the
    /// `_csrf` form field AND in the row's `csrf_token` column. Validated by
    /// `validate_csrf_token` on POST.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html.
    pub csp_nonce: String,
    /// Wizard step indicator: 1, 2, or 3. Reserved for the shared progress
    /// component a future P2 refactor will pull into a partial template.
    #[allow(dead_code)]
    pub current_step: u8,
}

impl SignupAccountTemplate {
    fn new(
        email: impl Into<String>,
        display_name: impl Into<String>,
        error: Option<String>,
        csrf_token: impl Into<String>,
        csp_nonce: impl Into<String>,
    ) -> Self {
        Self {
            email: email.into(),
            display_name: display_name.into(),
            error,
            csrf_token: csrf_token.into(),
            csp_nonce: csp_nonce.into(),
            current_step: 1,
        }
    }
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct SignupAccountForm {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// Render the Step 1 form. Reuses an existing draft cookie if it carries a
/// still-active row at step "account"; otherwise issues a fresh draft.
pub async fn get_step1_account(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // If the user already has a valid Classic UI session, send them to the inbox
    // — typing `/classic/signup` while logged in is almost always a mistake.
    if headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(';').map(str::trim).any(|p| {
                p.starts_with(&format!("{}=", super::CLASSIC_SESSION_COOKIE))
            })
        })
        .unwrap_or(false)
    {
        return Ok(Redirect::to(INBOX_PATH).into_response());
    }

    let (ip, ua) = extract_audit_fields(&headers);

    // Reuse an existing draft if the cookie still resolves to a live row.
    // Otherwise mint a fresh draft + cookie. This lets the user hit Back from
    // Step 2 and land here with the form they were about to submit.
    let draft = match load_active_draft(&state, &headers).await {
        Some(d) => {
            // Best-effort audit bump; ignore errors.
            let _ = ClassicSignupDraft::touch(&state.db, d.id, ip.as_deref(), ua.as_deref()).await;
            d
        }
        None => {
            let csrf = generate_csrf_token();
            ClassicSignupDraft::create(&state.db, &csrf, ip.as_deref(), ua.as_deref())
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("draft create failed: {e}")))?
        }
    };

    let template = SignupAccountTemplate::new(
        String::new(),
        String::new(),
        None,
        &draft.csrf_token,
        csp_nonce.as_str(),
    );
    render_with_draft_cookie(
        StatusCode::OK,
        template,
        Some(build_set_draft_cookie_header(&state.config.jwt.secret, draft.id)),
    )
}

/// Render any template with an optional Set-Cookie attached. Shared by all
/// three steps' GET + POST re-render paths.
fn render_with_draft_cookie<T: Template>(
    status: StatusCode,
    template: T,
    set_cookie: Option<String>,
) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!("classic signup template render failed: {e}"))
    })?;
    let mut resp = (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response();
    if let Some(c) = set_cookie {
        if let Ok(hv) = HeaderValue::from_str(&c) {
            resp.headers_mut().append(header::SET_COOKIE, hv);
        }
    }
    Ok(resp)
}

/// POST Step 1 — validate + create Mailbox + attach to draft + redirect to
/// Step 2.
///
/// On any failure (CSRF, validation, conflict) re-renders the form with an
/// error message and the user's submitted values preserved. The draft cookie
/// is kept across failures so the user can resume on retry without losing
/// state.
pub async fn post_step1_account(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<SignupAccountForm>,
) -> Result<Response, AppError> {
    // CSRF FIRST — same ordering as the login handler.
    let Some(draft) = load_active_draft(&state, &headers).await else {
        // Cookie missing/stale/forged — bounce to Step 1 to mint a fresh draft.
        return Ok(bounce_to_step1());
    };
    if form.csrf.is_empty() || !validate_csrf_token(&form.csrf, &draft.csrf_token) {
        return render_step1_failure(
            &state,
            &form.email,
            &form.display_name,
            "Your form session expired. Please try again.",
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        );
    }

    // Validate inputs symmetrically — any field-level error returns to the
    // same template with a generic message.
    let email = form.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return render_step1_failure(
            &state,
            &form.email,
            &form.display_name,
            "Please enter a valid email address.",
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        );
    }
    if form.password.len() < MIN_PASSWORD_LEN {
        return render_step1_failure(
            &state,
            &form.email,
            &form.display_name,
            "Password must be at least 8 characters.",
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        );
    }

    // Duplicate check — clean 409 instead of a unique-violation 500.
    if Mailbox::find_by_username(&state.db, &email)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("mailbox lookup failed: {e}")))?
        .is_some()
    {
        return render_step1_failure(
            &state,
            &form.email,
            &form.display_name,
            "An account with this email address already exists. Try signing in instead.",
            StatusCode::CONFLICT,
            csp_nonce.as_str(),
            &draft,
        );
    }

    // Resolve the synthetic byok.tasmail domain — same as handlers::auth::signup.
    let domain_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM domains WHERE name = 'byok.tasmail' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("domain lookup failed: {e}")))?
    .ok_or_else(|| AppError::Internal(anyhow::anyhow!(
        "byok.tasmail domain row missing — re-run migration 056"
    )))?;

    let password_hash = auth_service::hash_password(&form.password)?;
    let display_name = if form.display_name.trim().is_empty() {
        None
    } else {
        Some(form.display_name.trim())
    };

    let mailbox = Mailbox::create(
        &state.db,
        &email,
        &password_hash,
        domain_id,
        display_name,
        DEFAULT_QUOTA_BYTES,
    )
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("mailbox create failed: {e}")))?;

    let updated = ClassicSignupDraft::attach_mailbox(&state.db, draft.id, mailbox.id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("draft attach_mailbox failed: {e}")))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("draft disappeared between read and update")))?;

    tracing::info!(
        mailbox_id = ?mailbox.id,
        draft_id = ?updated.id,
        "classic signup step 1 — mailbox created"
    );

    // 303 → Step 2. Keep the same draft cookie (re-issue with refreshed Max-Age).
    let mut resp = Redirect::to(SIGNUP_STEP2_PATH).into_response();
    if let Ok(hv) = HeaderValue::from_str(&build_set_draft_cookie_header(
        &state.config.jwt.secret,
        updated.id,
    )) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    Ok(resp)
}

fn render_step1_failure(
    state: &AppState,
    email: &str,
    display_name: &str,
    error: &str,
    status: StatusCode,
    csp_nonce: &str,
    draft: &ClassicSignupDraft,
) -> Result<Response, AppError> {
    let template = SignupAccountTemplate::new(
        email,
        display_name,
        Some(error.to_string()),
        &draft.csrf_token,
        csp_nonce,
    );
    render_with_draft_cookie(
        status,
        template,
        Some(build_set_draft_cookie_header(&state.config.jwt.secret, draft.id)),
    )
}

// ---------------------------------------------------------------------------
// Step 2: IMAP + SMTP servers
// ---------------------------------------------------------------------------

/// View-model for one provider preset in the picker. Flat shape so the
/// template doesn't have to walk nested JSON.
#[derive(Debug, Clone)]
pub struct PresetView {
    pub name: String,
    /// Provider's primary email domain (e.g. "gmail.com"). Reserved for a
    /// future P2 "suggest the right preset from the user's email" enhancement
    /// — currently only the `name` drives picker behaviour.
    #[allow(dead_code)]
    pub domain: String,
    pub imap_host: String,
    pub imap_port: i32,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_encryption: String,
    pub hint: String,
}

fn load_presets() -> Vec<PresetView> {
    provider_presets()
        .into_iter()
        .filter_map(|v| {
            let name = v.get("name")?.as_str()?.to_string();
            let domain = v.get("domain")?.as_str()?.to_string();
            let imap = v.get("imap")?;
            let smtp = v.get("smtp")?;
            Some(PresetView {
                name,
                domain,
                imap_host: imap.get("host")?.as_str()?.to_string(),
                imap_port: imap.get("port")?.as_i64()? as i32,
                imap_encryption: imap.get("encryption")?.as_str()?.to_string(),
                smtp_host: smtp.get("host")?.as_str()?.to_string(),
                smtp_port: smtp.get("port")?.as_i64()? as i32,
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

#[derive(Template)]
#[template(path = "classic/signup_servers.html")]
pub struct SignupServersTemplate {
    pub email: String,
    pub presets: Vec<PresetView>,
    /// Currently-picked preset name (matches `presets[i].name`) — empty
    /// string for "Custom / None of these".
    pub picked_name: String,
    pub picked_hint: String,
    pub imap_host: String,
    pub imap_port: String,
    pub imap_username: String,
    /// Always rendered as empty string in the actual template — kept on the
    /// struct so the test suite can assert "never reflect password back".
    #[allow(dead_code)]
    pub imap_password: String,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: String,
    pub smtp_username: String,
    #[allow(dead_code)]
    pub smtp_password: String,
    pub smtp_encryption: String,
    pub imap_error: Option<String>,
    pub smtp_error: Option<String>,
    /// Top-of-form non-field error (CSRF, encryption save).
    pub error: Option<String>,
    pub csrf_token: String,
    pub csp_nonce: String,
    /// Same as `SignupAccountTemplate::current_step` — reserved for the
    /// shared progress component a future P2 refactor will extract.
    #[allow(dead_code)]
    pub current_step: u8,
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct PresetQuery {
    #[serde(default)]
    pub preset: Option<String>,
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct SignupServersForm {
    #[serde(default)]
    pub picked_name: String,
    pub imap_host: String,
    pub imap_port: String,
    pub imap_username: String,
    pub imap_password: String,
    pub imap_encryption: String,
    pub smtp_host: String,
    pub smtp_port: String,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_encryption: String,
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// Render Step 2. If `?preset=Name` is in the URL (set by the in-page preset
/// `<a>` links), fold the matching preset's values into the form defaults so
/// the user only types the password.
pub async fn get_step2_servers(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
    Query(query): Query<PresetQuery>,
) -> Result<Response, AppError> {
    let Some(draft) = load_active_draft(&state, &headers).await else {
        return Ok(bounce_to_step1());
    };
    if draft.mailbox_id.is_none() {
        // Tried to jump to Step 2 without completing Step 1. Send them back.
        return Ok(Redirect::to(SIGNUP_STEP1_PATH).into_response());
    }

    let (ip, ua) = extract_audit_fields(&headers);
    let _ = ClassicSignupDraft::touch(&state.db, draft.id, ip.as_deref(), ua.as_deref()).await;

    let email = load_mailbox_email(&state, draft.mailbox_id.unwrap()).await.unwrap_or_default();
    let presets = load_presets();
    let picked = query
        .preset
        .as_deref()
        .and_then(|name| presets.iter().find(|p| p.name == name).cloned());

    let template = SignupServersTemplate {
        email: email.clone(),
        presets: presets.clone(),
        picked_name: picked.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
        picked_hint: picked.as_ref().map(|p| p.hint.clone()).unwrap_or_default(),
        imap_host: picked.as_ref().map(|p| p.imap_host.clone()).unwrap_or_default(),
        imap_port: picked.as_ref().map(|p| p.imap_port.to_string()).unwrap_or_else(|| "993".to_string()),
        imap_username: email.clone(),
        imap_password: String::new(),
        imap_encryption: picked.as_ref().map(|p| p.imap_encryption.clone()).unwrap_or_else(|| "ssl".to_string()),
        smtp_host: picked.as_ref().map(|p| p.smtp_host.clone()).unwrap_or_default(),
        smtp_port: picked.as_ref().map(|p| p.smtp_port.to_string()).unwrap_or_else(|| "587".to_string()),
        smtp_username: email,
        smtp_password: String::new(),
        smtp_encryption: picked.as_ref().map(|p| p.smtp_encryption.clone()).unwrap_or_else(|| "starttls".to_string()),
        imap_error: None,
        smtp_error: None,
        error: None,
        csrf_token: draft.csrf_token.clone(),
        csp_nonce: csp_nonce.as_str().to_string(),
        current_step: 2,
    };
    render_with_draft_cookie(
        StatusCode::OK,
        template,
        Some(build_set_draft_cookie_header(&state.config.jwt.secret, draft.id)),
    )
}

async fn load_mailbox_email(state: &AppState, mailbox_id: Uuid) -> Option<String> {
    Mailbox::find_by_id(&state.db, mailbox_id).await.ok().flatten().map(|m| m.username)
}

/// POST Step 2 — validate, test IMAP+SMTP, save both, advance to Step 3.
pub async fn post_step2_servers(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<SignupServersForm>,
) -> Result<Response, AppError> {
    let Some(draft) = load_active_draft(&state, &headers).await else {
        return Ok(bounce_to_step1());
    };
    if draft.mailbox_id.is_none() {
        return Ok(Redirect::to(SIGNUP_STEP1_PATH).into_response());
    }
    if form.csrf.is_empty() || !validate_csrf_token(&form.csrf, &draft.csrf_token) {
        return render_step2_failure(
            &state,
            &form,
            "Your form session expired. Please try again.",
            None,
            None,
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        ).await;
    }

    // Field-level required checks.
    if form.imap_host.trim().is_empty()
        || form.imap_username.trim().is_empty()
        || form.imap_password.is_empty()
        || form.smtp_host.trim().is_empty()
        || form.smtp_username.trim().is_empty()
        || form.smtp_password.is_empty()
    {
        return render_step2_failure(
            &state,
            &form,
            "Please fill in all IMAP and SMTP fields before continuing.",
            None,
            None,
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        ).await;
    }

    let imap_port: i32 = match form.imap_port.trim().parse::<i32>() {
        Ok(n) if (1..=65_535).contains(&n) => n,
        _ => return render_step2_failure(
            &state,
            &form,
            "IMAP port must be a number between 1 and 65535.",
            None,
            None,
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        ).await,
    };
    let smtp_port: i32 = match form.smtp_port.trim().parse::<i32>() {
        Ok(n) if (1..=65_535).contains(&n) => n,
        _ => return render_step2_failure(
            &state,
            &form,
            "SMTP port must be a number between 1 and 65535.",
            None,
            None,
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        ).await,
    };
    if !matches!(form.imap_encryption.as_str(), "ssl" | "starttls" | "none") {
        return render_step2_failure(
            &state,
            &form,
            "Unknown IMAP encryption. Pick SSL/TLS, STARTTLS, or None.",
            None,
            None,
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        ).await;
    }
    if !matches!(form.smtp_encryption.as_str(), "ssl" | "starttls" | "none") {
        return render_step2_failure(
            &state,
            &form,
            "Unknown SMTP encryption. Pick SSL/TLS, STARTTLS, or None.",
            None,
            None,
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        ).await;
    }

    // Test both connections IN PARALLEL — the user is staring at a spinner-less
    // page so getting both results back as fast as possible matters.
    let imap_test = test_imap(
        &form.imap_host,
        imap_port,
        &form.imap_username,
        &form.imap_password,
        &form.imap_encryption,
    );
    let smtp_test = test_smtp(
        &form.smtp_host,
        smtp_port,
        &form.smtp_username,
        &form.smtp_password,
        &form.smtp_encryption,
    );
    let (imap_result, smtp_result) = tokio::join!(imap_test, smtp_test);

    // If EITHER fails, re-render with the per-section error and a top-of-form
    // hint so the user can fix and retry.
    if imap_result.is_err() || smtp_result.is_err() {
        return render_step2_failure(
            &state,
            &form,
            "We couldn't connect to one or both of your servers. Fix the highlighted setting and retry.",
            imap_result.err().map(|e| e.to_string()),
            smtp_result.err().map(|e| e.to_string()),
            StatusCode::BAD_REQUEST,
            csp_nonce.as_str(),
            &draft,
        ).await;
    }

    // Both tests passed — persist the encrypted-at-rest IMAP + SMTP rows.
    let mailbox_id = draft.mailbox_id.unwrap();
    let key = derive_encryption_key(&state.config.jwt.secret);

    let imap_enc = ImapEncryption::from_str(&form.imap_encryption)
        .map_err(|e| AppError::BadRequest(e))?;
    let imap_req = CreateImapConfigRequest {
        name: pick_config_name(&form.picked_name, "My IMAP server"),
        host: form.imap_host.trim().to_string(),
        port: imap_port,
        username: form.imap_username.trim().to_string(),
        password: form.imap_password.clone(),
        encryption: imap_enc,
        sent_folder: None,
        drafts_folder: None,
        trash_folder: None,
        spam_folder: None,
        archive_folder: None,
        is_default: true,
    };
    ImapConfiguration::create(&state.db, mailbox_id, &imap_req, &key)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("imap_configurations insert failed: {e}")))?;
    let _ = state
        .cache
        .invalidate_user_imap_config(&mailbox_id.to_string())
        .await;

    let smtp_encrypted = encrypt_api_key(&form.smtp_password, &key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("smtp encrypt failed: {e}")))?;
    let smtp_name = pick_config_name(&form.picked_name, "My SMTP server");
    let email = load_mailbox_email(&state, mailbox_id).await;
    let smtp_cfg = SmtpConfiguration::create(
        &state.db,
        mailbox_id,
        &smtp_name,
        form.smtp_host.trim(),
        smtp_port,
        form.smtp_username.trim(),
        &smtp_encrypted,
        &form.smtp_encryption,
        email.as_deref(),
    )
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("smtp_configurations insert failed: {e}")))?;
    // Promote to default so the send path picks it up.
    let _ = SmtpConfiguration::set_default(&state.db, smtp_cfg.id, mailbox_id).await;
    let _ = state
        .cache
        .invalidate_user_smtp_config(&mailbox_id.to_string())
        .await;

    let _ = ClassicSignupDraft::mark_servers_done(&state.db, draft.id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("draft mark_servers_done failed: {e}")))?;

    tracing::info!(
        mailbox_id = ?mailbox_id,
        draft_id = ?draft.id,
        "classic signup step 2 — IMAP + SMTP saved"
    );

    let mut resp = Redirect::to(SIGNUP_STEP3_PATH).into_response();
    if let Ok(hv) = HeaderValue::from_str(&build_set_draft_cookie_header(
        &state.config.jwt.secret,
        draft.id,
    )) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    Ok(resp)
}

/// Pick a friendly name for the saved IMAP/SMTP config. The provider preset
/// name wins; otherwise a generic fallback.
fn pick_config_name(picked: &str, fallback: &str) -> String {
    let trimmed = picked.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

async fn render_step2_failure(
    state: &AppState,
    form: &SignupServersForm,
    error: &str,
    imap_error: Option<String>,
    smtp_error: Option<String>,
    status: StatusCode,
    csp_nonce: &str,
    draft: &ClassicSignupDraft,
) -> Result<Response, AppError> {
    let email = match draft.mailbox_id {
        Some(id) => load_mailbox_email(state, id).await.unwrap_or_default(),
        None => String::new(),
    };
    let presets = load_presets();
    let template = SignupServersTemplate {
        email,
        presets,
        picked_name: form.picked_name.clone(),
        picked_hint: String::new(),
        imap_host: form.imap_host.clone(),
        imap_port: form.imap_port.clone(),
        imap_username: form.imap_username.clone(),
        imap_password: String::new(), // never reflect password back
        imap_encryption: form.imap_encryption.clone(),
        smtp_host: form.smtp_host.clone(),
        smtp_port: form.smtp_port.clone(),
        smtp_username: form.smtp_username.clone(),
        smtp_password: String::new(),
        smtp_encryption: form.smtp_encryption.clone(),
        imap_error,
        smtp_error,
        error: Some(error.to_string()),
        csrf_token: draft.csrf_token.clone(),
        csp_nonce: csp_nonce.to_string(),
        current_step: 2,
    };
    render_with_draft_cookie(
        status,
        template,
        Some(build_set_draft_cookie_header(&state.config.jwt.secret, draft.id)),
    )
}

/// Test an IMAP login. Same shape as `handlers::imap_config::test_imap_connection`
/// — duplicated here so the public test endpoint can stay JSON-shaped while we
/// return inline string errors from the wizard.
async fn test_imap(
    host: &str,
    port: i32,
    username: &str,
    password: &str,
    encryption: &str,
) -> Result<(), anyhow::Error> {
    use async_imap::Client;
    use tokio::net::TcpStream;
    use tokio_util::compat::TokioAsyncReadCompatExt;

    let tcp = TcpStream::connect((host, port as u16))
        .await
        .map_err(|e| anyhow::anyhow!("TCP connect failed: {}", e))?;
    match encryption {
        "ssl" => {
            let tls = async_native_tls::TlsConnector::new();
            let tls_stream = tls
                .connect(host, tcp.compat())
                .await
                .map_err(|e| anyhow::anyhow!("TLS handshake failed: {}", e))?;
            let client = Client::new(tls_stream);
            let mut session = client
                .login(username, password)
                .await
                .map_err(|(e, _)| anyhow::anyhow!("LOGIN failed: {}", e))?;
            let _ = session.logout().await;
            Ok(())
        }
        "starttls" | "none" => {
            let client = Client::new(tcp.compat());
            let mut session = client
                .login(username, password)
                .await
                .map_err(|(e, _)| anyhow::anyhow!("LOGIN failed: {}", e))?;
            let _ = session.logout().await;
            Ok(())
        }
        other => Err(anyhow::anyhow!("Unknown encryption: {}", other)),
    }
}

/// Test an SMTP connection. Mirrors `services::smtp_tester::build_transport`
/// but stops after the connection handshake — we don't want to send a real
/// test email from the signup wizard (the user hasn't agreed to it).
async fn test_smtp(
    host: &str,
    port: i32,
    username: &str,
    password: &str,
    encryption: &str,
) -> Result<(), anyhow::Error> {
    use lettre::{
        transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor,
    };
    let creds = Credentials::new(username.to_string(), password.to_string());
    let transport: AsyncSmtpTransport<Tokio1Executor> = match encryption {
        "ssl" => AsyncSmtpTransport::<Tokio1Executor>::relay(host)
            .map_err(|e| anyhow::anyhow!("Failed to create SSL transport: {}", e))?
            .port(port as u16)
            .credentials(creds)
            .build(),
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|e| anyhow::anyhow!("Failed to create STARTTLS transport: {}", e))?
            .port(port as u16)
            .credentials(creds)
            .build(),
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port as u16)
            .credentials(creds)
            .build(),
        other => return Err(anyhow::anyhow!("Unknown encryption: {}", other)),
    };

    // `test_connection` performs HELO + auth (lettre internals) without sending.
    transport
        .test_connection()
        .await
        .map_err(|e| anyhow::anyhow!("SMTP connection / auth failed: {}", e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Step 3: Done
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "classic/signup_done.html")]
pub struct SignupDoneTemplate {
    pub email: String,
    pub csrf_token: String,
    pub csp_nonce: String,
    /// Same as the other steps' `current_step` — reserved for the shared
    /// progress component a future P2 refactor will extract.
    #[allow(dead_code)]
    pub current_step: u8,
}

#[derive(serde::Deserialize, Debug)]
pub struct SignupDoneForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// Render the summary + "Go to inbox" button.
pub async fn get_step3_done(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let Some(draft) = load_active_draft(&state, &headers).await else {
        return Ok(bounce_to_step1());
    };
    let Some(mailbox_id) = draft.mailbox_id else {
        return Ok(Redirect::to(SIGNUP_STEP1_PATH).into_response());
    };
    use crate::models::classic_signup_draft::SignupDraftStep;
    if draft.step() != SignupDraftStep::Done {
        // User jumped to Step 3 without completing Step 2 — send them back.
        return Ok(Redirect::to(SIGNUP_STEP2_PATH).into_response());
    }
    let email = load_mailbox_email(&state, mailbox_id).await.unwrap_or_default();

    let template = SignupDoneTemplate {
        email,
        csrf_token: draft.csrf_token.clone(),
        csp_nonce: csp_nonce.as_str().to_string(),
        current_step: 3,
    };
    render_with_draft_cookie(
        StatusCode::OK,
        template,
        Some(build_set_draft_cookie_header(&state.config.jwt.secret, draft.id)),
    )
}

/// POST Step 3 — create the real `classic_sessions` row, clear the draft
/// cookie, redirect to the inbox.
pub async fn post_step3_done(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<SignupDoneForm>,
) -> Result<Response, AppError> {
    let Some(draft) = load_active_draft(&state, &headers).await else {
        return Ok(bounce_to_step1());
    };
    let Some(mailbox_id) = draft.mailbox_id else {
        return Ok(Redirect::to(SIGNUP_STEP1_PATH).into_response());
    };
    if form.csrf.is_empty() || !validate_csrf_token(&form.csrf, &draft.csrf_token) {
        return Ok(bounce_to_step1());
    }

    let (ip, ua) = extract_audit_fields(&headers);
    let established = create_session_and_cookie(&state, mailbox_id, ip.as_deref(), ua.as_deref()).await?;

    // Delete the draft row so the cookie clear isn't pointing at a live
    // hanging row.
    let _ = ClassicSignupDraft::delete(&state.db, draft.id).await;

    let mut resp = Redirect::to(INBOX_PATH).into_response();
    resp.headers_mut()
        .append(header::SET_COOKIE, established.set_cookie);
    if let Ok(hv) = HeaderValue::from_str(&build_clear_draft_cookie_header()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    tracing::info!(
        mailbox_id = ?mailbox_id,
        draft_id = ?draft.id,
        "classic signup step 3 — session established, draft cleared"
    );
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-jwt-secret-for-unit-tests-only";

    // ----- cookie helpers -----

    #[test]
    fn draft_cookie_value_has_uuid_dot_sig_shape() {
        let id = Uuid::new_v4();
        let v = build_draft_cookie_value(TEST_SECRET, id);
        let (left, right) = v.split_once('.').expect("cookie must be `<id>.<sig>`");
        assert_eq!(left, id.as_simple().to_string());
        // base64url no-pad over 32 bytes = 43 chars
        assert_eq!(right.len(), 43);
        for c in right.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '-' || c == '_',
                "non-URL-safe-base64 char {c:?}"
            );
        }
    }

    #[test]
    fn draft_signature_verifies_only_with_correct_secret() {
        let id = Uuid::new_v4();
        let v = build_draft_cookie_value(TEST_SECRET, id);
        let (_id, sig) = v.split_once('.').unwrap();
        assert!(verify_draft_signature(TEST_SECRET, id, sig));
        assert!(!verify_draft_signature("other-secret", id, sig));
    }

    #[test]
    fn draft_signature_verifies_only_against_correct_id() {
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let v = build_draft_cookie_value(TEST_SECRET, id);
        let (_, sig) = v.split_once('.').unwrap();
        assert!(verify_draft_signature(TEST_SECRET, id, sig));
        assert!(!verify_draft_signature(TEST_SECRET, other, sig));
    }

    #[test]
    fn set_cookie_header_has_strict_attributes() {
        let id = Uuid::new_v4();
        let h = build_set_draft_cookie_header(TEST_SECRET, id);
        assert!(h.contains(SIGNUP_DRAFT_COOKIE));
        assert!(h.contains("HttpOnly"));
        assert!(h.contains("Secure"));
        assert!(h.contains("SameSite=Strict"));
        assert!(h.contains("Path=/classic/signup"));
        assert!(h.contains("Max-Age=1800"));
    }

    #[test]
    fn clear_cookie_uses_max_age_zero() {
        let h = build_clear_draft_cookie_header();
        assert!(h.contains("Max-Age=0"));
        assert!(h.contains("Path=/classic/signup"));
        assert!(h.contains("HttpOnly") && h.contains("SameSite=Strict"));
    }

    #[test]
    fn extract_draft_cookie_pulls_out_id_and_sig() {
        let mut headers = HeaderMap::new();
        let id = Uuid::new_v4();
        let v = build_draft_cookie_value(TEST_SECRET, id);
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("other=1; {SIGNUP_DRAFT_COOKIE}={v}; baz=2"))
                .unwrap(),
        );
        let (parsed_id, parsed_sig) = extract_draft_cookie(&headers).expect("cookie present");
        assert_eq!(parsed_id, id);
        assert!(verify_draft_signature(TEST_SECRET, parsed_id, &parsed_sig));
    }

    #[test]
    fn extract_draft_cookie_returns_none_when_malformed() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("tasmail_classic_signup_draft=no-dot-here"),
        );
        assert!(extract_draft_cookie(&headers).is_none());
    }

    #[test]
    fn extract_draft_cookie_returns_none_when_absent() {
        let headers = HeaderMap::new();
        assert!(extract_draft_cookie(&headers).is_none());

        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("other=cookie"));
        assert!(extract_draft_cookie(&h).is_none());
    }

    // ----- route path constants are stable -----

    #[test]
    fn route_constants_live_under_classic_signup() {
        assert_eq!(SIGNUP_STEP1_PATH, "/classic/signup");
        assert_eq!(SIGNUP_STEP2_PATH, "/classic/signup/imap");
        assert_eq!(SIGNUP_STEP3_PATH, "/classic/signup/done");
    }

    // ----- helpers -----

    #[test]
    fn pick_config_name_prefers_picked_then_fallback() {
        assert_eq!(pick_config_name("Gmail", "fallback"), "Gmail");
        assert_eq!(pick_config_name("  Outlook  ", "fallback"), "Outlook");
        assert_eq!(pick_config_name("", "fallback"), "fallback");
        assert_eq!(pick_config_name("   ", "fallback"), "fallback");
    }

    #[test]
    fn load_presets_returns_known_providers() {
        let presets = load_presets();
        assert!(presets.iter().any(|p| p.name == "Gmail"));
        assert!(presets.iter().any(|p| p.name == "Outlook / Hotmail"));
        assert!(presets.iter().any(|p| p.name == "Zoho Mail"));
        for p in &presets {
            assert!(!p.imap_host.is_empty(), "preset {} missing IMAP host", p.name);
            assert!(p.imap_port > 0, "preset {} missing IMAP port", p.name);
            assert!(!p.smtp_host.is_empty(), "preset {} missing SMTP host", p.name);
            assert!(p.smtp_port > 0, "preset {} missing SMTP port", p.name);
        }
    }

    // ----- template smoke tests -----

    fn step1_template() -> SignupAccountTemplate {
        SignupAccountTemplate {
            email: "alice@example.com".into(),
            display_name: "Alice".into(),
            error: None,
            csrf_token: "fixed-csrf-token-for-tests".into(),
            csp_nonce: "fixed-nonce-for-tests".into(),
            current_step: 1,
        }
    }

    #[test]
    fn step1_template_renders_form_action_and_fields() {
        let body = step1_template().render().expect("renders");
        assert!(body.contains("action=\"/classic/signup\""), "form action missing");
        assert!(body.contains("method=\"post\""));
        assert!(body.contains("name=\"email\"") && body.contains("type=\"email\""));
        assert!(body.contains("name=\"password\"") && body.contains("type=\"password\""));
        assert!(body.contains("name=\"display_name\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("fixed-csrf-token-for-tests"));
        assert!(body.contains("value=\"alice@example.com\""));
        assert!(body.contains("value=\"Alice\""));
    }

    #[test]
    fn step1_template_inherits_base_and_has_no_scripts() {
        let body = step1_template().render().expect("renders");
        assert!(body.contains("<!DOCTYPE html>"));
        assert!(body.contains("class=\"skip-link\""));
        assert!(body.contains("<main id=\"main\""));
        assert!(body.contains("<style nonce=\"fixed-nonce-for-tests\">"));
        assert!(!body.contains("<script"), "no-JS rule: must have zero <script> tags");
    }

    #[test]
    fn step1_template_escapes_hostile_email() {
        let mut t = step1_template();
        t.email = "\"><script>alert(1)</script>".into();
        let body = t.render().expect("renders");
        assert!(!body.contains("\"><script>alert(1)</script>"),
                "raw <script> leaked into input value attribute");
    }

    #[test]
    fn step1_template_shows_error_in_alert_role() {
        let mut t = step1_template();
        t.error = Some("Password too short.".into());
        let body = t.render().expect("renders");
        assert!(body.contains("role=\"alert\""));
        assert!(body.contains("Password too short."));
    }

    fn step2_template() -> SignupServersTemplate {
        SignupServersTemplate {
            email: "alice@example.com".into(),
            presets: load_presets(),
            picked_name: "Gmail".into(),
            picked_hint: "Use a Google App Password, not your account password.".into(),
            imap_host: "imap.gmail.com".into(),
            imap_port: "993".into(),
            imap_username: "alice@example.com".into(),
            imap_password: "".into(),
            imap_encryption: "ssl".into(),
            smtp_host: "smtp.gmail.com".into(),
            smtp_port: "587".into(),
            smtp_username: "alice@example.com".into(),
            smtp_password: "".into(),
            smtp_encryption: "starttls".into(),
            imap_error: None,
            smtp_error: None,
            error: None,
            csrf_token: "fixed-csrf-token-for-tests".into(),
            csp_nonce: "fixed-nonce-for-tests".into(),
            current_step: 2,
        }
    }

    #[test]
    fn step2_template_renders_both_server_sections_and_presets() {
        let body = step2_template().render().expect("renders");
        assert!(body.contains("action=\"/classic/signup/imap\""));
        assert!(body.contains("name=\"imap_host\""));
        assert!(body.contains("name=\"imap_port\""));
        assert!(body.contains("name=\"imap_username\""));
        assert!(body.contains("name=\"imap_password\""));
        assert!(body.contains("name=\"smtp_host\""));
        assert!(body.contains("name=\"smtp_port\""));
        assert!(body.contains("name=\"smtp_username\""));
        assert!(body.contains("name=\"smtp_password\""));
        // Preset links (GET ?preset=…) so the user can swap presets without JS.
        assert!(body.contains("?preset=Gmail"));
        assert!(body.contains("?preset=Zoho%20Mail") || body.contains("?preset=Zoho Mail"));
    }

    #[test]
    fn step2_template_shows_section_specific_errors() {
        let mut t = step2_template();
        t.imap_error = Some("Could not authenticate IMAP.".into());
        t.smtp_error = Some("SMTP STARTTLS refused.".into());
        // Plain-ASCII copy so the assertions can avoid worrying about which
        // characters Askama auto-escapes (it escapes apostrophes / quotes).
        t.error = Some("Connection refused; fix and retry.".into());
        let body = t.render().expect("renders");
        assert!(body.contains("Could not authenticate IMAP."));
        assert!(body.contains("SMTP STARTTLS refused."));
        assert!(body.contains("Connection refused; fix and retry."));
    }

    #[test]
    fn step2_template_never_reflects_password_back() {
        let mut t = step2_template();
        t.imap_password = "should-not-leak".into();
        t.smtp_password = "also-secret".into();
        let body = t.render().expect("renders");
        assert!(
            !body.contains("should-not-leak"),
            "imap password leaked into rendered form"
        );
        assert!(
            !body.contains("also-secret"),
            "smtp password leaked into rendered form"
        );
    }

    #[test]
    fn step2_template_inherits_base_and_has_no_scripts() {
        let body = step2_template().render().expect("renders");
        assert!(body.contains("<!DOCTYPE html>"));
        assert!(body.contains("<style nonce=\"fixed-nonce-for-tests\">"));
        assert!(!body.contains("<script"));
    }

    fn step3_template() -> SignupDoneTemplate {
        SignupDoneTemplate {
            email: "alice@example.com".into(),
            csrf_token: "fixed-csrf-token-for-tests".into(),
            csp_nonce: "fixed-nonce-for-tests".into(),
            current_step: 3,
        }
    }

    #[test]
    fn step3_template_has_continue_form_to_inbox() {
        let body = step3_template().render().expect("renders");
        assert!(body.contains("action=\"/classic/signup/done\""));
        assert!(body.contains("method=\"post\""));
        assert!(body.contains("name=\"_csrf\""));
        assert!(body.contains("alice@example.com"));
        assert!(!body.contains("<script"));
    }

    #[test]
    fn route_paths_never_collide_with_login_or_inbox() {
        // Sanity: a future rename can't silently shadow the existing login /
        // inbox paths that the wizard redirects to.
        assert_ne!(SIGNUP_STEP1_PATH, super::super::auth::LOGIN_PATH);
        assert_ne!(SIGNUP_STEP1_PATH, INBOX_PATH);
        assert_ne!(SIGNUP_STEP2_PATH, INBOX_PATH);
        assert_ne!(SIGNUP_STEP3_PATH, INBOX_PATH);
    }

    #[test]
    fn min_password_length_matches_spa_signup() {
        // The SPA signup (handlers::auth::signup) enforces an 8-char minimum.
        // Locking it down here keeps both surfaces in sync — a drive-by edit
        // that loosens this would diverge the two paths.
        assert_eq!(MIN_PASSWORD_LEN, 8);
    }
}
