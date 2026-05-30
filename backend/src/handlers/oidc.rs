// Added: OIDC identity provider handlers for TMAIL-99
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::models::mailbox::Mailbox;
use crate::models::oidc_provider::{
    CreateOidcProviderRequest, OidcCallbackRequest, OidcDiscovery, OidcIdTokenClaims,
    OidcLoginProvider, OidcProvider, OidcTokenResponse, OidcUserLink, UpdateOidcProviderRequest,
};
use crate::services::auth_service::{self, Claims, TokenPair};
use crate::state::AppState;

/// PURPOSE: List all OIDC providers (admin view with full config)
/// EXTERNAL: GET /api/admin/oidc
pub async fn list_oidc_providers(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<OidcProvider>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let providers = OidcProvider::list(&state.db).await?;
    Ok(Json(providers))
}

/// PURPOSE: Create a new OIDC provider configuration
/// CONSTRAINTS: Requires name, issuer_url, client_id, client_secret, redirect_uri
/// EXTERNAL: POST /api/admin/oidc
pub async fn create_oidc_provider(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(request): Json<CreateOidcProviderRequest>,
) -> Result<(StatusCode, Json<OidcProvider>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate required fields are non-empty
    if request.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Provider name is required".to_string(),
        ));
    }
    if request.issuer_url.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Issuer URL is required".to_string(),
        ));
    }
    if request.client_id.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Client ID is required".to_string(),
        ));
    }
    if request.client_secret.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Client secret is required".to_string(),
        ));
    }
    if request.redirect_uri.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Redirect URI is required".to_string(),
        ));
    }

    // NOTE: In production, client_secret should be encrypted before storage.
    // For now, we store it as-is; a proper encryption service would wrap this.
    let encrypted_secret = &request.client_secret;

    let provider = OidcProvider::create(&state.db, &request, encrypted_secret).await?;
    Ok((StatusCode::CREATED, Json(provider)))
}

/// PURPOSE: Update an existing OIDC provider configuration
/// CONSTRAINTS: All fields optional — only provided fields are updated
/// EXTERNAL: PUT /api/admin/oidc/:id
pub async fn update_oidc_provider(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateOidcProviderRequest>,
) -> Result<Json<OidcProvider>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify provider exists before updating
    OidcProvider::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("OIDC provider {id} not found")))?;

    // NOTE: Only encrypt secret if a new one was provided
    let encrypted_secret = request.client_secret.as_deref();

    let provider =
        OidcProvider::update(&state.db, id, &request, encrypted_secret).await?;
    Ok(Json(provider))
}

/// PURPOSE: Delete an OIDC provider and its user links
/// EXTERNAL: DELETE /api/admin/oidc/:id
pub async fn delete_oidc_provider(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify provider exists before deleting
    OidcProvider::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("OIDC provider {id} not found")))?;

    OidcProvider::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: List active OIDC providers for the login page (public endpoint)
/// CONSTRAINTS: Only returns id, name, icon_url, button_label — no secrets
/// EXTERNAL: GET /api/auth/oidc/providers
pub async fn list_login_providers(
    State(state): State<AppState>,
) -> Result<Json<Vec<OidcLoginProvider>>, AppError> {
    let providers = OidcProvider::list_active(&state.db).await?;
    // Added: Map to public-facing struct that excludes sensitive config
    let login_providers: Vec<OidcLoginProvider> = providers
        .into_iter()
        .map(|p| OidcLoginProvider {
            id: p.id,
            name: p.name,
            icon_url: p.icon_url,
            button_label: p.button_label,
        })
        .collect();
    Ok(Json(login_providers))
}

/// PURPOSE: Generate an OIDC authorization URL and return it for frontend redirect
/// CONSTRAINTS: Generates a random state token for CSRF protection
/// EXTERNAL: GET /api/auth/oidc/:id/authorize
pub async fn get_authorize_url(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let provider = OidcProvider::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("OIDC provider {id} not found")))?;

    if !provider.active {
        return Err(AppError::BadRequest(
            "This OIDC provider is not active".to_string(),
        ));
    }

    // Added: Generate a random state token for CSRF protection
    let state_token = Uuid::new_v4().to_string();
    let authorize_url = provider.build_authorize_url(&state_token);

    Ok(Json(serde_json::json!({
        "authorize_url": authorize_url,
        "state": state_token,
    })))
}

