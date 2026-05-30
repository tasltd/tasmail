// TMAIL-167 / TMAIL-305: managed-mailbox provisioning endpoint.
//
// When the operator has installed Postfix + Dovecot and toggled the
// `dns_mx_onboarding_enabled` feature flag on, signed-up users can call this
// endpoint to provision a real mailbox on the managed mail server. The wizard's
// "Get a new mailbox on this server" tile drives this flow.
//
// PRODUCTION-GRADE GATING (fail-closed at every step):
//   1. Feature flag `dns_mx_onboarding_enabled` must be enabled.
//   2. The TASMAIL_MANAGED_DOMAIN env var must be set (defines the domain we provision into).
//   3. The TASMAIL_MANAGED_DOVECOT_HOST env var must point at the Dovecot server.
//   4. The SSH bridge to run `doveadm user add` / `doveadm pw` must be configured AND implemented.
//      Until both are true, the endpoint returns 503 — it MUST NOT write an
//      imap_configurations row with a placeholder password (TMAIL-305 regression: the
//      previous stub wrote literally "REPLACE_ME_WITH_DOVEADM_GENERATED_PASSWORD" and
//      told the user their mailbox was ready, but login was impossible).
//
// If any of those is missing the endpoint returns 503 with a descriptive message
// — same pattern as the BYOK code paths use when the user hasn't completed onboarding.

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::feature_flags;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ProvisionRequest {
    /// Local part of the desired email address (e.g. "alice" → alice@managed-domain).
    pub local_part: String,
}

#[derive(Debug, Serialize)]
pub struct ProvisionResponse {
    pub email: String,
    pub imap_host: String,
    pub imap_port: i32,
    pub imap_config_id: Uuid,
}

const LOCAL_PART_RE: &str = r"^[a-z0-9._-]{1,64}$";

/// POST /api/mailbox/provision
pub async fn provision_managed_mailbox(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<ProvisionRequest>,
) -> Result<(StatusCode, Json<ProvisionResponse>), AppError> {
    // -------- gate 1: feature flag --------
    if !feature_flags::is_enabled(&state, "dns_mx_onboarding_enabled").await {
        return Err(AppError::ServiceUnavailable(
            "Managed-mailbox onboarding is disabled. Use the BYOK path or ask your operator to enable dns_mx_onboarding_enabled.".into()
        ));
    }

    // -------- gate 2: managed domain configured --------
    let managed_domain = std::env::var("TASMAIL_MANAGED_DOMAIN").ok().filter(|s| !s.is_empty());
    let managed_dovecot = std::env::var("TASMAIL_MANAGED_DOVECOT_HOST").ok().filter(|s| !s.is_empty());
    let (managed_domain, managed_dovecot) = match (managed_domain, managed_dovecot) {
        (Some(d), Some(h)) => (d, h),
        _ => return Err(AppError::ServiceUnavailable(
            "Managed mail server is not configured (set TASMAIL_MANAGED_DOMAIN and TASMAIL_MANAGED_DOVECOT_HOST). \
             See docs/DNS-MX-ONBOARDING.md.".into()
        )),
    };

    // -------- input validation --------
    let local = body.local_part.trim().to_lowercase();
    let re = regex::Regex::new(LOCAL_PART_RE).expect("regex compiles");
    if !re.is_match(&local) {
        return Err(AppError::BadRequest(
            "local_part must be lowercase letters, digits, '.', '_', or '-' (1–64 chars)".into()
        ));
    }
    let new_email = format!("{}@{}", local, managed_domain);

    // -------- gate 4: doveadm SSH bridge (NOT YET IMPLEMENTED — fail-closed) --------
    // Fix (TMAIL-305): the previous build wrote an imap_configurations row whose
    // encrypted password was literally "REPLACE_ME_WITH_DOVEADM_GENERATED_PASSWORD",
    // then returned 201 — which told the user their mailbox was provisioned but
    // guaranteed they could not log in. Return 503 until the bridge is wired.
    //
    // When implementing the bridge, do all of the following BEFORE writing the
    // imap_configurations row:
    //   * ssh <managed_user>@<managed_dovecot> doveadm user add <new_email>
    //   * ssh <managed_user>@<managed_dovecot> doveadm pw -p <random 32-char password>
    //   * persist the freshly generated password (encrypted) on the imap_configurations row
    //   * verify the new user via `doveadm auth test`
    // Only after every step succeeds should the row be inserted with the real
    // generated password and the endpoint return 201.
    tracing::warn!(
        "TMAIL-305: refusing to provision {} on {} — doveadm SSH bridge is not implemented; returning 503 instead of writing a placeholder credential",
        new_email, managed_dovecot
    );
    Err(AppError::ServiceUnavailable(
        "Managed-mailbox provisioning is not wired yet — the doveadm SSH bridge is not implemented. \
         Use the BYOK path (attach your own IMAP/SMTP server) for now. Track TMAIL-305 for status."
            .into(),
    ))
}
