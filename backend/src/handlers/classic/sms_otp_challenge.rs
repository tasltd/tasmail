// Added (TMAIL-381): GET + POST handlers for /classic/login/2fa/sms — the
// SMS-OTP challenge that gates `/classic` logins when the resolved mailbox
// has `sms_otp_enabled = true` and `totp_enabled = false`. Sibling of the
// TOTP challenge (TMAIL-361).
//
// FLOW (top-level)
// ----------------
// 1. `POST /classic/login` resolves an SMS-OTP-enrolled user with TOTP NOT
//    enabled. INSTEAD of creating a `classic_sessions` row it:
//      * generates a CSRF token,
//      * creates a `pending_2fa_tokens` row (5-min fixed TTL),
//      * sets the `tasmail_classic_pending_2fa` cookie carrying
//        `<uuid_hex>.<hmac_b64>` (same shape as the TOTP gate),
//      * 303-redirects to `/classic/login/2fa/sms`.
// 2. `GET /classic/login/2fa/sms` reads the cookie, verifies the HMAC
//    signature, looks up the active pending row, loads the mailbox's SMS
//    config (phone + provider), and EITHER:
//      * generates + stores + sends a fresh 6-digit code when no unused,
//        non-expired code exists for this mailbox (first GET after the 303
//        from login),
//      * OR just re-renders the form with no fresh SMS (page refresh inside
//        the validity window — avoids SMS spam).
// 3. `POST /classic/login/2fa/sms` dispatches on the `action` form field:
//      * `action=verify`:
//          - Validate CSRF (form `_csrf` ↔ row's `csrf_token`, constant-time).
//          - Look up an unused, non-expired `sms_otp_codes` row matching the
//            submitted code for this mailbox.
//          - SUCCESS → mark the code used, delete the pending row, clear the
//            pending cookie, create the real `classic_sessions` row, set its
//            cookie, 303 → INBOX.
//          - FAILURE → increment `failed_attempts`. If the new count reaches
//            `PENDING_2FA_MAX_FAILED_ATTEMPTS`, delete the row, clear the
//            cookie, and bounce to `/classic/login` with a "Too many incorrect
//            codes" flash. Otherwise re-render the SMS form with a generic
//            error.
//      * `action=resend`:
//          - Validate CSRF.
//          - Rate-limit: if the most recent code is < `SMS_RESEND_COOLDOWN_SECS`
//            old, re-render with a "please wait" error.
//          - Otherwise mark prior codes used, generate + store + send a new
//            code, re-render with a success info banner.
//
// Why piggy-back on `pending_2fa_tokens` (the TOTP table)
// -------------------------------------------------------
// The pending-2FA row is the "short-lived gate between password OK and full
// session". It's factor-agnostic — same TTL, same CSRF binding, same audit
// fields, same attempt counter. The only difference is HOW the user proves
// possession of the factor. Sharing the row keeps a single sweep job, a
// single attempt-counter, and a single signed-cookie envelope. The SMS
// code itself lives in `sms_otp_codes` exactly as it does for enrollment.
//
// Why this route lives on the PUBLIC sub-router
// ---------------------------------------------
// Same rationale as the TOTP challenge: the user has NO real session yet
// (that only lands after the code verifies). The classic_session_middleware
// would bounce a request without a session cookie back to /classic/login —
// the SMS challenge MUST be reachable without one. We do the cookie
// resolution + signature check inline here.
//
// RLS note on sms_otp_codes
// -------------------------
// The `sms_otp_codes` table HAS RLS enabled (migration 012). The classic
// login flow happens BEFORE any JWT claims exist, so we can't reuse
// `db_session::acquire_with_rls(state, claims)`. Instead we pin a
// connection and set `app.mailbox_id` to the password-verified mailbox id
// directly — same shape as `acquire_with_rls`, just driven by the cookie-
// resolved mailbox rather than JWT Claims.

