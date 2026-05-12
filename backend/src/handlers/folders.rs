use axum::{extract::State, Json};

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::{Folder, ImapService};
use crate::state::AppState;

/// GET /api/folders — list all IMAP folders for the authenticated user.
/// BYOK: builds an `ImapService` from the user's saved imap_configurations row
/// and connects with their per-server credentials.
pub async fn list_folders(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Folder>>, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let folders = imap_service.list_folders(username, password).await?;
    Ok(Json(folders))
}
