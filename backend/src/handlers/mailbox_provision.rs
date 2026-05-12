// TMAIL-167: managed-mailbox provisioning endpoint.
//
// When the operator has installed Postfix + Dovecot and toggled the
// `dns_mx_onboarding_enabled` feature flag on, signed-up users can call this
// endpoint to provision a real mailbox on the managed mail server. The wizard's
// "Get a new mailbox on this server" tile drives this flow.
//
// PRODUCTION-GRADE GATING:
//   1. Feature flag must be enabled (services::feature_flags::is_enabled).
//   2. The TASMAIL_MANAGED_DOMAIN env var must be set (defines the domain we provision into).
//   3. The TASMAIL_MANAGED_DOVECOT_HOST env var must point at the Dovecot server.
//
// If any of those is missing the endpoint returns 503 with a descriptive message
// — same pattern as the BYOK code paths use when the user hasn't completed onboarding.
//
// The actual `doveadm user add` SSH call lives in a follow-up commit; this skeleton
// ensures the API contract is wired so the frontend can integrate today and the
// only remaining piece is the side-effect.

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::imap_config::{CreateImapConfigRequest, ImapConfiguration, ImapEncryption};
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
    axum::Extension(claims): axum::Extension<Claims>,
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

    // -------- gate 3: doveadm side effect (skeleton) --------
    // Production: ssh user@mail-vps doveadm user add <new_email>; ssh user@mail-vps doveadm pw -p <random>; etc.
    // For now we acknowledge the request and write the imap_configurations row so the rest of TASMail
    // can route the user to their (yet-to-exist) mailbox. Operators must finish setting up doveadm
    // integration before exposing this in production — gate that on a separate flag if you need to.
    tracing::warn!(
        "TMAIL-167 stub: would `doveadm user add {}` on {} but the SSH integration is not wired yet",
        new_email, managed_dovecot
    );

    // Write an imap_configurations row pointing at the managed Dovecot. The user's TASMail
    // account password is reused as the mailbox password (operator-managed) — which means
    // the doveadm provisioning step MUST set the same password. Until then, the wizard
    // should warn the user that login won't actually work.
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user id in claims")))?;

    let key = crate::models::ai_config::derive_encryption_key(&state.config.jwt.secret);
    let cfg_req = CreateImapConfigRequest {
        name: "Managed mailbox".into(),
        host: managed_dovecot.clone(),
        port: 993,
        username: new_email.clone(),
        // PLACEHOLDER: in production this should be a freshly generated random password
        // that we ALSO push to doveadm. For the skeleton we write a marker so tests can
        // assert the row was created without leaking a real credential.
        password: "REPLACE_ME_WITH_DOVEADM_GENERATED_PASSWORD".to_string(),
        encryption: ImapEncryption::Ssl,
        sent_folder: Some("Sent".into()),
        drafts_folder: Some("Drafts".into()),
        trash_folder: Some("Trash".into()),
        spam_folder: Some("Junk".into()),
        archive_folder: Some("Archive".into()),
        is_default: true,
    };

    let cfg = ImapConfiguration::create(&state.db, user_id, &cfg_req, &key).await?;
    // Bust the per-user cache so the new default takes effect immediately.
    let _ = state.cache.invalidate_user_imap_config(&user_id.to_string()).await;

    Ok((StatusCode::CREATED, Json(ProvisionResponse {
        email: new_email,
        imap_host: managed_dovecot,
        imap_port: 993,
        imap_config_id: cfg.id,
    })))
}