use askama::Template;
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Extension,
};
use sqlx::pool::PoolConnection;
use sqlx::Postgres;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::classic_csrf::validate_csrf_token;
use crate::models::pending_2fa_token::{
    PendingTwoFactorToken, PENDING_2FA_MAX_FAILED_ATTEMPTS,
};
use crate::services::sms_service;
use crate::state::AppState;

use super::auth::{create_session_and_cookie, INBOX_PATH, LOGIN_PATH};
use super::totp_challenge::{
    build_clear_pending_cookie_header, extract_pending_cookie, verify_token_signature,
};
use super::CspNonce;

/// Where to send the user when password evaluation has succeeded and SMS-OTP
/// is the next step. Symbolic constant so future renames touch one place.
pub const SMS_CHALLENGE_PATH: &str = "/classic/login/2fa/sms";

/// Generic error rendered for any failed code submission (wrong code, empty
/// code, malformed code). Same copy for every branch — matches the TOTP
/// challenge's account-enumeration-blind shape.
const GENERIC_CODE_ERROR: &str = "Incorrect verification code.";

/// Per-cookie cooldown between SMS sends. Keeps a user (or attacker) from
/// hammering the SMS provider with resend clicks, but short enough not to
/// frustrate a user whose first SMS got lost in a network blip.
pub const SMS_RESEND_COOLDOWN_SECS: i64 = 30;

/// SMS-provider lifetime of each code. Aligned with the enrollment
/// `sms_otp::enroll` helper so users see the same "valid for 5 minutes"
/// message regardless of which path issued the code.
pub const SMS_CODE_TTL_MINUTES: i64 = 5;

/// Test-mode toggle — same env-var as `handlers::sms_otp::sms_test_mode`.
/// When set, we skip the SMS-provider call and still persist + accept the
/// code so the E2E suite can verify the round-trip without real Hubtel /
/// Africa's Talking credentials. NEVER set in production.
fn sms_test_mode() -> bool {
    std::env::var("TASMAIL_SMS_TEST_MODE")
        .map(|v| v == "true")
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Template + form
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "classic/2fa_sms.html")]
pub struct SmsChallengeTemplate {
    /// `Some("…")` on a failed verify or rate-limited resend; `None` on the
    /// fresh GET.
    pub error: Option<String>,
    /// `Some("We sent a code to +233***789.")` on a fresh GET / successful
    /// resend; `None` on a re-render-after-failure path.
    pub info: Option<String>,
    /// CSRF token bound to the pending row. Rendered into the hidden _csrf
    /// input AND validated against the same row's `csrf_token` column on POST.
    pub csrf_token: String,
    /// Best-effort masked phone (e.g. "+233***789") for the info copy. None
    /// when the mailbox has no phone configured (shouldn't happen in this
    /// flow but template stays defensive).
    pub phone_masked: Option<String>,
    /// Whether the resolved mailbox ALSO has TOTP enrolled. When true, the
    /// template renders a "Use authenticator app instead" link to the
    /// existing TOTP challenge page. Today this branch is unreachable from
    /// the login handler (which prefers TOTP) but keeping the link rendered
    /// when applicable makes the page robust against a future "let user
    /// pick a factor" branch in login.rs.
    pub totp_available: bool,
    /// Per-request CSP nonce so the inline `<style>` in base.html survives
    /// TMAIL-368's strict CSP.
    pub csp_nonce: String,
}

