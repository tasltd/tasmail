// Added: OIDC identity provider handlers for TMAIL-99
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::oidc_provider::{
    CreateOidcProviderRequest, OidcCallbackRequest, OidcLoginProvider, OidcProvider,
    UpdateOidcProviderRequest,
};
use crate::services::auth_service::{self, Claims};
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

/// PURPOSE: Handle OIDC callback — exchange authorization code for tokens
/// CONSTRAINTS: Validates state, exchanges code, finds/creates user, issues JWT
/// EXTERNAL: POST /api/auth/oidc/callback
pub async fn oidc_callback(
    State(state): State<AppState>,
    Json(request): Json<OidcCallbackRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Added: Validate that code and state are present
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

    // NOTE: Full OIDC token exchange flow would:
    // 1. Validate state token matches what was stored in session/cookie
    // 2. Exchange authorization code for access_token + id_token at provider's token endpoint
    // 3. Validate id_token signature using provider's JWKS
    // 4. Extract subject + email from id_token claims
    // 5. Find existing OidcUserLink or create new user if auto_create_users is enabled
    // 6. Issue local JWT access + refresh tokens
    //
    // For now, return a placeholder response. The actual implementation requires
    // an HTTP client call to the provider's token endpoint.
    let _db = &state.db;

    Err(AppError::BadRequest(
        "OIDC callback token exchange not yet implemented. Provider token endpoint integration required.".to_string(),
    ))
}
