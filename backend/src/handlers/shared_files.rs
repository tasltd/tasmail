// Added: Shared file upload/download/list/delete handlers for TMAIL-138
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::shared_file::{generate_download_token, SharedFile, SharedFilePublicInfo};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: Parse and validate user_id from JWT claims
fn parse_user_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in token")))
}

/// PURPOSE: Build the shared files storage directory path from app config
fn shared_files_dir(state: &AppState) -> PathBuf {
    // NOTE: Store shared files alongside attachments in a sibling directory
    let base = PathBuf::from(&state.config.storage.attachment_dir);
    base.parent()
        .unwrap_or_else(|| std::path::Path::new("./data"))
        .join("shared-files")
}

/// POST /api/shared-files/upload — Upload a file and generate a shareable download link
/// CONSTRAINTS: Max file size enforced by config; multipart form must include a 'file' field
pub async fn upload_shared_file(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SharedFile>), AppError> {
    let user_id = parse_user_id(&claims)?;
    let max_size = state.config.storage.max_file_size;

    // Added: Extract file data and optional metadata from multipart form
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type = "application/octet-stream".to_string();
    let mut max_downloads: Option<i32> = None;
    let mut expires_at: Option<String> = None;
    let mut password: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
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
            "max_downloads" => {
                let text = field.text().await.unwrap_or_default();
                if !text.is_empty() {
                    max_downloads = text.parse().ok();
                }
            }
            "expires_at" => {
                let text = field.text().await.unwrap_or_default();
                if !text.is_empty() {
                    expires_at = Some(text);
                }
            }
            "password" => {
                let text = field.text().await.unwrap_or_default();
                if !text.is_empty() {
                    password = Some(text);
                }
            }
            _ => {}
        }
    }

    let data = file_data.ok_or_else(|| {
        AppError::BadRequest("No file field found in multipart upload. Include a 'file' field.".to_string())
    })?;
    let filename = filename.unwrap_or_else(|| "unnamed".to_string());
    let file_size = data.len() as i64;

    // Added: Generate unique download token for public access
    let download_token = generate_download_token();

    // Added: Parse optional expiry timestamp
    let parsed_expires_at = if let Some(expires_str) = expires_at {
        Some(
            chrono::DateTime::parse_from_rfc3339(&expires_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| {
                    AppError::BadRequest(format!(
                        "Invalid expires_at format '{}'. Use RFC 3339 (e.g., '2026-12-31T23:59:59Z'): {}",
                        expires_str, e
                    ))
                })?,
        )
    } else {
        None
    };

    // Added: Hash password if provided using Argon2id
    let password_hash = if let Some(ref pwd) = password {
        use argon2::password_hash::{rand_core::OsRng, SaltString};
        use argon2::{Argon2, PasswordHasher};
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(pwd.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to hash password: {}", e)))?
            .to_string();
        Some(hash)
    } else {
        None
    };

    // Added: Store file to disk in user-specific subdirectory
    let storage_dir = shared_files_dir(&state).join(user_id.to_string());
    tokio::fs::create_dir_all(&storage_dir)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create storage directory: {}", e)))?;

    let storage_filename = format!("{}_{}", Uuid::new_v4(), &filename);
    let storage_path = storage_dir.join(&storage_filename);
    tokio::fs::write(&storage_path, &data)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to write file to disk: {}", e)))?;

    let storage_path_str = storage_path
        .to_str()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Invalid storage path encoding")))?
        .to_string();

    // Added: Create database record
    let shared_file = SharedFile::create(
        &state.db,
        user_id,
        &filename,
        &content_type,
        file_size,
        &storage_path_str,
        &download_token,
        max_downloads,
        parsed_expires_at,
        password_hash.as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(shared_file)))
}

/// GET /api/shared-files — List all shared files for the current user
pub async fn list_shared_files(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<SharedFile>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let files = SharedFile::list_by_user(&state.db, user_id).await?;
    Ok(Json(files))
}