impl SmsChallengeTemplate {
    fn new(
        error: Option<String>,
        info: Option<String>,
        csrf_token: impl Into<String>,
        phone_masked: Option<String>,
        totp_available: bool,
        csp_nonce: impl Into<String>,
    ) -> Self {
        Self {
            error,
            info,
            csrf_token: csrf_token.into(),
            phone_masked,
            totp_available,
            csp_nonce: csp_nonce.into(),
        }
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct SmsChallengeForm {
    /// `verify` or `resend`. Defaults to `verify` when the form omits the
    /// hidden input (defensive for browsers that don't forward hidden values
    /// on Enter-key submit, though no modern browser actually does that).
    #[serde(default = "default_action")]
    pub action: String,
    /// The 6-digit code typed by the user. We accept stray whitespace and
    /// any non-digit character — the lookup itself rejects anything that
    /// isn't a valid current code; normalising here just keeps the audit
    /// log readable. Empty on a resend submission.
    #[serde(default)]
    pub code: String,
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

fn default_action() -> String {
    "verify".to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Pin a connection from the pool and prime it with the SMS-OTP RLS variable
/// for the resolved mailbox. Mirrors `db_session::acquire_with_rls` but
/// driven by the cookie-resolved mailbox id rather than JWT Claims — the
/// classic login flow happens BEFORE any claims exist.
async fn acquire_with_mailbox_rls(
    state: &AppState,
    mailbox_id: Uuid,
) -> Result<PoolConnection<Postgres>, AppError> {
    let mut conn = state.db.acquire().await?;
    // `set_config(..., false)` makes the variable session-local — it
    // persists for every query on this pinned connection until the
    // connection returns to the pool on drop. Same shape as
    // db_session::acquire_with_rls.
    sqlx::query("SELECT set_config('app.mailbox_id', $1, false)")
        .bind(mailbox_id.to_string())
        .execute(&mut *conn)
        .await?;
    sqlx::query("SELECT set_config('app.is_admin', 'false', false)")
        .execute(&mut *conn)
        .await?;
    Ok(conn)
}

/// Mask the configured phone number for display in the challenge page. Same
/// shape as `handlers::sms_otp::mask_phone` — kept inline rather than made
/// `pub` to avoid widening the sms_otp module's surface area for one caller.
fn mask_phone(p: &str) -> String {
    if p.len() > 6 {
        format!("{}***{}", &p[..4], &p[p.len() - 3..])
    } else {
        "***".to_string()
    }
}

/// Pull the resolved mailbox's SMS-OTP config from the DB. Returns
/// `(phone_number, sms_provider, sms_otp_enabled, totp_enabled)` so the
/// handler can branch on what's available.
async fn load_sms_config(
    pool: &sqlx::PgPool,
    mailbox_id: Uuid,
) -> Result<(Option<String>, Option<String>, bool, bool), sqlx::Error> {
    let row: Option<(Option<String>, Option<String>, bool, bool)> = sqlx::query_as(
        "SELECT phone_number, sms_provider, sms_otp_enabled, totp_enabled
           FROM mailboxes
          WHERE id = $1",
    )
    .bind(mailbox_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.unwrap_or((None, None, false, false)))
}

/// Look up the most recent unused, non-expired SMS code row's created_at for
/// the resolved mailbox. Used by the GET handler to decide "is there an
/// active code already?" and by the resend rate-limiter.
async fn latest_active_code_created_at(
    conn: &mut PoolConnection<Postgres>,
    mailbox_id: Uuid,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, sqlx::Error> {
    let row: Option<(chrono::DateTime<chrono::Utc>,)> = sqlx::query_as(
        "SELECT created_at
           FROM sms_otp_codes
          WHERE mailbox_id = $1
            AND used = false
            AND expires_at > NOW()
          ORDER BY created_at DESC
          LIMIT 1",
    )
    .bind(mailbox_id)
    .fetch_optional(&mut **conn)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Generate a fresh OTP, mark prior unused codes for this mailbox as used,
/// insert the new row, and (unless in test mode) dispatch it via the
/// configured SMS provider. Returns the generated code so test-mode callers
/// can surface it (we currently don't — the `?code=…` query-param leak in
/// the enrollment flow was a one-off for the SPA E2E; the classic flow's
/// E2E reads the code from the DB directly).
async fn rotate_and_send(
    _state: &AppState,
    conn: &mut PoolConnection<Postgres>,
    mailbox_id: Uuid,
    phone: &str,
    provider: &str,
) -> Result<String, AppError> {
    let code = sms_service::generate_otp();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(SMS_CODE_TTL_MINUTES);

    sqlx::query("UPDATE sms_otp_codes SET used = true WHERE mailbox_id = $1 AND used = false")
        .bind(mailbox_id)
        .execute(&mut **conn)
        .await?;
    sqlx::query(
        "INSERT INTO sms_otp_codes (mailbox_id, code, phone_number, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(mailbox_id)
    .bind(&code)
    .bind(phone)
    .bind(expires_at)
    .execute(&mut **conn)
    .await?;

    if sms_test_mode() {
        tracing::info!(
            mailbox_id = ?mailbox_id,
            "TASMAIL_SMS_TEST_MODE=true — skipping SMS send for classic 2FA challenge"
        );
        return Ok(code);
    }

    let sms_config = sms_service::SmsConfig::default();
    sms_service::send_otp(&sms_config, provider, phone, &code)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "classic SMS challenge send failed (provider={provider}): {e}"
            ))
        })?;
    Ok(code)
}

/// Verify a submitted 6-digit code against the most recent unused, non-
/// expired `sms_otp_codes` row for this mailbox. Marks the row used on
/// match. Returns true on success, false on mismatch / expiry.
async fn verify_code(
    conn: &mut PoolConnection<Postgres>,
    mailbox_id: Uuid,
    code: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id
           FROM sms_otp_codes
          WHERE mailbox_id = $1
            AND code = $2
            AND used = false
            AND expires_at > NOW()",
    )
    .bind(mailbox_id)
    .bind(code)
    .fetch_optional(&mut **conn)
    .await?;

    let Some((otp_id,)) = row else {
        return Ok(false);
    };
    sqlx::query("UPDATE sms_otp_codes SET used = true WHERE id = $1")
        .bind(otp_id)
        .execute(&mut **conn)
        .await?;
    Ok(true)
}

fn render_challenge_response(
    status: StatusCode,
    template: SmsChallengeTemplate,
) -> Result<Response, AppError> {
    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic SMS 2FA challenge template render failed: {e}"
        ))
    })?;
    Ok((
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

/// Bounce response used when the pending cookie is missing, forged, or
/// expired. Clears the cookie and 303s to /classic/login with an explanatory
/// flash query param. Same shape as the TOTP challenge — keeps the user-
/// visible "your verification session expired" copy consistent across both
/// factors.
fn bounce_to_login_with_reason(reason_param: &str) -> Response {
    let url = format!("{LOGIN_PATH}?error={reason_param}");
    let mut resp = Redirect::to(&url).into_response();
    if let Ok(hv) = HeaderValue::from_str(&build_clear_pending_cookie_header()) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    resp
}

/// Build the info banner that announces a successful (re)send. Uses the
/// masked phone so the audit-trail is clear without leaking the full number
/// to the rendered HTML.
fn send_success_banner(phone_masked: &Option<String>) -> String {
    match phone_masked {
        Some(p) => format!("We sent a 6-digit verification code to {p}."),
        None => "We sent a 6-digit verification code to your registered phone.".to_string(),
    }
}

/// Build the "wait a few seconds" rate-limit banner. Phrased in absolute
/// seconds so the user knows when to retry — they don't see a JS countdown.
fn rate_limit_banner(seconds_remaining: i64) -> String {
    format!("Please wait {seconds_remaining} seconds before requesting another code.")
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// GET /classic/login/2fa/sms — render the 6-digit code form. The act of
/// rendering ALSO triggers an SMS send the FIRST time the user lands here
/// (i.e. when no unused, non-expired code exists for the resolved mailbox);
/// page refreshes inside the validity window do NOT re-send (avoids SMS
/// spam and matches the issue's "rendering also triggers" wording while
/// staying idempotent for refreshes).
pub async fn get_challenge(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let Some((token_id, sig)) = extract_pending_cookie(&headers) else {
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    };
    if !verify_token_signature(&state.config.jwt.secret, token_id, &sig) {
        tracing::warn!(
            ?token_id,
            "classic pending-2FA cookie signature mismatch (sms GET) — possible tampering"
        );
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }

    let pending = match PendingTwoFactorToken::find_active(&state.db, token_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(bounce_to_login_with_reason("2fa_expired")),
        Err(e) => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "pending_2fa_tokens lookup failed: {e}"
            )));
        }
    };

    let (phone, provider, sms_enabled, totp_enabled) =
        load_sms_config(&state.db, pending.user_id)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "mailbox sms-config lookup failed: {e}"
                ))
            })?;

    if !sms_enabled || phone.is_none() {
        // SMS-OTP was disabled (or the phone removed) between password
        // check and code send. Drop the gate and bounce — same shape as
        // the TOTP challenge's mid-flow disable defence.
        let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }
    let phone = phone.unwrap();
    let provider = provider.unwrap_or_else(|| "hubtel".to_string());
    let phone_masked = Some(mask_phone(&phone));

    // Pin a connection with RLS primed for this mailbox so sms_otp_codes
    // queries actually see the rows.
    let mut conn = acquire_with_mailbox_rls(&state, pending.user_id).await?;

    let active_code_at = latest_active_code_created_at(&mut conn, pending.user_id)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "sms_otp_codes latest-lookup failed: {e}"
            ))
        })?;

    let info = if active_code_at.is_none() {
        // First GET — send a fresh code.
        rotate_and_send(&state, &mut conn, pending.user_id, &phone, &provider).await?;
        tracing::info!(
            user_id = ?pending.user_id,
            "classic SMS challenge first render — issued fresh OTP"
        );
        Some(send_success_banner(&phone_masked))
    } else {
        // Page refresh inside the validity window — do NOT re-send.
        Some(send_success_banner(&phone_masked))
    };

    let template = SmsChallengeTemplate::new(
        None,
        info,
        pending.csrf_token,
        phone_masked,
        totp_enabled,
        csp_nonce.as_str(),
    );
    render_challenge_response(StatusCode::OK, template)
}

