use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::{FullMessage, ImapService, MessageEnvelope};
use crate::services::smtp_service::{SendRequest, SmtpService};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListMessagesQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub folder: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MoveRequest {
    pub to_folder: String,
}

#[derive(Debug, Deserialize)]
pub struct FlagRequest {
    pub flag: String,
    pub add: bool,
}

/// GET /api/folders/:folder/messages — list messages in a folder
pub async fn list_messages(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(folder): Path<String>,
    Query(query): Query<ListMessagesQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let page = query.page.unwrap_or(0);
    let page_size = query.page_size.unwrap_or(50);

    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::new(state.config.imap.clone());
    let (messages, total) = imap_service
        .list_messages(&mailbox.username, &mailbox.password_hash, &folder, page, page_size)
        .await?;

    Ok(Json(serde_json::json!({
        "messages": messages,
        "total": total,
        "page": page,
        "page_size": page_size,
    })))
}

/// GET /api/folders/:folder/messages/:uid — get a full message
pub async fn get_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<Json<FullMessage>, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::new(state.config.imap.clone());
    let message = imap_service
        .get_message(&mailbox.username, &mailbox.password_hash, &folder, uid)
        .await?;

    Ok(Json(message))
}

/// POST /api/messages/send — send an email
pub async fn send_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SendRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let smtp_service = SmtpService::new(state.config.smtp.clone());
    smtp_service
        .send(&mailbox.username, &mailbox.password_hash, &body)
        .await?;

    Ok(StatusCode::CREATED)
}

/// GET /api/search — search messages across folders
pub async fn search_messages(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let folder = query.folder.as_deref().unwrap_or("INBOX");
    let imap_service = ImapService::new(state.config.imap.clone());
    let messages = imap_service
        .search_messages(&mailbox.username, &mailbox.password_hash, folder, &query.q)
        .await?;

    Ok(Json(serde_json::json!({
        "messages": messages,
        "total": messages.len(),
        "query": query.q,
        "folder": folder,
    })))
}

/// DELETE /api/folders/:folder/messages/:uid — delete a message
pub async fn delete_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::new(state.config.imap.clone());
    imap_service
        .delete_message(&mailbox.username, &mailbox.password_hash, &folder, uid)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/folders/:folder/messages/:uid/move — move a message
pub async fn move_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
    Json(body): Json<MoveRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::new(state.config.imap.clone());
    imap_service
        .move_message(&mailbox.username, &mailbox.password_hash, &folder, uid, &body.to_folder)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/folders/:folder/messages/:uid/flag — set/remove a flag
pub async fn flag_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
    Json(body): Json<FlagRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::new(state.config.imap.clone());
    imap_service
        .set_flag(&mailbox.username, &mailbox.password_hash, &folder, uid, &body.flag, body.add)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
