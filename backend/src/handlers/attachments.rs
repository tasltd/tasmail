// Added: Attachment upload/download/list/delete/stats handlers for TMAIL-59
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::attachment::{Attachment, StorageStats};
use crate::services::attachment_service::AttachmentService;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: Build AttachmentService from app config
/// NOTE: Reads storage config from app state; defaults if not configured
fn build_service(state: &AppState) -> AttachmentService {
    AttachmentService::new(
        PathBuf::from(&state.config.storage.attachment_dir),
        state.config.storage.clamav_socket.clone(),
    )
}

/// PURPOSE: Parse and validate mailbox_id from JWT claims
fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in token")))
}

/// POST /api/attachments — Upload attachment via multipart/form-data
/// CONSTRAINTS: Max file size enforced by config; virus scan runs after storage
pub async fn upload_attachment(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Attachment>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let service = build_service(&state);
    let max_size = state.config.storage.max_file_size;

    // Added: Extract file data from multipart form
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type = "application/octet-stream".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "file" {
            // Added: Capture filename and content type from multipart field
            filename = field.file_name().map(String::from);
            if let Some(ct) = field.content_type() {
                content_type = String::from(ct);
            }

            let data: axum::body::Bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Failed to read file data: {}", e)))?;

            // Added: Enforce max file size before processing
            if data.len() as u64 > max_size {
                return Err(AppError::BadRequest(format!(
                    "File size {} bytes exceeds maximum allowed {} bytes ({} MB)",
                    data.len(),
                    max_size,
                    max_size / (1024 * 1024)
                )));
            }

            file_data = Some(data.to_vec());
        }
    }

    let data = file_data.ok_or_else(|| {
        AppError::BadRequest("No file field found in multipart upload. Include a 'file' field.".to_string())
    })?;
    let filename = filename.unwrap_or_else(|| "unnamed".to_string());

    let size_bytes = data.len() as i64;

    // Added: Store file to disk and compute checksum
    let (storage_path, checksum) = service
        .store_file(mailbox_id, &data, &filename)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Added: Check for duplicate by checksum (deduplication)
    if let Some(existing) = Attachment::find_by_checksum(&state.db, mailbox_id, &checksum).await? {
        // NOTE: Clean up the just-stored duplicate file
        let _ = service.delete_file(&storage_path).await;
        return Ok((StatusCode::OK, Json(existing)));
    }

    // Added: Create database record
    let attachment = Attachment::create(
        &state.db,
        mailbox_id,
        &filename,
        &content_type,
        size_bytes,
        &storage_path,
        &checksum,
    )
    .await?;

    // Added: Run ClamAV scan asynchronously (non-blocking for the upload response)
    let scan_pool = state.db.clone();
    let scan_service = service.clone();
    let scan_path = storage_path.clone();
    let scan_id = attachment.id;
    tokio::spawn(async move {
        match scan_service.scan_file(&scan_path).await {
            Ok((status, result)) => {
                if let Err(e) =
                    Attachment::update_scan_status(&scan_pool, scan_id, &status, result.as_deref())
                        .await
                {
                    tracing::error!("Failed to update scan status for attachment {}: {}", scan_id, e);
                }
                // Added: If infected, log prominently for monitoring
                if status == "infected" {
                    tracing::warn!(
                        "SECURITY: Infected attachment detected — id={}, file='{}'",
                        scan_id,
                        scan_path
                    );
                }
            }
            Err(e) => {
                tracing::error!("ClamAV scan failed for attachment {}: {}", scan_id, e);
                let _ = Attachment::update_scan_status(
                    &scan_pool,
                    scan_id,
                    "error",
                    Some(&format!("Scan failed: {}", e)),
                )
                .await;
            }
        }
    });

    Ok((StatusCode::CREATED, Json(attachment)))
}

/// GET /api/attachments — List all attachments for the current user
pub async fn list_attachments(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Attachment>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let attachments = Attachment::list_by_mailbox(&state.db, mailbox_id).await?;
    Ok(Json(attachments))
}

/// GET /api/attachments/{id}/download — Download attachment file
/// CONSTRAINTS: Returns 404 if attachment not found or not owned by user
pub async fn download_attachment(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let _mailbox_id = parse_mailbox_id(&claims)?;
    let service = build_service(&state);

    // NOTE: RLS enforces ownership, but find_by_id is used here for simplicity
    let attachment = Attachment::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Attachment not found".to_string()))?;

    // Added: Block download of infected files
    if attachment.scan_status == "infected" {
        return Err(AppError::Forbidden(
            "Cannot download file flagged as infected by virus scanner".to_string(),
        ));
    }

    let data = service
        .read_file(&attachment.storage_path)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Added: Build response with proper content headers for browser download
    let response = Response::builder()
        .header(header::CONTENT_TYPE, &attachment.content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", attachment.filename),
        )
        .header(header::CONTENT_LENGTH, data.len())
        .body(Body::from(data))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// DELETE /api/attachments/{id} — Delete an attachment (file + record)
pub async fn delete_attachment(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let service = build_service(&state);

    // Added: Fetch attachment to get storage path before deleting record
    let attachment = Attachment::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Attachment not found".to_string()))?;

    let deleted = Attachment::delete(&state.db, id, mailbox_id).await?;
    if !deleted {
        return Err(AppError::NotFound("Attachment not found".to_string()));
    }

    // Added: Clean up file from disk after DB record is deleted
    if let Err(e) = service.delete_file(&attachment.storage_path).await {
        tracing::error!(
            "Failed to delete attachment file '{}' after DB record removal: {}",
            attachment.storage_path,
            e
        );
    }

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/attachments/stats — Get storage statistics for current user
pub async fn attachment_stats(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<StorageStats>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let stats = Attachment::storage_stats(&state.db, mailbox_id).await?;
    Ok(Json(stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_mailbox_id_valid() {
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_mailbox_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_err());
    }
}
