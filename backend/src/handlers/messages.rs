use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::webhook::WebhookEvent;
use crate::services::auth_service::Claims;
use crate::services::imap_service::{FullMessage, ImapService};
// Added (TMAIL-320): response types for streaming a single MIME part back to
// the browser as a binary download.
use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Response};
use crate::services::smtp_service::{SendRequest, SmtpService};
use crate::services::webhook_dispatcher;
use crate::state::AppState;
// Added: Input validation for message operations (TMAIL-37)
use crate::validation;

// Added: Fire-and-forget webhook dispatch (TMAIL-131). Spawned on the runtime so the request
// returns immediately even if downstream webhook receivers are slow or unreachable.
fn fire_webhook(state: &AppState, user_id: uuid::Uuid, event: WebhookEvent, data: serde_json::Value) {
    let db = state.db.clone();
    tokio::spawn(async move {
        webhook_dispatcher::dispatch_webhook_event(&db, user_id, event, data).await;
    });
}

#[derive(Debug, Deserialize)]
pub struct ListMessagesQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub folder: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MoveRequest {
    pub to_folder: String,
}

#[derive(Debug, Deserialize)]
pub struct FlagRequest {
    pub flag: String,
    pub add: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaveDraftRequest {
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub subject: String,
    pub html_body: Option<String>,
    pub text_body: Option<String>,
}

/// GET /api/folders/:folder/messages — list messages in a folder
pub async fn list_messages(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(folder): Path<String>,
    Query(query): Query<ListMessagesQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Added: Validate folder name and cap page_size to prevent abuse (TMAIL-37)
    validation::validate_folder_name(&folder)?;
    let page = query.page.unwrap_or(0);
    let page_size = query.page_size.unwrap_or(50).min(200);

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
    let (messages, total) = imap_service
        .list_messages(_imap_user, _imap_pass, &folder, page, page_size)
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

    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;
    let message = imap_service
        .get_message(_imap_user, _imap_pass, &folder, uid)
        .await?;

    Ok(Json(message))
}

/// Added (TMAIL-320): GET /api/folders/{folder}/messages/{uid}/parts/{part_id}
/// — stream the bytes of a single MIME part (typically an attachment) so the
/// SPA's Download button can trigger a real browser download via a blob URL.
///
/// `part_id` is the dotted path the same MIME walker assigns when it
/// populates `FullMessage.attachments` ("1", "2.1", …), so the SPA just
/// hands back the value it already has on the attachment object — no
/// separate lookup step.
///
/// We don't expose part bytes inline on `GET /messages/{uid}` because
/// attachments can be tens of megabytes and the JSON envelope would balloon
/// for every reader pane. This handler is the lazy fetch.
pub async fn download_message_part(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid, part_id)): Path<(String, u32, String)>,
) -> Result<Response<Body>, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    let (imap_user, imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    let part = imap_service
        .get_message_part(imap_user, imap_pass, &folder, uid, &part_id)
        .await?;

    let mut headers = HeaderMap::new();
    // application/octet-stream falls back when the source MIME type isn't a
    // valid HeaderValue (e.g. malformed Content-Type from an upstream MTA).
    let ct = HeaderValue::from_str(&part.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, ct);

    // RFC 6266: filename* with UTF-8 + a quoted ASCII fallback so non-ASCII
    // names survive both modern and legacy browsers. Doing both keeps the
    // download experience predictable across the matrix.
    let safe_filename = part.filename.replace(['\r', '\n', '"'], "_");
    let ascii_filename: String = safe_filename
        .chars()
        .map(|c| if c.is_ascii() { c } else { '_' })
        .collect();
    let encoded = urlencoding::encode(&safe_filename);
    let cd = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        ascii_filename, encoded
    );
    if let Ok(v) = HeaderValue::from_str(&cd) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(part.bytes.len()));

    let mut response = Response::new(Body::from(part.bytes));
    *response.headers_mut() = headers;
    Ok(response)
}

