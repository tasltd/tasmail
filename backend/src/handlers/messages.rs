use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::{FullMessage, ImapService};
use crate::services::smtp_service::{SendRequest, SmtpService};
use crate::state::AppState;
// Added: Input validation for message operations (TMAIL-37)
use crate::validation;

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

/// POST /api/messages/send — send an email
pub async fn send_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SendRequest>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let mailbox = crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // BYOK send: load the user's default SMTP server from smtp_configurations + decrypt the password.
    // The IMAP credentials we loaded above are the wrong key for SMTP — they likely won't even authenticate.
    let smtp_cfg = crate::models::smtp_config::SmtpConfiguration::find_default(&state.db, mailbox.id)
        .await?
        .ok_or_else(|| AppError::ServiceUnavailable(
            "No SMTP server configured. Complete the onboarding wizard at /onboarding.".into()
        ))?;
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

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/drafts — save a draft to IMAP Drafts folder
pub async fn save_draft(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SaveDraftRequest>,
) -> Result<StatusCode, AppError> {
    // Added: Validate draft subject length (TMAIL-37)
    validation::validate_subject(&body.subject)?;

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