/// PURPOSE: Handle OIDC callback — exchange authorization code for tokens (TMAIL-304).
///
/// CONSTRAINTS:
/// - `code`, `state`, and `provider_id` are all required from the SPA callback POST.
/// - The SPA performs the local CSRF check (state matches sessionStorage value)
///   before calling us. Server-side state tracking (Redis-backed) is a follow-up;
///   here we only require state is non-empty so the contract stays honest.
/// - The provider must be active and registered in `oidc_providers`.
/// - The id_token MUST verify against the provider's JWKS, with `aud == client_id`
///   and `iss == issuer_url`. jsonwebtoken's Validation struct enforces these.
///
/// FLOW (matches the steps from the original in-code comment):
///   1. Validate code + state + provider_id non-empty.
///   2. Load provider, check active.
///   3. Fetch OIDC discovery document (`/.well-known/openid-configuration`)
///      to resolve token_endpoint + jwks_uri — works correctly for Google,
///      Microsoft, Auth0, Okta etc. without hardcoded paths.
///   4. POST to token_endpoint (form-urlencoded) to exchange the auth code
///      for an id_token.
///   5. Fetch JWKS, find the matching key by `kid`, validate id_token
///      signature + aud + iss + exp.
///   6. Extract `sub` + `email` from verified id_token claims.
///   7. Find-or-create OidcUserLink → find-or-provision Mailbox
///      (gated by `auto_create_users`).
///   8. Issue local TokenPair (matches POST /api/auth/login shape).
///
/// EXTERNAL: POST /api/auth/oidc/callback
pub async fn oidc_callback(
    State(state): State<AppState>,
    Json(request): Json<OidcCallbackRequest>,
) -> Result<Json<TokenPair>, AppError> {
    // 1. Early-return guards — cheap input validation first, before any DB
    //    or network I/O. Empty `code` / `state` mean the SPA misrouted the
    //    redirect; missing `provider_id` means we can't look up the IdP.
    if request.code.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Authorization code is required".to_string(),
        ));
    }
    if request.state.trim().is_empty() {
        return Err(AppError::BadRequest(
            "State parameter is required for CSRF validation".to_string(),
        ));
    }
    let provider_id = request.provider_id.ok_or_else(|| {
        AppError::BadRequest(
            "provider_id is required so the backend can resolve the IdP config".to_string(),
        )
    })?;

    // 2. Load the IdP config + reject inactive providers.
    let provider = OidcProvider::get_by_id(&state.db, provider_id)
        .await
        .map_err(|_| AppError::NotFound("OIDC provider not found".to_string()))?;
    if !provider.active {
        return Err(AppError::BadRequest(
            "This OIDC provider is not active".to_string(),
        ));
    }

    // 3-5. Talk to the IdP: discovery → token exchange → JWKS verification.
    let claims =
        exchange_code_and_verify_id_token(&provider, &request.code).await?;

    // 6. Resolve the local mailbox: existing link → existing mailbox →
    //    auto-provision (gated by config.auto_create_users).
    let (mailbox, link_existed) =
        resolve_or_provision_user(&state, &provider, &claims).await?;

    // 7. Issue the same TokenPair shape POST /api/auth/login returns so the
    //    SPA's existing token-handling code keeps working unchanged.
    let tokens = auth_service::issue_token_pair_for_mailbox(
        &state.db,
        &state.config.jwt,
        &mailbox,
    )
    .await?;

    // 8. Audit. Investigators rely on this row for SSO forensics. Mirrors
    //    SAML's `auth.saml_login` event (TMAIL-303).
    let mailbox_id_str = mailbox.id.to_string();
    let _ = AuditLog::record(
        &state.db,
        Some(mailbox.id),
        "auth.oidc_login",
        Some("mailbox"),
        Some(mailbox_id_str.as_str()),
        Some(serde_json::json!({
            "oidc_provider_id": provider.id,
            "oidc_provider_name": provider.name,
            "subject": claims.sub,
            "link_existed": link_existed,
        })),
        None,
        None,
    )
    .await;

    Ok(Json(tokens))
}

