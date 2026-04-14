// Added: SAML 2.0 SSO configuration and authentication handlers for TMAIL-101
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::saml_config::{
    CreateSamlConfigRequest, SamlConfiguration, SamlSession, UpdateSamlConfigRequest,
};
use crate::state::AppState;

/// PURPOSE: List all SAML IdP configurations (admin only)
/// EXTERNAL: GET /api/admin/saml
pub async fn list_saml_configs(
    State(state): State<AppState>,
) -> Result<Json<Vec<SamlConfiguration>>, AppError> {
    let configs = SamlConfiguration::list(&state.db).await?;
    Ok(Json(configs))
}

/// PURPOSE: Create a new SAML IdP configuration (admin only)
/// CONSTRAINTS: Requires name, entity_id, sso_url, certificate
/// EXTERNAL: POST /api/admin/saml
pub async fn create_saml_config(
    State(state): State<AppState>,
    Json(request): Json<CreateSamlConfigRequest>,
) -> Result<(StatusCode, Json<SamlConfiguration>), AppError> {
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
    Ok((StatusCode::CREATED, Json(config)))
}

/// PURPOSE: Update an existing SAML IdP configuration (admin only)
/// CONSTRAINTS: All fields optional — only provided fields are updated
/// EXTERNAL: PUT /api/admin/saml/:id
pub async fn update_saml_config(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateSamlConfigRequest>,
) -> Result<Json<SamlConfiguration>, AppError> {
    // Added: Verify config exists before updating
    SamlConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("SAML configuration {id} not found")))?;

    let config = SamlConfiguration::update(&state.db, id, &request).await?;
    Ok(Json(config))
}

/// PURPOSE: Delete a SAML IdP configuration (admin only)
/// EXTERNAL: DELETE /api/admin/saml/:id
pub async fn delete_saml_config(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    // Added: Verify config exists before deleting
    SamlConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("SAML configuration {id} not found")))?;

    SamlConfiguration::delete(&state.db, id).await?;
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

/// PURPOSE: Process SAML Response callback from IdP (public)
/// CONSTRAINTS: In production, this must validate the XML signature against the IdP certificate
/// EXTERNAL: POST /api/auth/saml/callback
pub async fn saml_callback(
    State(state): State<AppState>,
    Json(request): Json<SamlCallbackRequest>,
) -> Result<Json<SamlCallbackResponse>, AppError> {
    // Added: Decode the base64 SAMLResponse
    use base64::Engine;
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(&request.saml_response)
        .map_err(|_| AppError::BadRequest("Invalid SAMLResponse encoding".to_string()))?;

    let _saml_xml = String::from_utf8(decoded_bytes)
        .map_err(|_| AppError::BadRequest("SAMLResponse is not valid UTF-8".to_string()))?;

    // NOTE: In a production implementation, the XML would be parsed and its signature
    // validated against the IdP certificate. For now, we extract the provided attributes
    // from the callback request to demonstrate the flow.

    // Added: Look up the SAML config if a config_id was provided
    let saml_config_id = request.relay_state.as_deref().and_then(|rs| Uuid::parse_str(rs).ok());
    let config = if let Some(config_id) = saml_config_id {
        Some(
            SamlConfiguration::get_by_id(&state.db, config_id)
                .await
                .map_err(|_| AppError::NotFound("SAML configuration not found".to_string()))?,
        )
    } else {
        None
    };

    // Added: Create a SAML session entry for SLO tracking
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(8);
    if let Some(ref cfg) = config {
        let _ = SamlSession::create(
            &state.db,
            cfg.id,
            None,
            request.session_index.as_deref(),
            &request.name_id.clone().unwrap_or_default(),
            &serde_json::json!({}),
            expires_at,
        )
        .await;
    }

    // NOTE: In production, this would find-or-create the user in the users table,
    // then issue a real JWT using auth_service. For now, we return a placeholder response.
    Ok(Json(SamlCallbackResponse {
        message: "SAML authentication processed. User lookup and JWT issuance would occur here."
            .to_string(),
        name_id: request.name_id,
        session_index: request.session_index,
    }))
}

// Added: Response type for SAML login redirect
#[derive(serde::Serialize)]
pub struct SamlLoginResponse {
    pub redirect_url: String,
}

// Added: Request payload for SAML callback (IdP posts back)
#[derive(serde::Deserialize)]
pub struct SamlCallbackRequest {
    #[serde(alias = "SAMLResponse")]
    pub saml_response: String,
    #[serde(alias = "RelayState")]
    pub relay_state: Option<String>,
    // NOTE: These fields would normally be extracted from the parsed SAML XML
    pub name_id: Option<String>,
    pub session_index: Option<String>,
}

// Added: Response type for SAML callback result
#[derive(serde::Serialize)]
pub struct SamlCallbackResponse {
    pub message: String,
    pub name_id: Option<String>,
    pub session_index: Option<String>,
}