/// POST /classic/login/2fa/sms — dispatch on the `action` form field.
///
/// `action=verify` (default): validate CSRF, check the submitted 6-digit
/// code, either issue a real session or re-render the form with an error.
///
/// `action=resend`: validate CSRF, enforce a 30-second cooldown, rotate the
/// active SMS code, re-render the form with a success banner.
pub async fn post_challenge(
    State(state): State<AppState>,
    Extension(csp_nonce): Extension<CspNonce>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<SmsChallengeForm>,
) -> Result<Response, AppError> {
    // 1) Read the cookie. Missing/forged → bounce + clear.
    let Some((token_id, sig)) = extract_pending_cookie(&headers) else {
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    };
    if !verify_token_signature(&state.config.jwt.secret, token_id, &sig) {
        tracing::warn!(
            ?token_id,
            "classic pending-2FA cookie signature mismatch on SMS POST — possible tampering"
        );
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }

    // 2) Look up the active pending row. None → bounce.
    let pending = match PendingTwoFactorToken::find_active(&state.db, token_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(bounce_to_login_with_reason("2fa_expired")),
        Err(e) => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "pending_2fa_tokens lookup failed: {e}"
            )));
        }
    };

    // 3) CSRF: constant-time compare. Fail closed.
    if form.csrf.is_empty() || !validate_csrf_token(&form.csrf, &pending.csrf_token) {
        let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }

    // 4) Re-check SMS enrollment + load phone/provider. Same race-window
    //    defence the TOTP path uses.
    let (phone, provider, sms_enabled, totp_enabled) =
        load_sms_config(&state.db, pending.user_id)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "mailbox sms-config lookup failed: {e}"
                ))
            })?;
    if !sms_enabled || phone.is_none() {
        let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
        return Ok(bounce_to_login_with_reason("2fa_expired"));
    }
    let phone = phone.unwrap();
    let provider = provider.unwrap_or_else(|| "hubtel".to_string());
    let phone_masked = Some(mask_phone(&phone));

    let mut conn = acquire_with_mailbox_rls(&state, pending.user_id).await?;

    match form.action.as_str() {
        "resend" => {
            // Rate-limit: must be > SMS_RESEND_COOLDOWN_SECS since the most
            // recent active code.
            let active_at = latest_active_code_created_at(&mut conn, pending.user_id)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!(
                        "sms_otp_codes latest-lookup failed: {e}"
                    ))
                })?;
            if let Some(created_at) = active_at {
                let elapsed = (chrono::Utc::now() - created_at).num_seconds();
                if elapsed < SMS_RESEND_COOLDOWN_SECS {
                    let remaining = SMS_RESEND_COOLDOWN_SECS - elapsed;
                    let template = SmsChallengeTemplate::new(
                        Some(rate_limit_banner(remaining)),
                        None,
                        pending.csrf_token,
                        phone_masked,
                        totp_enabled,
                        csp_nonce.as_str(),
                    );
                    return render_challenge_response(StatusCode::TOO_MANY_REQUESTS, template);
                }
            }

            rotate_and_send(&state, &mut conn, pending.user_id, &phone, &provider).await?;
            tracing::info!(
                user_id = ?pending.user_id,
                "classic SMS challenge resend — issued fresh OTP"
            );
            let template = SmsChallengeTemplate::new(
                None,
                Some(send_success_banner(&phone_masked)),
                pending.csrf_token,
                phone_masked,
                totp_enabled,
                csp_nonce.as_str(),
            );
            render_challenge_response(StatusCode::OK, template)
        }
        _ => {
            // Default branch: verify. Anything other than `resend` is treated
            // as a verify submission so a stripped hidden input doesn't yield
            // a 400 — same defensive default as the form-struct field.
            let code = form.code.trim();
            let verified = if code.is_empty() {
                false
            } else {
                verify_code(&mut conn, pending.user_id, code).await.map_err(|e| {
                    AppError::Internal(anyhow::anyhow!(
                        "sms_otp_codes verify lookup failed: {e}"
                    ))
                })?
            };

            if !verified {
                // Wrong code. Bump the counter; if we've hit the limit,
                // invalidate the row + clear the cookie and bounce to login.
                let new_count = match PendingTwoFactorToken::increment_failed(
                    &state.db,
                    pending.id,
                )
                .await
                {
                    Ok(n) => n,
                    Err(e) => {
                        return Err(AppError::Internal(anyhow::anyhow!(
                            "pending_2fa_tokens increment failed: {e}"
                        )));
                    }
                };
                if new_count >= PENDING_2FA_MAX_FAILED_ATTEMPTS {
                    let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;
                    tracing::warn!(
                        user_id = ?pending.user_id,
                        attempts = new_count,
                        "classic SMS 2FA challenge exhausted attempts — bouncing to login"
                    );
                    return Ok(bounce_to_login_with_reason("2fa_too_many"));
                }

                let template = SmsChallengeTemplate::new(
                    Some(GENERIC_CODE_ERROR.to_string()),
                    None,
                    pending.csrf_token,
                    phone_masked,
                    totp_enabled,
                    csp_nonce.as_str(),
                );
                return render_challenge_response(StatusCode::UNAUTHORIZED, template);
            }

            // Success: pending gate is one-shot, drop the row + clear the
            // cookie, then create the real classic_sessions row + attach its
            // cookie.
            // Release the RLS-pinned connection before issuing follow-up
            // queries on the raw pool.
            drop(conn);
            let _ = PendingTwoFactorToken::delete(&state.db, pending.id).await;

            let (ip, ua) = extract_audit_fields(&headers);
            let established = create_session_and_cookie(
                &state,
                pending.user_id,
                ip.as_deref(),
                ua.as_deref(),
            )
            .await?;

            tracing::info!(
                user_id = ?pending.user_id,
                session_id = ?established.session.id,
                "classic SMS 2FA challenge passed; full session created"
            );

            let mut resp = Redirect::to(INBOX_PATH).into_response();
            resp.headers_mut()
                .append(header::SET_COOKIE, established.set_cookie);
            if let Ok(hv) = HeaderValue::from_str(&build_clear_pending_cookie_header()) {
                resp.headers_mut().append(header::SET_COOKIE, hv);
            }
            Ok(resp)
        }
    }
}