/// PURPOSE (TMAIL-304): Discover the IdP endpoints, exchange the auth code
/// for an id_token, and verify the id_token signature/aud/iss.
/// Returns the parsed + verified id_token claims on success.
///
/// CONSTRAINTS: Network calls bounded to 10s each to avoid pinning a
/// connection on a slow IdP. Errors are mapped to `BadRequest` (caller's
/// fault: wrong code, expired code) vs `ServiceUnavailable` (IdP down).
async fn exchange_code_and_verify_id_token(
    provider: &OidcProvider,
    code: &str,
) -> Result<OidcIdTokenClaims, AppError> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to build OIDC HTTP client: {e}"
            ))
        })?;

    // OIDC Discovery 1.0 — gives us token_endpoint + jwks_uri without
    // hardcoding provider-specific paths.
    let discovery: OidcDiscovery = http
        .get(provider.discovery_url())
        .send()
        .await
        .map_err(|e| {
            AppError::ServiceUnavailable(format!(
                "Failed to reach OIDC discovery endpoint: {e}"
            ))
        })?
        .error_for_status()
        .map_err(|e| {
            AppError::ServiceUnavailable(format!(
                "OIDC discovery returned HTTP error: {e}"
            ))
        })?
        .json()
        .await
        .map_err(|e| {
            AppError::ServiceUnavailable(format!(
                "OIDC discovery returned malformed JSON: {e}"
            ))
        })?;

    // RFC 6749 §4.1.3 token-exchange request.
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", provider.redirect_uri.as_str()),
        ("client_id", provider.client_id.as_str()),
        ("client_secret", provider.client_secret_encrypted.as_str()),
    ];
    let token_resp = http
        .post(&discovery.token_endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&form)
        .send()
        .await
        .map_err(|e| {
            AppError::ServiceUnavailable(format!(
                "Failed to reach OIDC token endpoint: {e}"
            ))
        })?;

    // IdP-side rejection (invalid_grant, invalid_client, etc.) is the
    // caller's fault — surface as 400 so the SPA can prompt re-login.
    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "OIDC token endpoint rejected the authorization code (HTTP {status}): {body}"
        )));
    }
    let tokens: OidcTokenResponse = token_resp.json().await.map_err(|e| {
        AppError::ServiceUnavailable(format!(
            "OIDC token endpoint returned malformed JSON: {e}"
        ))
    })?;

    let claims = verify_id_token(&http, &tokens.id_token, &discovery, provider).await?;
    Ok(claims)
}

