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
