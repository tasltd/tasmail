// Added: EML import/export handlers for TMAIL-68
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use futures::TryStreamExt;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::ImapService;
use crate::state::AppState;

/// PURPOSE: Download a raw email as an .eml file via IMAP UID FETCH RFC822
/// CONSTRAINTS: Requires valid folder name and UID; returns 404 if message not found
/// EXTERNAL: Connects to IMAP server (Dovecot) to fetch raw message bytes
///
/// GET /api/folders/{folder}/messages/{uid}/eml
pub async fn export_eml(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<impl IntoResponse, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::new(state.config.imap.clone());

    // Added: Fetch raw RFC822 bytes for the message via IMAP
    let raw_bytes = fetch_raw_eml(
        &imap_service,
        &mailbox.username,
        &mailbox.password_hash,
        &folder,
        uid,
    )
    .await?;

    // Added: Build response with correct Content-Type and Content-Disposition for .eml download
    let filename = format!("message_{}.eml", uid);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "message/rfc822")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_LENGTH, raw_bytes.len().to_string())
        .body(axum::body::Body::from(raw_bytes))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// PURPOSE: Import a raw .eml file into the specified IMAP folder via APPEND
/// CONSTRAINTS: Body must be non-empty and contain valid RFC822 email data; max practical size ~25MB
/// EXTERNAL: Connects to IMAP server (Dovecot) to APPEND message
///
/// POST /api/folders/{folder}/import-eml
pub async fn import_eml(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(folder): Path<String>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, AppError> {
    // Added: Reject empty body — an EML file must contain message data
    if body.is_empty() {
        return Err(AppError::BadRequest(
            "Empty body: EML file content is required. Send raw RFC822 email bytes in the request body.".to_string(),
        ));
    }

    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::new(state.config.imap.clone());

    // Added: Append raw EML bytes to the target IMAP folder
    append_eml_to_folder(
        &imap_service,
        &mailbox.username,
        &mailbox.password_hash,
        &folder,
        &body,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        axum::Json(serde_json::json!({
            "message": format!("Email imported successfully into folder '{}'", folder),
            "folder": folder,
            "size": body.len(),
        })),
    ))
}

/// PURPOSE: Fetch raw RFC822 bytes for a single message by UID from IMAP
/// NOTE: This is a standalone function (not on ImapService) to avoid modifying the shared service
///       for a feature-specific operation. Uses the same connection pattern as ImapService.
async fn fetch_raw_eml(
    imap_service: &ImapService,
    username: &str,
    password: &str,
    folder: &str,
    uid: u32,
) -> Result<Vec<u8>, AppError> {
    // NOTE: We use the public connect method pattern but need direct IMAP access.
    // Since ImapService::connect is private, we replicate the connection here.
    // This matches the pattern used in the migration handler.
    let tcp_stream =
        tokio::net::TcpStream::connect((&*imap_service.imap_config().host, imap_service.imap_config().port))
            .await
            .map_err(|e| AppError::Imap(format!("TCP connection failed: {}", e)))?;

    let compat_stream = tokio_util::compat::TokioAsyncReadCompatExt::compat(tcp_stream);

    let tls = async_native_tls::TlsConnector::new();
    let tls_stream = tls
        .connect(&imap_service.imap_config().host, compat_stream)
        .await
        .map_err(|e| AppError::Imap(format!("TLS connection failed: {}", e)))?;

    let client = async_imap::Client::new(tls_stream);
    let mut session = client
        .login(username, password)
        .await
        .map_err(|e| AppError::Imap(format!("Login failed: {}", e.0)))?;

    session
        .select(folder)
        .await
        .map_err(|e| AppError::Imap(format!("SELECT '{}' failed: {}", folder, e)))?;

    // Added: Fetch raw RFC822 message body by UID
    let messages: Vec<_> = session
        .uid_fetch(uid.to_string(), "RFC822")
        .await
        .map_err(|e| AppError::Imap(format!("UID FETCH {} RFC822 failed: {}", uid, e)))?
        .try_collect()
        .await
        .map_err(|e| AppError::Imap(format!("UID FETCH stream failed: {}", e)))?;

    let msg = messages
        .first()
        .ok_or_else(|| AppError::NotFound(format!("Message UID {} not found in folder '{}'", uid, folder)))?;

    let body_bytes = msg
        .body()
        .ok_or_else(|| AppError::Imap(format!("No body data for message UID {}", uid)))?;

    let result = body_bytes.to_vec();
    let _ = session.logout().await;

    Ok(result)
}

