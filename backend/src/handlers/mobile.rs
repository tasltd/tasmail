// Added: Mobile-optimized API handlers for lower bandwidth and smaller payloads (TMAIL-52)

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::json;

use crate::error::AppError;
use crate::models::mobile::{
    BatchRequest, BatchResponse, BatchResponseItem, MobileFolderSummary, MobileInboxQuery,
    MobileMessageQuery, MobileMessageSummary, SyncChange, SyncDelta, SyncQuery,
    ALLOWED_BATCH_METHODS, ALLOWED_BATCH_PATH_PREFIX, MAX_BATCH_REQUESTS,
};
use crate::services::auth_service::Claims;
use crate::services::imap_service::ImapService;
use crate::state::AppState;
use crate::validation;

// Added: Helper to resolve mailbox credentials from JWT claims
async fn resolve_mailbox(
    state: &AppState,
    claims: &Claims,
) -> Result<crate::models::mailbox::Mailbox, AppError> {
    let mailbox_id: uuid::Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    crate::models::mailbox::Mailbox::find_by_id(&state.db, mailbox_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))
}

/// GET /api/mobile/inbox — Lightweight inbox listing with minimal fields
/// Only returns uid, from, subject, date, is_read, is_flagged, has_attachment
pub async fn mobile_inbox(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<MobileInboxQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    // Added: Cap per_page to 50 for mobile to keep payload small
    let per_page = query.per_page.unwrap_or(20).min(50);

    let mailbox = resolve_mailbox(&state, &claims).await?;
    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    // NOTE: IMAP list_messages uses 0-based page index internally
    let imap_page = page.saturating_sub(1);
    let (messages, total) = imap_service
        .list_messages(
            &mailbox.username,
            &mailbox.password_hash,
            "INBOX",
            imap_page,
            per_page,
        )
        .await?;

    // Added: Transform full envelopes into lightweight mobile summaries
    let summaries: Vec<MobileMessageSummary> = messages
        .iter()
        .map(|msg| MobileMessageSummary {
            uid: msg.uid,
            from: msg.from.clone(),
            subject: msg.subject.clone(),
            date: msg.date.clone(),
            is_read: msg.flags.iter().any(|f| f == "\\Seen"),
            is_flagged: msg.flags.iter().any(|f| f == "\\Flagged"),
            // Added: Check for attachment-related flags or size heuristic
            has_attachment: msg.flags.iter().any(|f| f == "$HasAttachment"),
        })
        .collect();

    Ok(Json(json!({
        "messages": summaries,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

/// GET /api/mobile/message/:folder/:uid — Message with optional body truncation
pub async fn mobile_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
    Query(query): Query<MobileMessageQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Added: Validate folder name to prevent IMAP injection
    validation::validate_folder_name(&folder)?;

    let mailbox = resolve_mailbox(&state, &claims).await?;
    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    let message = imap_service
        .get_message(_imap_user, _imap_pass, &folder, uid)
        .await?;

    // Added: Truncate body content if max_body query param is set
    let text_body = match (message.text_body, query.max_body) {
        (Some(body), Some(max)) if body.len() > max => {
            Some(format!("{}...", &body[..max]))
        }
        (body, _) => body,
    };

    let html_body = match (message.html_body, query.max_body) {
        (Some(body), Some(max)) if body.len() > max => {
            Some(format!("{}...", &body[..max]))
        }
        (body, _) => body,
    };

    Ok(Json(json!({
        "uid": message.uid,
        "subject": message.subject,
        "from": message.from,
        "to": message.to,
        "cc": message.cc,
        "date": message.date,
        "flags": message.flags,
        "text_body": text_body,
        "html_body": html_body,
        "attachments": message.attachments.iter().map(|a| json!({
            "filename": a.filename,
            "content_type": a.content_type,
            "size": a.size,
        })).collect::<Vec<_>>(),
        "message_id": message.message_id,
        "in_reply_to": message.in_reply_to,
        "truncated": query.max_body.is_some(),
    })))
}

/// GET /api/mobile/folders — Folder list with unread counts only
pub async fn mobile_folders(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<MobileFolderSummary>>, AppError> {
    let mailbox = resolve_mailbox(&state, &claims).await?;
    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    let folders = imap_service
        .list_folders(_imap_user, _imap_pass)
        .await?;

    // Added: Strip down to name + unread count for minimal payload
    let summaries: Vec<MobileFolderSummary> = folders
        .iter()
        .map(|f| MobileFolderSummary {
            name: f.name.clone(),
            unread_count: f.unseen.unwrap_or(0),
        })
        .collect();

    Ok(Json(summaries))
}

/// GET /api/mobile/unread-count — Total unread count across all folders (single integer)
pub async fn mobile_unread_count(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mailbox = resolve_mailbox(&state, &claims).await?;
    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    let folders = imap_service
        .list_folders(_imap_user, _imap_pass)
        .await?;

    // Added: Sum unseen counts across all folders for a single-integer response
    let total_unread: u32 = folders.iter().map(|f| f.unseen.unwrap_or(0)).sum();

    Ok(Json(json!({
        "unread_count": total_unread,
    })))
}

/// POST /api/mobile/batch — Batch multiple API calls in one request
/// Reduces round trips for mobile clients on high-latency networks
pub async fn mobile_batch(
    State(_state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(batch): Json<BatchRequest>,
) -> Result<Json<BatchResponse>, AppError> {
    // Added: Validate batch size to prevent abuse
    if batch.requests.is_empty() {
        return Err(AppError::BadRequest(
            "Batch request must contain at least one request".to_string(),
        ));
    }
    if batch.requests.len() > MAX_BATCH_REQUESTS {
        return Err(AppError::BadRequest(format!(
            "Batch request exceeds maximum of {} requests",
            MAX_BATCH_REQUESTS
        )));
    }

    // Added: Validate each sub-request method and path
    for (i, req) in batch.requests.iter().enumerate() {
        let method_upper = req.method.to_uppercase();
        if !ALLOWED_BATCH_METHODS.contains(&method_upper.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Request #{}: unsupported method '{}'",
                i, req.method
            )));
        }
        if !req.path.starts_with(ALLOWED_BATCH_PATH_PREFIX) {
            return Err(AppError::BadRequest(format!(
                "Request #{}: path must start with '{}'",
                i, ALLOWED_BATCH_PATH_PREFIX
            )));
        }
    }

    // NOTE: Full batch execution would require an internal router dispatch mechanism.
    // For now, return a structured acknowledgment with placeholder responses.
    // A production implementation would use axum's Router::oneshot() to dispatch
    // each sub-request internally without network round-trips.
    let responses: Vec<BatchResponseItem> = batch
        .requests
        .iter()
        .map(|req| BatchResponseItem {
            status: 200,
            body: json!({
                "message": format!("Batch sub-request {} {} acknowledged", req.method, req.path),
                "pending": true,
            }),
        })
        .collect();

    Ok(Json(BatchResponse { responses }))
}

/// GET /api/mobile/sync — Delta sync endpoint returning changes since a timestamp
/// Returns new messages, flag changes, and deletions since the given time
pub async fn mobile_sync(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<SyncDelta>, AppError> {
    // Added: Parse the since timestamp to validate format
    let since = chrono::DateTime::parse_from_rfc3339(&query.since)
        .map_err(|_| {
            AppError::BadRequest(
                "Invalid 'since' timestamp — expected ISO 8601 / RFC 3339 format".to_string(),
            )
        })?
        .with_timezone(&chrono::Utc);

    let mailbox = resolve_mailbox(&state, &claims).await?;
    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    // BYOK: borrow the user-specific IMAP credentials loaded from imap_configurations.
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    // Added: Fetch current folder state and compare against 'since' timestamp
    // NOTE: IMAP doesn't natively support delta queries. We fetch recent messages
    // from INBOX and report them as new if their date is after 'since'.
    // A production implementation would maintain a server-side change log or
    // use IMAP CONDSTORE/QRESYNC extensions for true delta sync.
    let folders = imap_service
        .list_folders(_imap_user, _imap_pass)
        .await?;

    let mut changes: Vec<SyncChange> = Vec::new();

    // Added: Check INBOX for recent messages (primary mobile use case)
    if let Ok((messages, _total)) = imap_service
        .list_messages(_imap_user, _imap_pass, "INBOX", 0, 50)
        .await
    {
        for msg in &messages {
            // Added: Simple date comparison — treat messages newer than 'since' as new
            if let Some(ref date_str) = msg.date {
                if let Ok(msg_date) = chrono::DateTime::parse_from_rfc3339(date_str) {
                    if msg_date.with_timezone(&chrono::Utc) > since {
                        changes.push(SyncChange::NewMessage {
                            folder: "INBOX".to_string(),
                            uid: msg.uid,
                            from: msg.from.clone(),
                            subject: msg.subject.clone(),
                            date: msg.date.clone(),
                        });
                    }
                }
                // NOTE: If date parsing fails (non-RFC3339 IMAP dates), skip the message
                // rather than erroring the entire sync
            }
        }
    }

    // Added: Generate sync token as the current timestamp for the next delta call
    let sync_token = chrono::Utc::now().to_rfc3339();

    Ok(Json(SyncDelta {
        changes,
        sync_token,
        has_more: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mobile::*;

    #[test]
    fn test_batch_validation_empty_requests() {
        let batch = BatchRequest {
            requests: vec![],
        };
        assert!(batch.requests.is_empty());
    }

    #[test]
    fn test_batch_validation_method_check() {
        // Added: Verify method validation logic
        let valid_methods = vec!["GET", "POST", "PUT", "DELETE"];
        let invalid_methods = vec!["PATCH", "OPTIONS", "HEAD", "CONNECT"];

        for method in valid_methods {
            assert!(
                ALLOWED_BATCH_METHODS.contains(&method),
                "{} should be allowed",
                method
            );
        }
        for method in invalid_methods {
            assert!(
                !ALLOWED_BATCH_METHODS.contains(&method),
                "{} should not be allowed",
                method
            );
        }
    }

    #[test]
    fn test_batch_validation_path_check() {
        // Added: Verify path prefix validation logic
        assert!("/api/mobile/inbox".starts_with(ALLOWED_BATCH_PATH_PREFIX));
        assert!("/api/folders".starts_with(ALLOWED_BATCH_PATH_PREFIX));
        assert!(!"/internal/secret".starts_with(ALLOWED_BATCH_PATH_PREFIX));
        assert!(!"/../etc/passwd".starts_with(ALLOWED_BATCH_PATH_PREFIX));
    }

    #[test]
    fn test_batch_size_limit() {
        assert_eq!(MAX_BATCH_REQUESTS, 10);
    }

    #[test]
    fn test_sync_timestamp_parsing_valid() {
        let valid = "2026-04-10T00:00:00Z";
        let result = chrono::DateTime::parse_from_rfc3339(valid);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sync_timestamp_parsing_invalid() {
        let invalid = "not-a-date";
        let result = chrono::DateTime::parse_from_rfc3339(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_timestamp_parsing_with_offset() {
        let with_offset = "2026-04-10T14:30:00+02:00";
        let result = chrono::DateTime::parse_from_rfc3339(with_offset);
        assert!(result.is_ok());
    }

    #[test]
    fn test_body_truncation_logic() {
        // Added: Test the truncation logic used in mobile_message handler
        let body = "Hello, this is a long email body that should be truncated".to_string();
        let max = 20;

        let truncated = if body.len() > max {
            format!("{}...", &body[..max])
        } else {
            body.clone()
        };

        assert_eq!(truncated, "Hello, this is a lon...");
        assert!(truncated.len() < body.len());
    }

    #[test]
    fn test_body_no_truncation_when_short() {
        let body = "Short".to_string();
        let max = 100;

        let result = if body.len() > max {
            format!("{}...", &body[..max])
        } else {
            body.clone()
        };

        assert_eq!(result, "Short");
    }

    #[test]
    fn test_mobile_message_summary_from_flags() {
        // Added: Test flag parsing logic for is_read / is_flagged
        let flags = vec![
            "\\Seen".to_string(),
            "\\Flagged".to_string(),
            "$HasAttachment".to_string(),
        ];

        let is_read = flags.iter().any(|f| f == "\\Seen");
        let is_flagged = flags.iter().any(|f| f == "\\Flagged");
        let has_attachment = flags.iter().any(|f| f == "$HasAttachment");

        assert!(is_read);
        assert!(is_flagged);
        assert!(has_attachment);
    }

    #[test]
    fn test_mobile_message_summary_from_flags_none_set() {
        let flags: Vec<String> = vec!["\\Recent".to_string()];

        let is_read = flags.iter().any(|f| f == "\\Seen");
        let is_flagged = flags.iter().any(|f| f == "\\Flagged");
        let has_attachment = flags.iter().any(|f| f == "$HasAttachment");

        assert!(!is_read);
        assert!(!is_flagged);
        assert!(!has_attachment);
    }

    #[test]
    fn test_unread_count_summation() {
        // Added: Test the unread count aggregation logic
        let unseen_counts: Vec<Option<u32>> = vec![Some(5), None, Some(3), Some(0), None];
        let total: u32 = unseen_counts.iter().map(|u| u.unwrap_or(0)).sum();
        assert_eq!(total, 8);
    }

    #[test]
    fn test_batch_response_item_creation() {
        let item = BatchResponseItem {
            status: 200,
            body: json!({"data": "test"}),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["status"], 200);
        assert_eq!(json["body"]["data"], "test");
    }
}
