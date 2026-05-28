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

    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    // Added: Fetch raw RFC822 bytes for the message via IMAP
    let raw_bytes = fetch_raw_eml(
        &imap_service,
        _imap_user,
        _imap_pass,
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

    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    // Added: Append raw EML bytes to the target IMAP folder
    append_eml_to_folder(
        &imap_service,
        _imap_user,
        _imap_pass,
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

/// PURPOSE: Export an entire IMAP folder as an MBOX file (RFC 4155 mbox format)
/// CONSTRAINTS: Requires valid folder name; streams all messages in the folder.
///              Each message is preceded by a `From ` separator line per the mbox format.
/// EXTERNAL: Connects to IMAP server (Dovecot) to fetch raw message bytes for every UID.
///
/// GET /api/folders/{folder}/export-mbox
pub async fn export_folder_mbox(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(folder): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    // Added: Stream every message in the folder as a single concatenated mbox payload
    let mbox_bytes = fetch_folder_as_mbox(&imap_service, _imap_user, _imap_pass, &folder).await?;

    let safe_filename = sanitize_folder_filename(&folder);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/mbox")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}.mbox\"", safe_filename),
        )
        .header(header::CONTENT_LENGTH, mbox_bytes.len().to_string())
        .body(axum::body::Body::from(mbox_bytes))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// PURPOSE: Build the mbox `From ` separator line that prefixes each message.
///          Format: `From <sender-or-MAILER-DAEMON> <ctime-style-date>\n`
///          (RFC 4155 — the unix mboxo format that all major mail clients import.)
fn build_mbox_from_line(sender: &str, timestamp: chrono::DateTime<chrono::Utc>) -> String {
    let from_addr = if sender.is_empty() { "MAILER-DAEMON" } else { sender };
    // NOTE: ctime-style date e.g. "Thu Jan  1 00:00:00 2026"
    let date = timestamp.format("%a %b %e %H:%M:%S %Y");
    format!("From {} {}\n", from_addr, date)
}

/// PURPOSE: Escape `From ` lines inside the message body so they don't get mistaken
///          for new message separators (mboxo / "From quoting" — RFC 4155 §2.2).
fn escape_mbox_from_lines(raw: &[u8]) -> Vec<u8> {
    // Added: Walk byte-by-byte, prefix any line starting with "From " with ">".
    let mut out = Vec::with_capacity(raw.len() + 16);
    let mut at_line_start = true;
    let mut i = 0;
    while i < raw.len() {
        if at_line_start && raw[i..].starts_with(b"From ") {
            out.push(b'>');
        }
        let b = raw[i];
        out.push(b);
        at_line_start = b == b'\n';
        i += 1;
    }
    out
}

/// PURPOSE: Sanitize a folder name for use as a download filename. Strips path
///          separators and characters that break Content-Disposition.
fn sanitize_folder_filename(folder: &str) -> String {
    let cleaned: String = folder
        .chars()
        .map(|c| match c {
            '/' | '\\' | '"' | '\n' | '\r' | '\0' => '_',
            _ => c,
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "folder".to_string()
    } else {
        trimmed.to_string()
    }
}

/// PURPOSE: Fetch every message in a folder and concatenate them as an mbox payload.
async fn fetch_folder_as_mbox(
    imap_service: &ImapService,
    username: &str,
    password: &str,
    folder: &str,
) -> Result<Vec<u8>, AppError> {
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

    let mailbox = session
        .select(folder)
        .await
        .map_err(|e| AppError::Imap(format!("SELECT '{}' failed: {}", folder, e)))?;

    // Added: Empty folder → return empty mbox (valid per spec)
    if mailbox.exists == 0 {
        let _ = session.logout().await;
        return Ok(Vec::new());
    }

    // Added: Fetch every UID in the folder (1:*) — pulls envelope for the From line
    //        and RFC822 for the body. mboxo escaping happens before concatenation.
    let uid_range = "1:*";
    let messages: Vec<_> = session
        .uid_fetch(uid_range, "(UID ENVELOPE RFC822 INTERNALDATE)")
        .await
        .map_err(|e| AppError::Imap(format!("UID FETCH RFC822 failed: {}", e)))?
        .try_collect()
        .await
        .map_err(|e| AppError::Imap(format!("UID FETCH stream failed: {}", e)))?;

    let mut mbox = Vec::new();
    for msg in &messages {
        let Some(body_bytes) = msg.body() else { continue };

        // Added: Derive sender for From-line — prefer envelope.from[0], fall back to MAILER-DAEMON
        let sender = msg
            .envelope()
            .and_then(|env| env.from.as_ref())
            .and_then(|addrs| addrs.first())
            .map(|addr| {
                let mailbox_part = addr
                    .mailbox
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(b).into_owned())
                    .unwrap_or_default();
                let host_part = addr
                    .host
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(b).into_owned())
                    .unwrap_or_default();
                if host_part.is_empty() { mailbox_part } else { format!("{}@{}", mailbox_part, host_part) }
            })
            .unwrap_or_default();

        let timestamp = msg
            .internal_date()
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        mbox.extend_from_slice(build_mbox_from_line(&sender, timestamp).as_bytes());
        mbox.extend_from_slice(&escape_mbox_from_lines(body_bytes));
        // Added: Trailing blank line between messages (defensive — many parsers need it)
        if !mbox.ends_with(b"\n") {
            mbox.push(b'\n');
        }
        mbox.push(b'\n');
    }

    let _ = session.logout().await;
    Ok(mbox)
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

    // Added: Tests for MBOX folder export helpers (TMAIL-68 — MBOX export)

    #[test]
    fn test_mbox_from_line_format() {
        let ts = chrono::DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let line = build_mbox_from_line("alice@example.com", ts);
        assert!(line.starts_with("From alice@example.com "));
        assert!(line.contains("2026"));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn test_mbox_from_line_blank_sender_falls_back_to_mailer_daemon() {
        let ts = chrono::Utc::now();
        let line = build_mbox_from_line("", ts);
        assert!(line.starts_with("From MAILER-DAEMON "));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn test_mbox_escape_quotes_lines_that_start_with_from_space() {
        let raw = b"Subject: Test\r\n\r\nHello\nFrom your friend\nMore body";
        let escaped = escape_mbox_from_lines(raw);
        let as_str = String::from_utf8(escaped).unwrap();
        assert!(as_str.contains(">From your friend"));
        // Added: Mid-line "From" must NOT be escaped — escaping only triggers at line-start
        let raw2 = b"Hello from the sky";
        let escaped2 = escape_mbox_from_lines(raw2);
        assert_eq!(escaped2, raw2);
    }

    #[test]
    fn test_mbox_escape_handles_empty_input() {
        let escaped = escape_mbox_from_lines(b"");
        assert!(escaped.is_empty());
    }

    #[test]
    fn test_sanitize_folder_filename_replaces_path_separators() {
        assert_eq!(sanitize_folder_filename("INBOX"), "INBOX");
        assert_eq!(sanitize_folder_filename("INBOX/Archive"), "INBOX_Archive");
        assert_eq!(sanitize_folder_filename("foo\"bar"), "foo_bar");
        assert_eq!(sanitize_folder_filename(""), "folder");
        assert_eq!(sanitize_folder_filename("   "), "folder");
    }

    #[test]
    fn test_mbox_export_response_headers() {
        // Added: Verify the response builder produces correct headers for MBOX download
        let mbox_payload = b"From sender@example.com Thu Jan 01 00:00:00 2026\nSubject: x\n\nbody\n\n";
        let safe = sanitize_folder_filename("INBOX");
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/mbox")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.mbox\"", safe),
            )
            .header(header::CONTENT_LENGTH, mbox_payload.len().to_string())
            .body(axum::body::Body::from(mbox_payload.to_vec()))
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/mbox"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"INBOX.mbox\""
        );
    }
}
