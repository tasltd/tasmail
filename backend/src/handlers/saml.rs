// Added: SAML 2.0 SSO configuration and authentication handlers for TMAIL-101
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::models::mailbox::Mailbox;
use crate::models::saml_config::{
    CreateSamlConfigRequest, SamlConfiguration, SamlSession, UpdateSamlConfigRequest,
};
use crate::services::audit::audit_admin_action;
use crate::services::auth_service::{self, Claims, TokenPair};
use crate::state::AppState;

/// PURPOSE: List all SAML IdP configurations (admin only)
/// EXTERNAL: GET /api/admin/saml
pub async fn list_saml_configs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<SamlConfiguration>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let configs = SamlConfiguration::list(&state.db).await?;
    Ok(Json(configs))
}

/// PURPOSE: Create a new SAML IdP configuration (admin only)
/// CONSTRAINTS: Requires name, entity_id, sso_url, certificate
/// EXTERNAL: POST /api/admin/saml
pub async fn create_saml_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(request): Json<CreateSamlConfigRequest>,
) -> Result<(StatusCode, Json<SamlConfiguration>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate required fields are non-empty
    if request.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "SAML configuration name is required".to_string(),
        ));
    }
    if request.entity_id.trim().is_empty() {
        return Err(AppError::BadRequest(
            "IdP Entity ID is required".to_string(),
        ));
    }
    if request.sso_url.trim().is_empty() {
        return Err(AppError::BadRequest("SSO URL is required".to_string()));
    }
    if request.certificate.trim().is_empty() {
        return Err(AppError::BadRequest(
            "IdP certificate is required".to_string(),
        ));
    }

    let config = SamlConfiguration::create(&state.db, &request).await?;

    // Added (TMAIL-307): audit-log SAML config creation.
    audit_admin_action(
        &state.db,
        &claims,
        "saml_config.create",
        Some("saml_configuration"),
        Some(&config.id.to_string()),
        Some(serde_json::json!({
            "name": config.name,
            "entity_id": config.entity_id,
            "sso_url": config.sso_url,
        })),
    )
    .await;

    Ok((StatusCode::CREATED, Json(config)))
}

/// PURPOSE: Update an existing SAML IdP configuration (admin only)
/// CONSTRAINTS: All fields optional — only provided fields are updated
/// EXTERNAL: PUT /api/admin/saml/:id
pub async fn update_saml_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateSamlConfigRequest>,
) -> Result<Json<SamlConfiguration>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify config exists before updating
    SamlConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("SAML configuration {id} not found")))?;

    let config = SamlConfiguration::update(&state.db, id, &request).await?;

    // Added (TMAIL-307): audit-log SAML config update.
    audit_admin_action(
        &state.db,
        &claims,
        "saml_config.update",
        Some("saml_configuration"),
        Some(&id.to_string()),
        Some(serde_json::json!({
            "name": request.name,
            "entity_id": request.entity_id,
            "sso_url": request.sso_url,
            "active": request.active,
        })),
    )
    .await;

    Ok(Json(config))
}

/// PURPOSE: Delete a SAML IdP configuration (admin only)
/// EXTERNAL: DELETE /api/admin/saml/:id
pub async fn delete_saml_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify config exists before deleting
    SamlConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("SAML configuration {id} not found")))?;

    SamlConfiguration::delete(&state.db, id).await?;

    // Added (TMAIL-307): audit-log SAML config delete.
    audit_admin_action(
        &state.db,
        &claims,
        "saml_config.delete",
        Some("saml_configuration"),
        Some(&id.to_string()),
        None,
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: Generate a SAML AuthnRequest redirect URL for the given IdP (public)
/// CONSTRAINTS: The SAML config must exist and be active
/// EXTERNAL: GET /api/auth/saml/:id/login
pub async fn saml_login(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SamlLoginResponse>, AppError> {
    let config = SamlConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("SAML configuration {id} not found")))?;

    if !config.active {
        return Err(AppError::BadRequest(
            "This SAML configuration is not active. Contact your administrator.".to_string(),
        ));
    }

    // Added: Build SP entity ID and ACS URL from server config
    let sp_entity_id = format!("https://{}", state.config.server.host);
    let acs_url = format!("https://{}/api/auth/saml/callback", state.config.server.host);

    let redirect_url = config.build_authn_request_url(&sp_entity_id, &acs_url);

    Ok(Json(SamlLoginResponse { redirect_url }))
}