/// GET /api/shared-files/:id — Get details of a specific shared file
pub async fn get_shared_file(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<SharedFile>, AppError> {
    let _user_id = parse_user_id(&claims)?;
    // NOTE: RLS enforces ownership — only the owner can see their files
    let shared_file = SharedFile::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Shared file not found".to_string()))?;
    Ok(Json(shared_file))
}

/// DELETE /api/shared-files/:id — Delete a shared file (record + disk file)
pub async fn delete_shared_file(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Fetch file to get storage path before deleting record
    let shared_file = SharedFile::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Shared file not found".to_string()))?;

    let deleted = SharedFile::delete(&state.db, id, user_id).await?;
    if !deleted {
        return Err(AppError::NotFound("Shared file not found".to_string()));
    }

    // Added: Clean up file from disk after DB record is deleted
    if let Err(e) = tokio::fs::remove_file(&shared_file.storage_path).await {
        tracing::error!(
            "Failed to delete shared file '{}' after DB record removal: {}",
            shared_file.storage_path,
            e
        );
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Added: Query params for public download endpoint
#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub password: Option<String>,
}

/// GET /api/dl/:token — Public download endpoint (no auth required)
/// CONSTRAINTS: Checks expiry, max downloads, and optional password before serving file
pub async fn download_by_token(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, AppError> {
    // NOTE: This query bypasses RLS since it's a public endpoint without auth context
    let shared_file = SharedFile::find_by_token(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found or link is invalid".to_string()))?;

    // Added: Check if file has expired (time or download count)
    if shared_file.is_expired() {
        return Err(AppError::BadRequest(
            "This download link has expired".to_string(),
        ));
    }

    // Added: Verify password if required
    if shared_file.requires_password() {
        let provided_password = query.password.as_deref().ok_or_else(|| {
            AppError::Unauthorized(
                "This file requires a password. Provide it as ?password=<value>".to_string(),
            )
        })?;

        use argon2::{Argon2, PasswordHash, PasswordVerifier};
        let stored_hash = shared_file.password_hash.as_ref().unwrap();
        let parsed_hash = PasswordHash::new(stored_hash)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Invalid password hash in DB: {}", e)))?;

        Argon2::default()
            .verify_password(provided_password.as_bytes(), &parsed_hash)
            .map_err(|_| AppError::Unauthorized("Incorrect password".to_string()))?;
    }

    // Added: Read file from disk
    let data = tokio::fs::read(&shared_file.storage_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to read shared file '{}': {}",
            shared_file.storage_path,
            e
        ))
    })?;

    // Added: Increment download count after successful read
    SharedFile::increment_download_count(&state.db, shared_file.id).await?;

    // Added: Build response with proper content headers for browser download
    let response = Response::builder()
        .header(header::CONTENT_TYPE, &shared_file.content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", shared_file.filename),
        )
        .header(header::CONTENT_LENGTH, data.len())
        .body(Body::from(data))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to build response: {}", e)))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::shared_file::generate_download_token;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }

    #[test]
    fn test_download_token_uniqueness() {
        let token1 = generate_download_token();
        let token2 = generate_download_token();
        assert_ne!(token1, token2);
        // NOTE: Tokens are 64 hex chars (32 bytes)
        assert_eq!(token1.len(), 64);
    }

    #[test]
    fn test_download_token_is_url_safe() {
        let token = generate_download_token();
        // NOTE: Hex encoding produces only [0-9a-f] — always URL-safe
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_expiry_check_no_limits() {
        let file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            file_size: 100,
            storage_path: "/tmp/test.txt".to_string(),
            download_token: "tok".to_string(),
            download_count: 0,
            max_downloads: None,
            expires_at: None,
            password_hash: None,
            created_at: chrono::Utc::now(),
        };
        // NOTE: No limits set — should never expire
        assert!(!file.is_expired());
    }

    #[test]
    fn test_expiry_check_time_expired() {
        let file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            file_size: 100,
            storage_path: "/tmp/test.txt".to_string(),
            download_token: "tok".to_string(),
            download_count: 0,
            max_downloads: None,
            expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
            password_hash: None,
            created_at: chrono::Utc::now(),
        };
        assert!(file.is_expired());
    }

    #[test]
    fn test_expiry_check_downloads_exceeded() {
        let file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            file_size: 100,
            storage_path: "/tmp/test.txt".to_string(),
            download_token: "tok".to_string(),
            download_count: 5,
            max_downloads: Some(5),
            expires_at: None,
            password_hash: None,
            created_at: chrono::Utc::now(),
        };
        assert!(file.is_expired());
    }

    #[test]
    fn test_password_requirement_detection() {
        let mut file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "secret.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            file_size: 1024,
            storage_path: "/tmp/secret.pdf".to_string(),
            download_token: "tok".to_string(),
            download_count: 0,
            max_downloads: None,
            expires_at: None,
            password_hash: None,
            created_at: chrono::Utc::now(),
        };

        assert!(!file.requires_password());
        file.password_hash = Some("$argon2id$hash".to_string());
        assert!(file.requires_password());
    }
}
