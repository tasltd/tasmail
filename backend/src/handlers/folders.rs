use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

/// TMAIL-324: built-in folder names that the alt-UI sidebar's Add/Delete flow
/// must never touch. The list is intentionally short — server-side hierarchy
/// folders like `[Gmail]/...` are still creatable/deletable by power users via
/// other UIs; this gate is only for the sidebar quick-add flow.
const PROTECTED_FOLDER_NAMES: &[&str] = &[
    "INBOX",
    "Inbox",
    "Sent",
    "Sent Items",
    "Drafts",
    "Trash",
    "Deleted Items",
    "Bin",
    "Junk",
    "Junk Mail",
    "Spam",
    "Archive",
];

fn is_protected_folder(name: &str) -> bool {
    PROTECTED_FOLDER_NAMES
        .iter()
        .any(|p| p.eq_ignore_ascii_case(name))
}

/// TMAIL-324: validate the user-supplied folder name. We refuse empty strings,
/// names longer than 255 chars (RFC 3501 doesn't fix a limit but most servers
/// cap somewhere here), names containing IMAP hierarchy delimiters (`/`, `.`),
/// names starting with the namespace prefix `[Gmail]` (which Gmail rejects
/// anyway), and names that match a built-in mailbox.
fn validate_folder_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Folder name is required".to_string()));
    }
    if trimmed.len() > 255 {
        return Err(AppError::BadRequest(
            "Folder name must be 255 characters or fewer".to_string(),
        ));
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(AppError::BadRequest(
            "Folder name must not contain path separators".to_string(),
        ));
    }
    if trimmed.contains(['\r', '\n', '\0']) {
        return Err(AppError::BadRequest(
            "Folder name must not contain control characters".to_string(),
        ));
    }
    if is_protected_folder(trimmed) {
        return Err(AppError::BadRequest(format!(
            "'{}' is a built-in folder and cannot be created or deleted",
            trimmed
        )));
    }
    Ok(trimmed.to_string())
}

/// TMAIL-324: POST /api/folders — create a new IMAP mailbox.
/// Body: `{ "name": "Projects" }`. Returns the freshly-listed Folder.
pub async fn create_folder(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(req): Json<CreateFolderRequest>,
) -> Result<(StatusCode, Json<Folder>), AppError> {
    let name = validate_folder_name(&req.name)?;

    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;

    let folder = imap_service.create_folder(username, password, &name).await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

/// TMAIL-324: DELETE /api/folders/{folder} — delete an IMAP mailbox.
/// Refuses to delete built-in folders. The IMAP DELETE per RFC 3501 §6.3.4
/// removes the mailbox along with all messages it contains.
pub async fn delete_folder(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(folder): Path<String>,
) -> Result<StatusCode, AppError> {
    let name = validate_folder_name(&folder)?;

    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;

    imap_service.delete_folder(username, password, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_folder_name_accepts_simple_names() {
        assert_eq!(validate_folder_name("Projects").unwrap(), "Projects");
        assert_eq!(validate_folder_name("Receipts 2026").unwrap(), "Receipts 2026");
        // Trims surrounding whitespace.
        assert_eq!(validate_folder_name("  Work  ").unwrap(), "Work");
    }

    #[test]
    fn validate_folder_name_rejects_empty_and_whitespace_only() {
        assert!(validate_folder_name("").is_err());
        assert!(validate_folder_name("   ").is_err());
    }

    #[test]
    fn validate_folder_name_rejects_path_separators() {
        assert!(validate_folder_name("foo/bar").is_err());
        assert!(validate_folder_name("foo\\bar").is_err());
    }

    #[test]
    fn validate_folder_name_rejects_control_chars() {
        assert!(validate_folder_name("foo\nbar").is_err());
        assert!(validate_folder_name("foo\rbar").is_err());
        assert!(validate_folder_name("foo\0bar").is_err());
    }

    #[test]
    fn validate_folder_name_rejects_too_long() {
        let long = "a".repeat(256);
        assert!(validate_folder_name(&long).is_err());
        let max = "a".repeat(255);
        assert!(validate_folder_name(&max).is_ok());
    }

    #[test]
    fn validate_folder_name_rejects_built_ins_case_insensitive() {
        assert!(validate_folder_name("INBOX").is_err());
        assert!(validate_folder_name("inbox").is_err());
        assert!(validate_folder_name("Inbox").is_err());
        assert!(validate_folder_name("Sent").is_err());
        assert!(validate_folder_name("trash").is_err());
        assert!(validate_folder_name("Archive").is_err());
        assert!(validate_folder_name("Drafts").is_err());
    }

    #[test]
    fn is_protected_folder_recognises_all_built_ins() {
        for name in PROTECTED_FOLDER_NAMES {
            assert!(is_protected_folder(name), "{name} should be protected");
        }
        // Custom names are NOT protected.
        assert!(!is_protected_folder("Projects"));
        assert!(!is_protected_folder("Receipts"));
    }
}
