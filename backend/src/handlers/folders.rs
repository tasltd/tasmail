use axum::{extract::State, Json};

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::{Folder, ImapService};
use crate::state::AppState;

/// GET /api/folders — list all IMAP folders for the authenticated user
pub async fn list_folders(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Folder>>, AppError> {
    // NOTE: In production, we'd retrieve the IMAP password from a secure store
    // or use master-user auth with Dovecot. For now, this requires the user's
    // IMAP credentials to be stored or passed via a session mechanism.
    let imap_service = ImapService::new(state.config.imap.clone());

    // Retrieve user's IMAP credentials from the database
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // Use Dovecot master-user authentication: username*masteruser with master password
    // For development, we use the user's own credentials
    let folders = imap_service
        .list_folders(&mailbox.username, &mailbox.password_hash)
        .await?;

    Ok(Json(folders))
}
