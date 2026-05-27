// Added: Attachment upload/download/list/delete/stats handlers for TMAIL-59
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::attachment::{Attachment, StorageStats};
use crate::models::mailbox::Mailbox;
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

    // Added (TMAIL-59 gap): Enforce per-mailbox storage quota BEFORE touching the disk.
    // Attachments count toward the mailbox's quota_bytes budget per the spec.
    // quota_bytes <= 0 is treated as "unlimited / not configured" — admin-created
    // mailboxes always seed a positive default, so this only matters for legacy rows.
    let mailbox = Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Mailbox not found".to_string()))?;

    if mailbox.quota_bytes > 0 {
        let used = Attachment::total_size_for_mailbox(&state.db, mailbox_id).await?;
        if would_exceed_quota(used, size_bytes, mailbox.quota_bytes) {
            return Err(AppError::BadRequest(format!(
                "Attachment would exceed mailbox quota: {} used + {} new > {} allowed",
                used, size_bytes, mailbox.quota_bytes
            )));
        }
    }

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
/// CONSTRAINTS: Returns 404 if attachment not found or not owned by user.
/// Supports HTTP Range requests (RFC 7233) so large attachments can be streamed
/// in chunks instead of buffered fully in memory — important for the 25 MB limit
/// and for resumable downloads on flaky mobile networks.
pub async fn download_attachment(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    headers: HeaderMap,
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

    // Added: Look up the on-disk size once so we can build correct Content-Range
    // headers and validate any Range request before reading bytes.
    let total_size = service
        .file_size(&attachment.storage_path)
        .await
        .map_err(|e| AppError::Internal(e))?;

    let range_header = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Added: If the client sent a Range header, serve 206 Partial Content from disk
    // using a bounded seek+read instead of loading the whole file into memory.
    if let Some(raw_range) = range_header {
        match parse_byte_range(&raw_range, total_size) {
            Ok((start, end)) => {
                let chunk = service
                    .read_file_range(&attachment.storage_path, start, end)
                    .await
                    .map_err(|e| AppError::Internal(e))?;

                let response = Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, &attachment.content_type)
                    .header(
                        header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{}\"", attachment.filename),
                    )
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(
                        header::CONTENT_RANGE,
                        format!("bytes {}-{}/{}", start, end, total_size),
                    )
                    .header(header::CONTENT_LENGTH, chunk.len())
                    .body(Body::from(chunk))
                    .map_err(|e| {
                        AppError::Internal(anyhow::anyhow!("Failed to build response: {}", e))
                    })?;
                return Ok(response);
            }
            Err(RangeError::Unsatisfiable) => {
                // NOTE: RFC 7233 §4.4 — 416 must include Content-Range with the
                // representation's complete length so clients can recover.
                let response = Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{}", total_size))
                    .body(Body::empty())
                    .map_err(|e| {
                        AppError::Internal(anyhow::anyhow!("Failed to build response: {}", e))
                    })?;
                return Ok(response);
            }
            // NOTE: Malformed Range headers are ignored per RFC 7233 §3.1 — fall
            // through to a normal 200 response.
            Err(RangeError::Malformed) => {}
        }
    }

    let data = service
        .read_file(&attachment.storage_path)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Added: Build response with proper content headers for browser download.
    // Accept-Ranges advertises Range support so clients (and reverse proxies) know
    // they can issue partial requests on retry.
    let response = Response::builder()
        .header(header::CONTENT_TYPE, &attachment.content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", attachment.filename),
        )
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, data.len())
        .body(Body::from(data))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// PURPOSE: Categorise Range-header failures so the caller can decide between 416
/// (the range is well-formed but outside the representation) and a normal 200
/// (the header was malformed and per RFC 7233 §3.1 should be ignored).
#[derive(Debug, PartialEq, Eq)]
enum RangeError {
    Malformed,
    Unsatisfiable,
}

/// PURPOSE: Parse a single-range `Range: bytes=START-END` header against the file's
/// total size. Returns the resolved inclusive byte offsets.
///
/// Supports:
///   - `bytes=0-99`         → first 100 bytes
///   - `bytes=100-`         → from byte 100 to end-of-file
///   - `bytes=-500`         → last 500 bytes (suffix range)
///
/// Multi-range requests (`bytes=0-10,20-30`) are intentionally not supported —
/// returning the first range as a 206 is RFC-compliant and avoids the
/// multipart/byteranges complexity that no mail client actually needs.
fn parse_byte_range(value: &str, total_size: u64) -> Result<(u64, u64), RangeError> {
    let spec = value.strip_prefix("bytes=").ok_or(RangeError::Malformed)?;
    // NOTE: Take the first range only — multipart byteranges are out of scope.
    let first = spec.split(',').next().ok_or(RangeError::Malformed)?.trim();

    let (start_str, end_str) = first.split_once('-').ok_or(RangeError::Malformed)?;

    if total_size == 0 {
        // NOTE: Any byte range against an empty representation is unsatisfiable.
        return Err(RangeError::Unsatisfiable);
    }
    let last_byte = total_size - 1;

    let (start, end) = match (start_str.trim(), end_str.trim()) {
        ("", "") => return Err(RangeError::Malformed),
        // Suffix range: last N bytes
        ("", suffix) => {
            let n: u64 = suffix.parse().map_err(|_| RangeError::Malformed)?;
            if n == 0 {
                return Err(RangeError::Unsatisfiable);
            }
            let n = n.min(total_size);
            (total_size - n, last_byte)
        }
        // Open-ended: from start to EOF
        (start, "") => {
            let s: u64 = start.parse().map_err(|_| RangeError::Malformed)?;
            (s, last_byte)
        }
        // Closed: start to end (clamped to last byte)
        (start, end) => {
            let s: u64 = start.parse().map_err(|_| RangeError::Malformed)?;
            let e: u64 = end.parse().map_err(|_| RangeError::Malformed)?;
            (s, e.min(last_byte))
        }
    };

    if start > last_byte || end < start {
        return Err(RangeError::Unsatisfiable);
    }
    Ok((start, end))
}