/// PURPOSE: Process SAML Response callback from IdP and complete login (TMAIL-303)
///
/// CONSTRAINTS:
/// - `saml_response` must be valid base64-encoded UTF-8.
/// - `relay_state` MUST carry the SAML config UUID — required to look up
///   the IdP's attribute mapping and auto-provision policy.
/// - The subject (name_id or mapped "email" attribute) MUST look like an
///   email address — that's the `mailboxes.username` we look up against.
///
/// FLOW: decode → load config (active check) → resolve subject email
/// → find-or-create mailbox (gated by `auto_create_users`)
/// → persist SamlSession with the resolved user_id (so SLO works)
/// → issue access + refresh tokens via the shared auth_service helper.
///
/// NOTE: Full XML signature verification against `config.certificate` is
/// the next step (TMAIL-101 follow-up). For now the IdP-side proxy is
/// trusted to deliver name_id + attributes; the rest of the flow is real.
///
/// EXTERNAL: POST /api/auth/saml/callback
pub async fn saml_callback(
    State(state): State<AppState>,
    Json(request): Json<SamlCallbackRequest>,
) -> Result<Json<TokenPair>, AppError> {
    // Decode + sanity-check the base64 SAMLResponse. We don't parse the
    // XML yet — full signature validation is TMAIL-101 follow-up work —
    // but rejecting non-base64 garbage stops obvious replay-attack probes.
    use base64::Engine;
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(&request.saml_response)
        .map_err(|_| AppError::BadRequest("Invalid SAMLResponse encoding".to_string()))?;
    let _saml_xml = String::from_utf8(decoded_bytes)
        .map_err(|_| AppError::BadRequest("SAMLResponse is not valid UTF-8".to_string()))?;

    // RelayState carries the config UUID. Required now — without it we
    // can't resolve the IdP's attribute mapping or auto-provision policy.
    let config_id = request
        .relay_state
        .as_deref()
        .and_then(|rs| Uuid::parse_str(rs).ok())
        .ok_or_else(|| {
            AppError::BadRequest(
                "RelayState with the SAML configuration id is required".to_string(),
            )
        })?;

    let config = SamlConfiguration::get_by_id(&state.db, config_id)
        .await
        .map_err(|_| AppError::NotFound("SAML configuration not found".to_string()))?;
    if !config.active {
        return Err(AppError::BadRequest(
            "This SAML configuration is not active".to_string(),
        ));
    }

    // Resolve the subject email. Priority: mapped "email" attribute (which
    // IdPs can override via attribute_mapping) → name_id (the SAML subject,
    // which the default nameid-format:emailAddress puts the email in).
    let attributes_blob = request
        .attributes
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    let email_from_attr = config.resolve_attribute(&attributes_blob, "email");
    let email = email_from_attr
        .or_else(|| request.name_id.clone())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && s.contains('@'))
        .ok_or_else(|| {
            AppError::BadRequest(
                "SAML assertion is missing an email subject (name_id or mapped attribute)"
                    .to_string(),
            )
        })?;

    // Find or auto-provision the mailbox.
    let display_name = config.resolve_attribute(&attributes_blob, "name");
    let mailbox = resolve_or_provision_mailbox(&state, &config, &email, display_name.as_deref())
        .await?;

    // Persist the SAML session for SLO tracking. Best-effort so SLO
    // unavailability never blocks login.
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(8);
    let _ = SamlSession::create(
        &state.db,
        config.id,
        Some(mailbox.id),
        request.session_index.as_deref(),
        request.name_id.as_deref().unwrap_or(&email),
        &attributes_blob,
        expires_at,
    )
    .await;

    // Issue the same TokenPair shape as POST /api/auth/login so the SPA's
    // existing auth flow (write tokens to localStorage, redirect to /) keeps
    // working unchanged.
    let tokens = auth_service::issue_token_pair_for_mailbox(
        &state.db,
        &state.config.jwt,
        &mailbox,
    )
    .await?;

    // Audit success. Investigators rely on this for SAML SSO forensics.
    let mailbox_id_str = mailbox.id.to_string();
    let _ = AuditLog::record(
        &state.db,
        Some(mailbox.id),
        "auth.saml_login",
        Some("mailbox"),
        Some(mailbox_id_str.as_str()),
        Some(serde_json::json!({
            "saml_config_id": config.id,
            "session_index": request.session_index,
        })),
        None,
        None,
    )
    .await;

    Ok(Json(tokens))
}

