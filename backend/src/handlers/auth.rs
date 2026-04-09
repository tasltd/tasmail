use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::services::auth_service;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<(StatusCode, Json<auth_service::TokenPair>), AppError> {
    let tokens = auth_service::authenticate(
        &state.db,
        &state.config.jwt,
        &body.username,
        &body.password,
        None,
        None,
    )
    .await?;

    Ok((StatusCode::OK, Json(tokens)))
}

/// POST /api/auth/refresh
pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<auth_service::TokenPair>, AppError> {
    let tokens =
        auth_service::refresh_tokens(&state.db, &state.config.jwt, &body.refresh_token).await?;

    Ok(Json(tokens))
}

/// POST /api/auth/logout
pub async fn logout(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<auth_service::Claims>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in token")))?;

    // Delete all sessions for this user
    crate::models::session::Session::delete_all_for_mailbox(&state.db, mailbox_id).await?;

    Ok(StatusCode::NO_CONTENT)
}