/// PURPOSE: Pure check used by upload_attachment to decide whether adding a new
/// attachment would push the mailbox above its storage quota.
/// CONSTRAINTS: Callers must already have filtered out the unlimited case
/// (`quota_bytes <= 0`); this helper assumes the quota is enforced.
/// Saturating arithmetic prevents i64 overflow on absurdly-sized inputs.
fn would_exceed_quota(used: i64, incoming: i64, quota_bytes: i64) -> bool {
    used.saturating_add(incoming) > quota_bytes
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
            is_compliance_officer: false,
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
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_err());
    }

    #[test]
    fn test_parse_byte_range_closed() {
        assert_eq!(parse_byte_range("bytes=0-99", 1000), Ok((0, 99)));
        assert_eq!(parse_byte_range("bytes=200-299", 1000), Ok((200, 299)));
    }

    #[test]
    fn test_parse_byte_range_end_clamped_to_last_byte() {
        // Added: requested end beyond EOF is clamped to last byte, NOT 416
        assert_eq!(parse_byte_range("bytes=0-99999", 1000), Ok((0, 999)));
    }

    #[test]
    fn test_parse_byte_range_open_ended() {
        // Added: bytes=100- means from 100 to EOF
        assert_eq!(parse_byte_range("bytes=100-", 1000), Ok((100, 999)));
    }

    #[test]
    fn test_parse_byte_range_suffix() {
        // Added: bytes=-200 means last 200 bytes
        assert_eq!(parse_byte_range("bytes=-200", 1000), Ok((800, 999)));
    }

    #[test]
    fn test_parse_byte_range_suffix_larger_than_file() {
        // Added: suffix range bigger than file returns full file
        assert_eq!(parse_byte_range("bytes=-5000", 1000), Ok((0, 999)));
    }

    #[test]
    fn test_parse_byte_range_first_of_multi_range() {
        // NOTE: Only the first range of a multi-range request is honored
        assert_eq!(parse_byte_range("bytes=0-10,20-30", 1000), Ok((0, 10)));
    }

    #[test]
    fn test_parse_byte_range_start_past_eof_is_unsatisfiable() {
        assert_eq!(
            parse_byte_range("bytes=2000-3000", 1000),
            Err(RangeError::Unsatisfiable)
        );
    }

    #[test]
    fn test_parse_byte_range_empty_file_is_unsatisfiable() {
        assert_eq!(
            parse_byte_range("bytes=0-0", 0),
            Err(RangeError::Unsatisfiable)
        );
    }

    #[test]
    fn test_parse_byte_range_malformed() {
        assert_eq!(parse_byte_range("0-99", 1000), Err(RangeError::Malformed));
        assert_eq!(parse_byte_range("bytes=abc", 1000), Err(RangeError::Malformed));
        assert_eq!(parse_byte_range("bytes=-", 1000), Err(RangeError::Malformed));
        assert_eq!(parse_byte_range("bytes=abc-xyz", 1000), Err(RangeError::Malformed));
    }

    #[test]
    fn test_parse_byte_range_zero_length_suffix_is_unsatisfiable() {
        // Added: bytes=-0 has no defined semantic; treat as unsatisfiable
        assert_eq!(
            parse_byte_range("bytes=-0", 1000),
            Err(RangeError::Unsatisfiable)
        );
    }

    // Added (TMAIL-59 gap fix): per-mailbox attachment quota enforcement
    #[test]
    fn test_would_exceed_quota_under_limit() {
        // 100 used + 50 new = 150 ≤ 200 quota → allowed
        assert!(!would_exceed_quota(100, 50, 200));
    }

    #[test]
    fn test_would_exceed_quota_exact_fit() {
        // Exact fit must be allowed — the boundary belongs to the user
        assert!(!would_exceed_quota(150, 50, 200));
    }

    #[test]
    fn test_would_exceed_quota_over_limit() {
        // 150 used + 100 new = 250 > 200 quota → rejected
        assert!(would_exceed_quota(150, 100, 200));
    }

    #[test]
    fn test_would_exceed_quota_empty_mailbox_first_upload_too_big() {
        // First upload that already exceeds the quota must be rejected
        assert!(would_exceed_quota(0, 300, 200));
    }

    #[test]
    fn test_would_exceed_quota_saturates_on_overflow() {
        // Plain `used + incoming` would wrap to a negative i64 here and
        // incorrectly report "under quota". saturating_add pins the result
        // to i64::MAX so the comparison still rejects the upload.
        let used = i64::MAX - 5;
        let incoming = 100;
        let quota = i64::MAX - 10;
        assert!(would_exceed_quota(used, incoming, quota));
    }
}