/// PURPOSE (TMAIL-304): Fetch JWKS, find the signing key, validate the
/// id_token signature + standard OIDC claims (iss, aud, exp).
async fn verify_id_token(
    http: &reqwest::Client,
    id_token: &str,
    discovery: &OidcDiscovery,
    provider: &OidcProvider,
) -> Result<OidcIdTokenClaims, AppError> {
    // jsonwebtoken's decode_header is signature-less — gives us the `kid`
    // and `alg` so we can pick the right JWK.
    let header = decode_header(id_token).map_err(|e| {
        AppError::BadRequest(format!("id_token header is unparseable: {e}"))
    })?;
    let kid = header.kid.ok_or_else(|| {
        AppError::BadRequest("id_token header is missing `kid`".to_string())
    })?;
    let algorithm = header.alg;

    let jwks: JwkSet = http
        .get(&discovery.jwks_uri)
        .send()
        .await
        .map_err(|e| {
            AppError::ServiceUnavailable(format!(
                "Failed to fetch OIDC JWKS: {e}"
            ))
        })?
        .error_for_status()
        .map_err(|e| {
            AppError::ServiceUnavailable(format!(
                "OIDC JWKS endpoint returned HTTP error: {e}"
            ))
        })?
        .json()
        .await
        .map_err(|e| {
            AppError::ServiceUnavailable(format!(
                "OIDC JWKS endpoint returned malformed JSON: {e}"
            ))
        })?;

    let jwk = jwks.find(&kid).ok_or_else(|| {
        AppError::BadRequest(format!(
            "id_token kid '{kid}' not found in IdP JWKS"
        ))
    })?;
    let decoding_key = DecodingKey::from_jwk(jwk).map_err(|e| {
        AppError::BadRequest(format!(
            "id_token JWK could not be converted to a decoding key: {e}"
        ))
    })?;

    // Validation: signature + aud == client_id + iss == discovery.issuer
    // (which matches the configured issuer_url for compliant IdPs) + exp.
    let mut validation = Validation::new(jwk_to_algorithm(algorithm));
    validation.set_audience(&[provider.client_id.as_str()]);
    validation.set_issuer(&[discovery.issuer.as_str()]);
    let token_data = decode::<OidcIdTokenClaims>(id_token, &decoding_key, &validation)
        .map_err(|e| {
            AppError::BadRequest(format!("id_token failed verification: {e}"))
        })?;

    Ok(token_data.claims)
}

/// Added (TMAIL-304): Pin the JWS algorithm we'll validate the id_token
/// under. We accept the algorithm declared in the id_token header *as long
/// as it's one of the OIDC-recommended signing algs* — RS256 is by far the
/// most common (Google, Microsoft, Auth0 default). Symmetric algs (HS*)
/// would let a stolen client_secret forge tokens, so we refuse them here.
fn jwk_to_algorithm(alg: Algorithm) -> Algorithm {
    match alg {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::ES256
        | Algorithm::ES384
        | Algorithm::PS256
        | Algorithm::PS384
        | Algorithm::PS512 => alg,
        // Fall back to RS256 for anything else — the JWK match step has
        // already constrained the key to the kid in the header, and the
        // signature verify will then fail loudly if the alg disagrees.
        _ => Algorithm::RS256,
    }
}

/// PURPOSE (TMAIL-304): Resolve the local mailbox for a verified OIDC
/// subject. Three cases:
///   - Link exists → existing user — fetch mailbox.
///   - No link, mailbox exists by email → create link, reuse mailbox.
///   - No link, no mailbox → auto-provision (gated by
///     `provider.auto_create_users`).
/// Returns the mailbox + whether the link already existed (for the audit row).
async fn resolve_or_provision_user(
    state: &AppState,
    provider: &OidcProvider,
    claims: &OidcIdTokenClaims,
) -> Result<(Mailbox, bool), AppError> {
    // Case 1: existing link → fast path.
    if let Some(link) =
        OidcUserLink::find_by_provider_subject(&state.db, provider.id, &claims.sub).await?
    {
        let mailbox = Mailbox::find_by_id(&state.db, link.user_id)
            .await?
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!(
                    "OIDC link references missing mailbox {}",
                    link.user_id
                ))
            })?;
        return Ok((mailbox, true));
    }

    // Cases 2 + 3 require an email.
    let email = claims.resolve_email().ok_or_else(|| {
        AppError::BadRequest(
            "OIDC id_token has no email claim — cannot resolve a local mailbox"
                .to_string(),
        )
    })?;

    // Case 2: mailbox exists, just missing the link → link it.
    if let Some(existing) = Mailbox::find_by_username(&state.db, &email).await? {
        let _ = OidcUserLink::create(
            &state.db,
            existing.id,
            provider.id,
            &claims.sub,
            &email,
        )
        .await?;
        return Ok((existing, false));
    }

    // Case 3: no mailbox yet — gated provisioning.
    if !provider.auto_create_users {
        return Err(AppError::Forbidden(
            "No mailbox exists for this OIDC subject and auto-provisioning is disabled"
                .to_string(),
        ));
    }

    // Reuse the BYOK synthetic domain — OIDC-provisioned users don't bring
    // their own DNS, they bring their own IdP. Same pattern as SAML.
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

    // Strong random password — the user authenticates via OIDC, not this
    // hash. We still need a value because mailboxes.password_hash is NOT NULL.
    let random_password = format!("oidc-{}-{}", Uuid::new_v4(), Uuid::new_v4());
    let password_hash = auth_service::hash_password(&random_password)?;

    let display_name = claims.resolve_display_name();
    let mailbox = Mailbox::create(
        &state.db,
        &email,
        &password_hash,
        domain_id,
        display_name.as_deref(),
        1_073_741_824, // 1 GiB default quota, matches signup + SAML provisioning.
    )
    .await?;

    let _ = OidcUserLink::create(
        &state.db,
        mailbox.id,
        provider.id,
        &claims.sub,
        &email,
    )
    .await?;

    Ok((mailbox, false))
}