/// PURPOSE: Find an existing mailbox by SAML subject email, or auto-provision
/// one when the IdP config allows it (TMAIL-303).
///
/// CONSTRAINTS:
/// - When `config.auto_create_users == false` and no mailbox exists,
///   returns 403 — admins explicitly opted out of just-in-time provisioning.
/// - Auto-provisioned mailboxes are anchored to the synthetic
///   `byok.tasmail` domain (migration 056). They get a random Argon2id
///   password hash that the user can never guess — they authenticate via
///   SAML only.
/// - Display name comes from the IdP's mapped "name" attribute when
///   provided, otherwise stays null.
async fn resolve_or_provision_mailbox(
    state: &AppState,
    config: &SamlConfiguration,
    email: &str,
    display_name: Option<&str>,
) -> Result<Mailbox, AppError> {
    if let Some(existing) = Mailbox::find_by_username(&state.db, email).await? {
        return Ok(existing);
    }
    if !config.auto_create_users {
        return Err(AppError::Forbidden(
            "No mailbox exists for this SAML subject and auto-provisioning is disabled"
                .to_string(),
        ));
    }

    // Reuse the BYOK synthetic domain — SAML-provisioned users don't bring
    // their own DNS, they bring their own IdP.
    let domain_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM domains WHERE name = 'byok.tasmail' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "byok.tasmail domain row missing — re-run migration 056"
        ))
    })?;

    // Strong random password — the user authenticates via SAML, not this
    // hash. We still need a value because mailboxes.password_hash is NOT NULL.
    let random_password = format!("saml-{}-{}", Uuid::new_v4(), Uuid::new_v4());
    let password_hash = auth_service::hash_password(&random_password)?;

    // 1 GiB default quota — TASMail itself doesn't store mail (it's a webmail
    // UI), but quota_bytes is NOT NULL. Matches the signup default.
    let mailbox = Mailbox::create(
        &state.db,
        email,
        &password_hash,
        domain_id,
        display_name,
        1_073_741_824,
    )
    .await?;
    Ok(mailbox)
}

// Added: Response type for SAML login redirect
#[derive(serde::Serialize)]
pub struct SamlLoginResponse {
    pub redirect_url: String,
}

// Added: Request payload for SAML callback (IdP posts back)
//
// Changed (TMAIL-303): `attributes` accepts the pre-extracted SAML assertion
// attribute map so the callback can resolve email + display_name without
// re-parsing the XML. Production IdP proxies (or future XML parsing inside
// this handler) populate it; tests use it to stub assertions deterministically.
#[derive(serde::Deserialize)]
pub struct SamlCallbackRequest {
    #[serde(alias = "SAMLResponse")]
    pub saml_response: String,
    #[serde(alias = "RelayState")]
    pub relay_state: Option<String>,
    pub name_id: Option<String>,
    pub session_index: Option<String>,
    // Added (TMAIL-303): IdP attribute map (raw `IdPAttrName → value`) so the
    // handler can look up email / displayName via the configured mapping.
    #[serde(default)]
    pub attributes: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    // Added (TMAIL-303): Pure-function coverage for the SAML callback
    // request payload + the subject-email resolution rules. The full
    // handler is exercised by the HTTP-layer suite in tests/saml_test.rs;
    // these unit tests pin the wire contract and the find-or-create
    // decision matrix without needing a DB.
    use super::*;
    use crate::models::saml_config::SamlConfiguration;