/// POST /api/messages/send — send an email
pub async fn send_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SendRequest>,
) -> Result<StatusCode, AppError> {
    // Added: TMAIL-37 — validate all user-controlled headers and body before SMTP.
    // Lettre rejects most malformed headers on its own, but defense-in-depth blocks
    // CRLF injection and oversized payloads at the API boundary so we never even
    // reach the SMTP transport with hostile input.
    validation::validate_subject(&body.subject)?;
    if body.to.is_empty() {
        return Err(AppError::BadRequest("At least one To recipient is required".into()));
    }
    validation::validate_recipient_list("To", &body.to)?;
    if let Some(ref cc) = body.cc {
        validation::validate_recipient_list("Cc", cc)?;
    }
    if let Some(ref bcc) = body.bcc {
        validation::validate_recipient_list("Bcc", bcc)?;
    }
    if let Some(ref text) = body.text_body {
        validation::validate_body_size(text)?;
    }
    if let Some(ref html) = body.html_body {
        validation::validate_body_size(html)?;
    }

    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // BYOK send: load the user's default SMTP server from smtp_configurations + decrypt the password.
    // The IMAP credentials we loaded above are the wrong key for SMTP — they likely won't even authenticate.
    // TMAIL-158: try Redis first; cache holds the encrypted ciphertext, never plaintext.
    let cache_key = mailbox.id.to_string();
    let smtp_cfg: crate::models::smtp_config::SmtpConfiguration = match state
        .cache
        .get_user_smtp_config::<crate::models::smtp_config::SmtpConfiguration>(&cache_key)
        .await
    {
        Some(hit) => hit,
        None => {
            let row = crate::models::smtp_config::SmtpConfiguration::find_default(&state.db, mailbox.id)
                .await?
                .ok_or_else(|| AppError::ServiceUnavailable(
                    "No SMTP server configured. Complete the onboarding wizard at /onboarding.".into()
                ))?;
            let _ = state.cache.set_user_smtp_config(&cache_key, &row).await;
            row
        }
    };
    let enc_key = crate::models::ai_config::derive_encryption_key(&state.config.jwt.secret);
    let smtp_password = smtp_cfg.decrypted_password(&enc_key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to decrypt SMTP password: {}", e)))?;
    let smtp_from = smtp_cfg.from_address.clone().unwrap_or_else(|| smtp_cfg.username.clone());

    let smtp_runtime_cfg = crate::config::SmtpConfig {
        host: smtp_cfg.host.clone(),
        port: smtp_cfg.port as u16,
        tls: matches!(smtp_cfg.encryption.as_str(), "ssl" | "starttls"),
        notification_from: None,
        notification_username: None,
        notification_password: None,
    };
    let smtp_service = SmtpService::new(smtp_runtime_cfg);
    smtp_service.send(&smtp_from, &smtp_password, &body).await?;

    // Added: TMAIL-119 — auto-collect contacts from outgoing recipients. Fire-and-forget so a
    // contacts DB hiccup never blocks send. Existing rows are left alone (display name we
    // already chose wins over whatever was typed in this compose).
    {
        let db = state.db.clone();
        let to = body.to.clone();
        let cc = body.cc.clone().unwrap_or_default();
        let bcc = body.bcc.clone().unwrap_or_default();
        let mailbox_id = mailbox.id;
        tokio::spawn(async move {
            for raw in to.iter().chain(cc.iter()).chain(bcc.iter()) {
                if let Some((name, email)) = crate::models::contact::parse_recipient(raw) {
                    if let Err(e) = crate::models::contact::Contact::upsert_from_send(
                        &db,
                        mailbox_id,
                        &email,
                        name.as_deref(),
                    )
                    .await
                    {
                        tracing::warn!(error = ?e, email = %email, "auto-collect contact upsert failed");
                    }
                }
            }
        });
    }

    // Added: TMAIL-131 — fire email.sent webhook with envelope including recipients and subject
    fire_webhook(
        &state,
        mailbox.id,
        WebhookEvent::EmailSent,
        serde_json::json!({
            "from": smtp_from,
            "to": body.to,
            "cc": body.cc,
            "subject": body.subject,
        }),
    );

    Ok(StatusCode::CREATED)
}

/// GET /api/search — search messages across folders
pub async fn search_messages(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Added: Validate search query to prevent IMAP injection (TMAIL-37)
    validation::validate_search_query(&query.q)?;
    if let Some(ref f) = query.folder {
        validation::validate_folder_name(f)?;
    }

    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let folder = query.folder.as_deref().unwrap_or("INBOX");
    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;
    let messages = imap_service
        .search_messages(_imap_user, _imap_pass, folder, &query.q)
        .await?;

    Ok(Json(serde_json::json!({
        "messages": messages,
        "total": messages.len(),
        "query": query.q,
        "folder": folder,
    })))
}

/// DELETE /api/folders/:folder/messages/:uid — delete a message
pub async fn delete_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<StatusCode, AppError> {
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
    imap_service
        .delete_message(_imap_user, _imap_pass, &folder, uid)
        .await?;

    // Added: TMAIL-131 — fire email.deleted webhook
    fire_webhook(
        &state,
        mailbox.id,
        WebhookEvent::EmailDeleted,
        serde_json::json!({ "folder": folder, "uid": uid }),
    );

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/folders/:folder/messages/:uid/move — move a message
pub async fn move_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
    Json(body): Json<MoveRequest>,
) -> Result<StatusCode, AppError> {
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
    imap_service
        .move_message(_imap_user, _imap_pass, &folder, uid, &body.to_folder)
        .await?;

    // Added: TMAIL-131 — fire email.moved webhook with source + destination folder
    fire_webhook(
        &state,
        mailbox.id,
        WebhookEvent::EmailMoved,
        serde_json::json!({
            "from_folder": folder,
            "to_folder": body.to_folder,
            "uid": uid,
        }),
    );

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/folders/:folder/messages/:uid/flag — set/remove a flag
pub async fn flag_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
    Json(body): Json<FlagRequest>,
) -> Result<StatusCode, AppError> {
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
    imap_service
        .set_flag(_imap_user, _imap_pass, &folder, uid, &body.flag, body.add)
        .await?;

    // Added: TMAIL-131 — fire email.flagged webhook (covers both adding and removing flags)
    fire_webhook(
        &state,
        mailbox.id,
        WebhookEvent::EmailFlagged,
        serde_json::json!({
            "folder": folder,
            "uid": uid,
            "flag": body.flag,
            "added": body.add,
        }),
    );

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/drafts — save a draft to IMAP Drafts folder
pub async fn save_draft(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SaveDraftRequest>,
) -> Result<StatusCode, AppError> {
    // Added: TMAIL-37 — full header-injection guard. The raw RFC 2822 message
    // below is built with format!() against user-controlled to/cc/subject, so
    // any CR/LF/NUL in those fields would let an attacker splice in arbitrary
    // headers (Bcc, From spoofing, MIME boundary manipulation). Validate ALL
    // inputs before we touch the format! macro.
    validation::validate_subject(&body.subject)?;
    validation::validate_recipient_list("To", &body.to)?;
    if let Some(ref cc) = body.cc {
        validation::validate_recipient_list("Cc", cc)?;
    }
    if let Some(ref text) = body.text_body {
        validation::validate_body_size(text)?;
    }
    if let Some(ref html) = body.html_body {
        validation::validate_body_size(html)?;
    }

    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // Build a minimal RFC 2822 message for the draft
    let to_header = body.to.join(", ");
    let cc_header = body.cc.as_deref().unwrap_or(&[]).join(", ");
    let text = body.text_body.as_deref().unwrap_or("");
    let html = body.html_body.as_deref().unwrap_or("");

    let mut raw_msg = format!(
        "From: {}\r\nTo: {}\r\n",
        mailbox.username, to_header,
    );
    if !cc_header.is_empty() {
        raw_msg.push_str(&format!("Cc: {}\r\n", cc_header));
    }
    raw_msg.push_str(&format!("Subject: {}\r\n", body.subject));
    raw_msg.push_str(&format!("Date: {}\r\n", chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000")));
    raw_msg.push_str("MIME-Version: 1.0\r\n");

    if !html.is_empty() {
        let boundary = format!("----=_Part_{}", uuid::Uuid::new_v4().simple());
        raw_msg.push_str(&format!("Content-Type: multipart/alternative; boundary=\"{}\"\r\n\r\n", boundary));
        raw_msg.push_str(&format!("--{}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n", boundary, text));
        raw_msg.push_str(&format!("--{}\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{}\r\n", boundary, html));
        raw_msg.push_str(&format!("--{}--\r\n", boundary));
    } else {
        raw_msg.push_str("Content-Type: text/plain; charset=utf-8\r\n\r\n");
        raw_msg.push_str(text);
    }

    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;
    imap_service
        .save_draft(_imap_user, _imap_pass, raw_msg.as_bytes())
        .await?;

    Ok(StatusCode::CREATED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_messages_query_defaults() {
        let json = r#"{}"#;
        let query: ListMessagesQuery = serde_json::from_str(json).unwrap();
        assert!(query.page.is_none());
        assert!(query.page_size.is_none());
    }

    #[test]
    fn test_list_messages_query_with_values() {
        let json = r#"{"page": 2, "page_size": 25}"#;
        let query: ListMessagesQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, Some(2));
        assert_eq!(query.page_size, Some(25));
    }

    #[test]
    fn test_list_messages_query_partial() {
        let json = r#"{"page": 5}"#;
        let query: ListMessagesQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, Some(5));
        assert!(query.page_size.is_none());
    }

    #[test]
    fn test_search_query_required_q() {
        let json = r#"{"q": "invoice"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.q, "invoice");
        assert!(query.folder.is_none());
    }

    #[test]
    fn test_search_query_with_folder() {
        let json = r#"{"q": "hello", "folder": "Sent"}"#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.q, "hello");
        assert_eq!(query.folder.as_deref(), Some("Sent"));
    }

    #[test]
    fn test_search_query_missing_q_fails() {
        let json = r#"{"folder": "INBOX"}"#;
        let result = serde_json::from_str::<SearchQuery>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_move_request_deserialization() {
        let json = r#"{"to_folder": "Trash"}"#;
        let req: MoveRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.to_folder, "Trash");
    }

    #[test]
    fn test_move_request_missing_folder_fails() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<MoveRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_flag_request_deserialization() {
        let json = r#"{"flag": "\\Seen", "add": true}"#;
        let req: FlagRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.flag, "\\Seen");
        assert!(req.add);
    }

    #[test]
    fn test_flag_request_remove() {
        let json = r#"{"flag": "\\Flagged", "add": false}"#;
        let req: FlagRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.flag, "\\Flagged");
        assert!(!req.add);
    }

    #[test]
    fn test_save_draft_request_full() {
        let json = r#"{
            "to": ["alice@example.com"],
            "cc": ["bob@example.com"],
            "subject": "Draft subject",
            "html_body": "<p>Hello</p>",
            "text_body": "Hello"
        }"#;
        let req: SaveDraftRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.to, vec!["alice@example.com"]);
        assert_eq!(req.cc.as_ref().unwrap().len(), 1);
        assert_eq!(req.subject, "Draft subject");
        assert_eq!(req.html_body.as_deref(), Some("<p>Hello</p>"));
        assert_eq!(req.text_body.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_save_draft_request_minimal() {
        let json = r#"{"to": ["x@y.com"], "subject": "Test"}"#;
        let req: SaveDraftRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.to, vec!["x@y.com"]);
        assert_eq!(req.subject, "Test");
        assert!(req.cc.is_none());
        assert!(req.html_body.is_none());
        assert!(req.text_body.is_none());
    }

    #[test]
    fn test_save_draft_request_missing_subject_fails() {
        let json = r#"{"to": ["x@y.com"]}"#;
        let result = serde_json::from_str::<SaveDraftRequest>(json);
        assert!(result.is_err());
    }
}