#[cfg(test)]
mod tests {
    // Added (TMAIL-304): Pure-function coverage for the OIDC callback's
    // payload contract + algorithm-pinning helper. The handler's I/O paths
    // (discovery / token exchange / JWKS) need a network, so we cover the
    // contract surface here and let tests/oidc_test.rs pin the HTTP-layer
    // early-return guards.
    use super::*;

    #[test]
    fn callback_request_missing_provider_id_deserializes_but_is_caught_by_handler() {
        // The handler enforces provider_id; the wire shape stays permissive
        // so legacy clients see a 400 instead of a 422 (kinder error).
        let req: OidcCallbackRequest =
            serde_json::from_str(r##"{"code":"abc","state":"xyz"}"##).unwrap();
        assert!(req.provider_id.is_none());
        assert_eq!(req.code, "abc");
        assert_eq!(req.state, "xyz");
    }

    #[test]
    fn jwk_to_algorithm_passes_asymmetric_algs_through() {
        // RS/ES/PS are all asymmetric — keep them as the IdP declared them.
        assert!(matches!(jwk_to_algorithm(Algorithm::RS256), Algorithm::RS256));
        assert!(matches!(jwk_to_algorithm(Algorithm::RS384), Algorithm::RS384));
        assert!(matches!(jwk_to_algorithm(Algorithm::RS512), Algorithm::RS512));
        assert!(matches!(jwk_to_algorithm(Algorithm::ES256), Algorithm::ES256));
        assert!(matches!(jwk_to_algorithm(Algorithm::ES384), Algorithm::ES384));
        assert!(matches!(jwk_to_algorithm(Algorithm::PS256), Algorithm::PS256));
        assert!(matches!(jwk_to_algorithm(Algorithm::PS384), Algorithm::PS384));
        assert!(matches!(jwk_to_algorithm(Algorithm::PS512), Algorithm::PS512));
    }

    #[test]
    fn jwk_to_algorithm_refuses_hmac_algs() {
        // HS* algorithms would let a leaked client_secret forge id_tokens.
        // We force-downgrade to RS256 so the JWK conversion fails loudly
        // instead of silently accepting an HMAC-signed token.
        assert!(matches!(jwk_to_algorithm(Algorithm::HS256), Algorithm::RS256));
        assert!(matches!(jwk_to_algorithm(Algorithm::HS384), Algorithm::RS256));
        assert!(matches!(jwk_to_algorithm(Algorithm::HS512), Algorithm::RS256));
    }

    #[test]
    fn callback_request_provider_id_is_parsed_as_uuid() {
        let pid = Uuid::new_v4();
        let json_str = format!(
            r##"{{"code":"abc","state":"xyz","provider_id":"{}"}}"##,
            pid
        );
        let req: OidcCallbackRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(req.provider_id, Some(pid));
    }

    #[test]
    fn callback_request_rejects_non_uuid_provider_id() {
        // Misshapen provider_id values must fail at the JSON-extractor
        // layer before the handler sees them.
        let result: Result<OidcCallbackRequest, _> = serde_json::from_str(
            r##"{"code":"abc","state":"xyz","provider_id":"not-a-uuid"}"##,
        );
        assert!(result.is_err());
    }
}
