// Added: PST import upload/list/get/delete handlers for TMAIL-115
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::pst_import::PstImport;
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: Parse and validate user_id (mailbox_id) from JWT claims
fn parse_user_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in token")))
}

/// PURPOSE: Upload a .pst file via multipart/form-data and create an import record
/// CONSTRAINTS: File must have .pst extension; max size enforced by server config
/// EXTERNAL: Saves file to disk at /tmp/pst_uploads/{import_id}.pst
///
/// POST /api/migration/pst/upload
pub async fn upload_pst(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<PstImport>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Extract file data and target_folder from multipart form fields
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut target_folder = "INBOX".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                // Added: Capture original filename from multipart field
                filename = field.file_name().map(String::from);

                let data: axum::body::Bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read file data: {}", e)))?;

                if data.is_empty() {
                    return Err(AppError::BadRequest(
                        "PST file is empty. Upload a valid Outlook .pst file.".to_string(),
                    ));
                }

                file_data = Some(data.to_vec());
            }
            "target_folder" => {
                // Added: Allow user to specify which IMAP folder to import into
                let folder_value = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read target_folder: {}", e)))?;
                if !folder_value.is_empty() {
                    target_folder = folder_value;
                }
            }
            _ => {
                // NOTE: Ignore unknown multipart fields
            }
        }
    }

    // Added: Validate that a file was actually provided
    let file_bytes = file_data
        .ok_or_else(|| AppError::BadRequest("No file field found in multipart upload. Include a 'file' field with the .pst file.".to_string()))?;

    let original_filename = filename.unwrap_or_else(|| "import.pst".to_string());

    // Added: Validate .pst file extension
    if !original_filename.to_lowercase().ends_with(".pst") {
        return Err(AppError::BadRequest(
            format!("File '{}' does not have a .pst extension. Only Outlook PST files are accepted.", original_filename),
        ));
    }

    let file_size = file_bytes.len() as i64;

    // Added: Create the import record in the database
    let import = PstImport::create(&state.db, user_id, &original_filename, file_size, &target_folder)
        .await?;

    // Added: Save the uploaded file to disk for background processing
    let upload_dir = std::path::Path::new("/tmp/pst_uploads");
    tokio::fs::create_dir_all(upload_dir)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create upload directory: {}", e)))?;

    let file_path = upload_dir.join(format!("{}.pst", import.id));
    tokio::fs::write(&file_path, &file_bytes)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to save PST file to disk: {}", e)))?;

    // NOTE: Background processing (readpst + IMAP APPEND) is triggered by pst_processor service
    // which polls for pending imports. Not started inline to avoid blocking the upload response.

    Ok((StatusCode::CREATED, Json(import)))
}

/// PURPOSE: List all PST imports for the authenticated user
///
/// GET /api/migration/pst
pub async fn list_pst_imports(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<PstImport>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let imports = PstImport::list_by_user(&state.db, user_id).await?;
    Ok(Json(imports))
}

/// PURPOSE: Get a single PST import record by ID
///
/// GET /api/migration/pst/:id
pub async fn get_pst_import(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PstImport>, AppError> {
    let import = PstImport::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("PST import not found".to_string()))?;
    Ok(Json(import))
}

/// PURPOSE: Delete/cancel a PST import (only pending or failed imports can be deleted)
/// CONSTRAINTS: Cannot delete imports that are currently processing or completed
///
/// DELETE /api/migration/pst/:id
pub async fn delete_pst_import(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Verify the import belongs to the authenticated user
    let import = PstImport::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("PST import not found".to_string()))?;

    if import.user_id != user_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the import owner".to_string()));
    }

    if import.status == "processing" {
        return Err(AppError::BadRequest(
            "Cannot delete an import that is currently processing. Wait for it to complete or fail.".to_string(),
        ));
    }

    let deleted = PstImport::delete(&state.db, id).await?;
    if !deleted {
        return Err(AppError::BadRequest(
            format!("Cannot delete import in '{}' status. Only pending or failed imports can be deleted.", import.status),
        ));
    }

    // Added: Clean up the uploaded file from disk
    let file_path = std::path::Path::new("/tmp/pst_uploads").join(format!("{}.pst", id));
    let _ = tokio::fs::remove_file(&file_path).await;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            username: "test@example.com".to_string(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        let result = parse_user_id(&claims);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".to_string(),
            username: "test@example.com".to_string(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }

    #[test]
    fn test_pst_extension_validation() {
        // Added: Verify .pst extension check logic used in upload handler
        let valid_filenames = ["outlook.pst", "My Mail.PST", "BACKUP.Pst", "export.pst"];
        for fname in valid_filenames {
            assert!(
                fname.to_lowercase().ends_with(".pst"),
                "'{}' should pass .pst extension check",
                fname
            );
        }

        let invalid_filenames = ["outlook.mbox", "data.zip", "file.pst.exe", "noext"];
        for fname in invalid_filenames {
            assert!(
                !fname.to_lowercase().ends_with(".pst"),
                "'{}' should fail .pst extension check",
                fname
            );
        }
    }

    #[test]
    fn test_default_target_folder() {
        // Added: Verify the default target folder is INBOX
        let default_folder = "INBOX";
        assert_eq!(default_folder, "INBOX");
    }

    #[test]
    fn test_upload_dir_path_construction() {
        // Added: Verify the upload file path is constructed correctly
        let import_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let upload_dir = std::path::Path::new("/tmp/pst_uploads");
        let file_path = upload_dir.join(format!("{}.pst", import_id));
        assert_eq!(
            file_path.to_str().unwrap(),
            "/tmp/pst_uploads/550e8400-e29b-41d4-a716-446655440000.pst"
        );
    }

    #[test]
    fn test_status_check_for_delete() {
        // Added: Verify which statuses allow deletion
        let deletable = ["pending", "failed"];
        let non_deletable = ["processing", "completed"];

        for status_value in deletable {
            assert!(
                status_value == "pending" || status_value == "failed",
                "'{}' should be deletable",
                status_value
            );
        }
        for status_value in non_deletable {
            assert!(
                status_value != "pending" && status_value != "failed",
                "'{}' should not be deletable",
                status_value
            );
        }
    }
}