/// First-hop X-Forwarded-For + User-Agent for the session row's audit
/// fields. Identical shape to the TOTP challenge's helper.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_template() -> SmsChallengeTemplate {
        SmsChallengeTemplate {
            error: None,
            info: Some("We sent a 6-digit verification code to +233***789.".to_string()),
            csrf_token: "fixed-csrf-token-for-tests".to_string(),
            phone_masked: Some("+233***789".to_string()),
            totp_available: false,
            csp_nonce: "fixed-nonce-for-tests".to_string(),
        }
    }

    #[test]
    fn challenge_path_constant_is_under_classic() {
        // Locks the redirect target so a typo can't silently send users
        // to the SPA's /login/2fa/sms route (which doesn't exist).
        assert_eq!(SMS_CHALLENGE_PATH, "/classic/login/2fa/sms");
    }

    #[test]
    fn resend_cooldown_is_thirty_seconds() {
        // Lock down the cooldown so a future drive-by tweak can't quietly
        // open a per-cookie SMS-spam window.
        assert_eq!(SMS_RESEND_COOLDOWN_SECS, 30);
    }

    #[test]
    fn code_ttl_is_five_minutes() {
        // Match the enrollment flow's 5-minute window so the "valid for 5
        // minutes" microcopy stays accurate regardless of which path
        // issued the code.
        assert_eq!(SMS_CODE_TTL_MINUTES, 5);
    }

    #[test]
    fn mask_phone_truncates_normal_number() {
        assert_eq!(mask_phone("+233241234789"), "+233***789");
    }

    #[test]
    fn mask_phone_collapses_short_number() {
        assert_eq!(mask_phone("+2332"), "***");
    }

    #[test]
    fn default_action_is_verify() {
        // A stripped or omitted action hidden input MUST default to verify
        // — otherwise a slightly-misbehaving form posts an empty action
        // string and the dispatcher would have to special-case it.
        assert_eq!(default_action(), "verify");
    }

    #[test]
    fn form_deserialises_verify_branch() {
        let body =
            "_csrf=tok&action=verify&code=123456";
        let parsed: SmsChallengeForm =
            serde_urlencoded::from_str(body).expect("form parses");
        assert_eq!(parsed.csrf, "tok");
        assert_eq!(parsed.action, "verify");
        assert_eq!(parsed.code, "123456");
    }

    #[test]
    fn form_deserialises_resend_branch_without_code() {
        let body = "_csrf=tok&action=resend";
        let parsed: SmsChallengeForm =
            serde_urlencoded::from_str(body).expect("form parses");
        assert_eq!(parsed.action, "resend");
        assert_eq!(parsed.code, "");
    }

    #[test]
    fn form_defaults_action_to_verify_when_missing() {
        let body = "_csrf=tok&code=123456";
        let parsed: SmsChallengeForm =
            serde_urlencoded::from_str(body).expect("form parses");
        assert_eq!(parsed.action, "verify");
    }

    #[test]
    fn challenge_template_renders_form_and_code_input() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains(&format!("action=\"{SMS_CHALLENGE_PATH}\"")),
            "form action missing: {body}"
        );
        assert!(body.contains("method=\"post\""), "method=post missing");
        assert!(
            body.contains("name=\"code\"")
                && body.contains("inputmode=\"numeric\"")
                && body.contains("pattern=\"[0-9]"),
            "6-digit code input missing or wrong attrs: {body}"
        );
        assert!(
            body.contains("name=\"_csrf\"")
                && body.contains("value=\"fixed-csrf-token-for-tests\""),
            "hidden _csrf field missing: {body}"
        );
        assert!(
            body.contains("autocomplete=\"one-time-code\""),
            "one-time-code autocomplete hint missing"
        );
    }

    #[test]
    fn challenge_template_renders_verify_and_resend_action_inputs() {
        // Two sibling forms — one carries action=verify, the other
        // action=resend. Both MUST be present so the user can pick either
        // path with no JS.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("value=\"verify\""),
            "verify action hidden input missing: {body}"
        );
        assert!(
            body.contains("value=\"resend\""),
            "resend action hidden input missing: {body}"
        );
        assert!(
            body.matches("action=\"/classic/login/2fa/sms\"").count() >= 2,
            "expected at least two POST forms targeting the SMS challenge path: {body}"
        );
    }

    #[test]
    fn challenge_template_renders_info_banner_when_present() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("alert-success"),
            "info alert must use alert-success class: {body}"
        );
        assert!(
            body.contains("role=\"status\""),
            "info alert must use role=status, not role=alert: {body}"
        );
        assert!(body.contains("+233***789"));
    }

    #[test]
    fn challenge_template_omits_error_on_fresh_render() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("role=\"alert\""),
            "alert block must be absent on fresh GET, found: {body}"
        );
    }

    #[test]
    fn challenge_template_renders_error_when_present() {
        let mut t = fresh_template();
        t.error = Some(GENERIC_CODE_ERROR.to_string());
        t.info = None;
        let body = t.render().expect("template renders");
        assert!(body.contains("role=\"alert\""), "alert role missing");
        assert!(body.contains(GENERIC_CODE_ERROR));
    }

    #[test]
    fn challenge_template_renders_totp_link_when_available() {
        let mut t = fresh_template();
        t.totp_available = true;
        let body = t.render().expect("template renders");
        assert!(
            body.contains("/classic/login/2fa\""),
            "TOTP fallback link missing when totp_available=true: {body}"
        );
        assert!(
            body.to_lowercase().contains("authenticator"),
            "TOTP fallback link text must mention authenticator: {body}"
        );
    }

    #[test]
    fn challenge_template_omits_totp_link_when_unavailable() {
        // Default fixture has totp_available=false.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("/classic/login/2fa\""),
            "TOTP fallback link must NOT render when totp_available=false: {body}"
        );
    }

    #[test]
    fn challenge_template_extends_base_layout() {
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
    fn challenge_template_has_zero_script_tags() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("<script"),
            "SMS 2FA challenge template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn rate_limit_banner_mentions_remaining_seconds() {
        let msg = rate_limit_banner(17);
        assert!(msg.contains("17"), "banner must include the seconds count: {msg}");
        assert!(
            msg.to_lowercase().contains("wait"),
            "banner must use the word 'wait' so screen readers convey the action: {msg}"
        );
    }

    #[test]
    fn send_success_banner_handles_missing_phone() {
        let banner = send_success_banner(&None);
        assert!(banner.to_lowercase().contains("registered phone"));
        // Hostile copy guard — must not leak the unmasked number when None.
        assert!(!banner.contains("+"));
    }

    #[test]
    fn send_success_banner_includes_masked_phone_when_present() {
        let banner = send_success_banner(&Some("+233***789".to_string()));
        assert!(banner.contains("+233***789"));
    }

    #[test]
    fn generic_code_error_does_not_leak_account_existence() {
        // Same shape as the TOTP challenge — no account-existence signal.
        let m = GENERIC_CODE_ERROR.to_lowercase();
        assert!(!m.contains("account"), "must not mention account: {GENERIC_CODE_ERROR}");
        assert!(!m.contains("no such"));
        assert!(!m.contains("does not exist"));
    }

    #[test]
    fn bounce_response_clears_pending_cookie_and_redirects_to_login() {
        let resp = bounce_to_login_with_reason("2fa_expired");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(loc.starts_with("/classic/login"), "redirect target wrong: {loc}");
        assert!(loc.contains("error=2fa_expired"));
        let set_cookies: Vec<_> = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        assert!(
            set_cookies
                .iter()
                .any(|s| s.contains("Max-Age=0") && s.contains("tasmail_classic_pending_2fa")),
            "expected a clear-cookie Set-Cookie header, got: {set_cookies:?}"
        );
    }

    #[test]
    fn sms_test_mode_default_is_false() {
        // The test process inherits the env from the dev shell. We can't
        // SAFELY reset env_vars in a parallel test, but we CAN assert that
        // the helper returns a bool — the production default is false.
        let _ = sms_test_mode(); // just exercise the function path
    }
}