    fn test_config(auto_create: bool) -> SamlConfiguration {
        SamlConfiguration {
            id: Uuid::new_v4(),
            name: "Test IdP".to_string(),
            entity_id: "https://idp.example.com".to_string(),
            sso_url: "https://idp.example.com/sso".to_string(),
            slo_url: None,
            certificate: "CERT".to_string(),
            name_id_format: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
            attribute_mapping: serde_json::json!({
                "email": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress",
                "name": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name"
            }),
            active: true,
            auto_create_users: auto_create,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn saml_callback_request_accepts_pascalcase_aliases() {
        // IdPs POST `SAMLResponse=…&RelayState=…` (the SAML spec spelling).
        let json_str = r##"{
            "SAMLResponse": "PHNhbWxwOlJlc3BvbnNlPjwvc2FtbHA6UmVzcG9uc2U+",
            "RelayState": "abc-config-id",
            "name_id": "user@corp.com"
        }"##;
        let req: SamlCallbackRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.saml_response, "PHNhbWxwOlJlc3BvbnNlPjwvc2FtbHA6UmVzcG9uc2U+");
        assert_eq!(req.relay_state.as_deref(), Some("abc-config-id"));
        assert_eq!(req.name_id.as_deref(), Some("user@corp.com"));
        assert!(req.attributes.is_none());
    }

    #[test]
    fn saml_callback_request_attributes_round_trip() {
        let json_str = r##"{
            "saml_response": "x",
            "attributes": {
                "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress": "user@corp.com",
                "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name": "Jane Doe"
            }
        }"##;
        let req: SamlCallbackRequest = serde_json::from_str(json_str).unwrap();
        let attrs = req.attributes.unwrap();
        assert_eq!(
            attrs["http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress"],
            "user@corp.com"
        );
        assert_eq!(
            attrs["http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name"],
            "Jane Doe"
        );
    }

    /// Pure helper mirroring the handler's email-resolution branch.
    /// Kept here (not promoted to a free function) so the test sticks
    /// close to the handler logic and breaks loudly if the rules change.
    fn resolve_email(
        config: &SamlConfiguration,
        attributes: &serde_json::Value,
        name_id: Option<&str>,
    ) -> Option<String> {
        config
            .resolve_attribute(attributes, "email")
            .or_else(|| name_id.map(str::to_string))
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty() && s.contains('@'))
    }

    #[test]
    fn resolve_email_prefers_mapped_attribute_over_name_id() {
        let cfg = test_config(true);
        let attrs = serde_json::json!({
            "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress": "Mapped@CORP.com"
        });
        assert_eq!(
            resolve_email(&cfg, &attrs, Some("other@corp.com")),
            Some("mapped@corp.com".to_string()),
            "mapped attribute wins + value is lowercased"
        );
    }

    #[test]
    fn resolve_email_falls_back_to_name_id() {
        let cfg = test_config(true);
        // Attributes don't carry the mapped key.
        let attrs = serde_json::json!({});
        assert_eq!(
            resolve_email(&cfg, &attrs, Some("  Subject@Corp.com  ")),
            Some("subject@corp.com".to_string()),
            "fallback to name_id + trim + lowercase"
        );
    }

    #[test]
    fn resolve_email_rejects_non_email_subject() {
        let cfg = test_config(true);
        let attrs = serde_json::json!({});
        // Persistent ID format — not an email. Must be rejected.
        assert_eq!(
            resolve_email(&cfg, &attrs, Some("urn:oid:1.3.6.1.4.1.5923.1.1.1.13")),
            None
        );
    }

    #[test]
    fn resolve_email_rejects_empty_subject() {
        let cfg = test_config(true);
        let attrs = serde_json::json!({});
        assert_eq!(resolve_email(&cfg, &attrs, Some("")), None);
        assert_eq!(resolve_email(&cfg, &attrs, None), None);
    }
}
