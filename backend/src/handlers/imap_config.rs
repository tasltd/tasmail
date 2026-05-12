// Added: Per-user IMAP configuration handlers (BYOK webmail pivot).
// CRUD endpoints + connection tester. Mirrors smtp_config.rs structure.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::ai_config::derive_encryption_key;
use crate::models::imap_config::{
    provider_presets, CreateImapConfigRequest, ImapConfigSummary, ImapConfiguration,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

fn parse_user_id(claims: &Claims) -> Result<Uuid, AppError> {
    Uuid::parse_str(&claims.sub).map_err(|e| AppError::BadRequest(format!("Invalid user id: {}", e)))
}

/// GET /api/imap-configs — list user's saved IMAP servers (passwords scrubbed)
pub async fn list_imap_configs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ImapConfigSummary>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let configs = ImapConfiguration::list_for_user(&state.db, user_id).await?;
    Ok(Json(configs.into_iter().map(ImapConfigSummary::from).collect()))
}

/// POST /api/imap-configs — add a new IMAP server. Password is encrypted at rest.
pub async fn create_imap_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateImapConfigRequest>,
) -> Result<(StatusCode, Json<ImapConfigSummary>), AppError> {
    let user_id = parse_user_id(&claims)?;
    if body.host.trim().is_empty() || body.username.trim().is_empty() || body.password.is_empty() {
        return Err(AppError::BadRequest("host, username, and password are required".into()));
    }
    let key = derive_encryption_key(&state.config.jwt.secret);
    let cfg = ImapConfiguration::create(&state.db, user_id, &body, &key).await?;
    // TMAIL-162: drop the per-user cache so the next request picks up the new default.
    let _ = state.cache.invalidate_user_imap_config(&user_id.to_string()).await;
    Ok((StatusCode::CREATED, Json(ImapConfigSummary::from(cfg))))
}

/// DELETE /api/imap-configs/{id} — remove a saved IMAP server
pub async fn delete_imap_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let removed = ImapConfiguration::delete(&state.db, user_id, id).await?;
    if !removed {
        return Err(AppError::NotFound(format!("imap_configuration {}", id)));
    }
    // TMAIL-162: drop cache so the next request reflects the deletion.
    let _ = state.cache.invalidate_user_imap_config(&user_id.to_string()).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct TestImapRequest {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub encryption: String,
}

/// POST /api/imap-configs/test — TCP-connect to the IMAP server and try to LOGIN.
/// Used by the onboarding wizard's "Test connection" button.
pub async fn test_imap(
    State(_state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<TestImapRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = test_imap_connection(&body).await;
    match result {
        Ok(()) => Ok(Json(serde_json::json!({"ok": true, "message": "IMAP login succeeded"}))),
        Err(e) => Ok(Json(serde_json::json!({"ok": false, "message": e.to_string()}))),
    }
}

async fn test_imap_connection(req: &TestImapRequest) -> anyhow::Result<()> {
    use async_imap::Client;
    use tokio::net::TcpStream;
    use tokio_util::compat::TokioAsyncReadCompatExt;

    let tcp = TcpStream::connect((req.host.as_str(), req.port as u16))
        .await
        .map_err(|e| anyhow::anyhow!("TCP connect failed: {}", e))?;

    match req.encryption.as_str() {
        "ssl" => {
            // Mirror imap_service.rs's TLS path (async_native_tls)
            let tls = async_native_tls::TlsConnector::new();
            let tls_stream = tls.connect(&req.host, tcp.compat()).await
                .map_err(|e| anyhow::anyhow!("TLS handshake failed: {}", e))?;
            let client = Client::new(tls_stream);
            let mut session = client.login(&req.username, &req.password).await
                .map_err(|(e, _)| anyhow::anyhow!("LOGIN failed: {}", e))?;
            let _ = session.logout().await;
            Ok(())
        }
        "starttls" | "none" => {
            let client = Client::new(tcp.compat());
            let mut session = client.login(&req.username, &req.password).await
                .map_err(|(e, _)| anyhow::anyhow!("LOGIN failed: {}", e))?;
            let _ = session.logout().await;
            Ok(())
        }
        other => Err(anyhow::anyhow!("Unknown encryption: {}", other)),
    }
}

/// GET /api/imap-configs/presets — onboarding wizard auto-discover for popular providers
pub async fn list_provider_presets(
    State(_state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::Value::Array(provider_presets())))
}