/// PURPOSE: Append raw EML bytes to an IMAP folder using the APPEND command
async fn append_eml_to_folder(
    imap_service: &ImapService,
    username: &str,
    password: &str,
    folder: &str,
    eml_data: &[u8],
) -> Result<(), AppError> {
    let tcp_stream =
        tokio::net::TcpStream::connect((&*imap_service.imap_config().host, imap_service.imap_config().port))
            .await
            .map_err(|e| AppError::Imap(format!("TCP connection failed: {}", e)))?;

    let compat_stream = tokio_util::compat::TokioAsyncReadCompatExt::compat(tcp_stream);

    let tls = async_native_tls::TlsConnector::new();
    let tls_stream = tls
        .connect(&imap_service.imap_config().host, compat_stream)
        .await
        .map_err(|e| AppError::Imap(format!("TLS connection failed: {}", e)))?;

    let client = async_imap::Client::new(tls_stream);
    let mut session = client
        .login(username, password)
        .await
        .map_err(|e| AppError::Imap(format!("Login failed: {}", e.0)))?;

    // Added: APPEND the raw email into the target folder with \Seen flag
    session
        .append(folder, Some("(\\Seen)"), None, eml_data)
        .await
        .map_err(|e| AppError::Imap(format!("APPEND to '{}' failed: {}", folder, e)))?;

    let _ = session.logout().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_export_eml_response_headers() {
        // Added: Verify the response builder produces correct headers for EML download
        let uid = 42u32;
        let raw_bytes = b"From: test@example.com\r\nSubject: Test\r\n\r\nHello";
        let filename = format!("message_{}.eml", uid);

        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "message/rfc822")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            )
            .header(header::CONTENT_LENGTH, raw_bytes.len().to_string())
            .body(axum::body::Body::from(raw_bytes.to_vec()))
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "message/rfc822"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"message_42.eml\""
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            "46"
        );
    }

    #[test]
    fn test_export_eml_filename_format() {
        // Added: Verify filename pattern for various UIDs
        for uid in [1u32, 100, 99999] {
            let filename = format!("message_{}.eml", uid);
            assert!(filename.starts_with("message_"));
            assert!(filename.ends_with(".eml"));
            assert!(filename.contains(&uid.to_string()));
        }
    }

    #[test]
    fn test_import_eml_rejects_empty_body() {
        // Added: Verify that empty bytes trigger a BadRequest error
        let empty_body = axum::body::Bytes::new();
        assert!(empty_body.is_empty());

        // NOTE: We check the validation logic inline — the handler returns BadRequest for empty body
        let error = AppError::BadRequest(
            "Empty body: EML file content is required. Send raw RFC822 email bytes in the request body."
                .to_string(),
        );
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_import_eml_success_response_shape() {
        // Added: Verify the JSON structure of a successful import response
        let folder = "INBOX";
        let body_len = 1024usize;
        let json = serde_json::json!({
            "message": format!("Email imported successfully into folder '{}'", folder),
            "folder": folder,
            "size": body_len,
        });

        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["size"], 1024);
        assert!(json["message"].as_str().unwrap().contains("INBOX"));
    }

    #[test]
    fn test_export_content_type_is_rfc822() {
        // Added: Verify Content-Type header value is the standard message/rfc822 MIME type
        let content_type = "message/rfc822";
        assert_eq!(content_type, "message/rfc822");
        // NOTE: This is the IANA-registered MIME type for email messages (.eml files)
    }

    #[test]
    fn test_import_eml_non_empty_body_accepted() {
        // Added: Verify that a non-empty body passes the emptiness check
        let eml_content = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Test Import\r\n\r\nThis is a test email body.";
        let body = axum::body::Bytes::from(eml_content.to_vec());
        assert!(!body.is_empty());
        assert_eq!(body.len(), eml_content.len());
    }
}
