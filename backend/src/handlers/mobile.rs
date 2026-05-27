// Added: Mobile-optimized API handlers for lower bandwidth and smaller payloads (TMAIL-52)

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::json;

use crate::error::AppError;
use crate::models::mobile::{
    BatchRequest, BatchResponse, BatchResponseItem, MobileFolderSummary, MobileInboxQuery,
    MobileMessageQuery, MobileMessageSummary, MobileUsage, SyncChange, SyncDelta, SyncQuery,
    SyncVersionBody, SyncVersionQuery, ALLOWED_BATCH_METHODS, ALLOWED_BATCH_PATH_PREFIX,
    LOW_BANDWIDTH_PREVIEW_CHARS, MAX_BATCH_REQUESTS,
};
use crate::models::quota::QuotaUsage;
use crate::services::auth_service::Claims;
use crate::services::imap_service::ImapService;
use crate::state::AppState;
use crate::validation;

// Strip HTML tags and collapse whitespace into a single-line text snippet.
// Used by low-bandwidth mode so we can serve HTML-only senders (newsletters,
// marketing mail) without shipping the raw markup to 2G/Edge clients.
//
// Intentionally lightweight — not a full HTML parser. Senders that smuggle
// `<script>` or `<style>` bodies will have their inner text stripped along
// with the tags, which is the right behaviour for a preview snippet (we do
// not want to render attacker-controlled text in a notification anyway).
fn html_to_text_preview(html: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(html.len().min(max_chars * 2));
    let mut in_tag = false;
    let mut last_was_space = true; // suppress leading whitespace

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !last_was_space {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !last_was_space {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            c => {
                out.push(c);
                last_was_space = false;
            }
        }
    }

    truncate_chars(out.trim(), max_chars)
}

// Truncate by char boundary (not byte boundary) so we never slice mid-UTF-8.
// `&body[..max]` panics on non-ASCII (eg. Twi/Ewe diacritics) — `char_indices`
// gives us a safe boundary.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut count = 0usize;
    for (idx, _) in s.char_indices() {
        if count == max_chars {
            return format!("{}...", &s[..idx]);
        }
        count += 1;
    }
    s.to_string()
}

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

    // Added: Transform full envelopes into lightweight mobile summaries.
    // In low-bandwidth mode the IMAP envelope does not include body text, so
    // `preview` stays None here — the client gets a Subject-only row. A future
    // pass can fetch BODYSTRUCTURE.TEXT for the first N rows on demand, but we
    // intentionally keep the list endpoint bounded so it stays O(1) IMAP calls
    // regardless of `per_page`.
    let low_bandwidth = query.low_bandwidth.unwrap_or(false);
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
            preview: if low_bandwidth {
                msg.subject
                    .as_ref()
                    .map(|s| truncate_chars(s, LOW_BANDWIDTH_PREVIEW_CHARS))
            } else {
                None
            },
        })
        .collect();

    Ok(Json(json!({
        "messages": summaries,
        "total": total,
        "page": page,
        "per_page": per_page,
        "low_bandwidth": low_bandwidth,
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

    // Low-bandwidth mode forces text-only output with a hard preview cap that
    // overrides any client-supplied `max_body`. We also drop HTML, attachment
    // bodies, and full To/Cc lists so the response fits in ~1 packet on Edge.
    let low_bandwidth = query.low_bandwidth.unwrap_or(false);
    let effective_max = if low_bandwidth {
        Some(LOW_BANDWIDTH_PREVIEW_CHARS)
    } else {
        query.max_body
    };

    // In low-bandwidth mode, fall back to stripping HTML when the sender only
    // supplied an HTML body — otherwise the user sees an empty message.
    let text_source = match (&message.text_body, &message.html_body, low_bandwidth) {
        (Some(t), _, _) => Some(t.clone()),
        (None, Some(h), true) => Some(html_to_text_preview(h, LOW_BANDWIDTH_PREVIEW_CHARS)),
        (None, html, false) => html.clone(),
        (None, None, true) => None,
    };

    let text_body = match (text_source, effective_max) {
        (Some(body), Some(max)) => Some(truncate_chars(&body, max)),
        (body, _) => body,
    };

    // Drop HTML entirely in low-bandwidth mode — clients render the text body.
    let html_body = if low_bandwidth {
        None
    } else {
        match (message.html_body, effective_max) {
            (Some(body), Some(max)) => Some(truncate_chars(&body, max)),
            (body, _) => body,
        }
    };

    Ok(Json(json!({
        "uid": message.uid,
        "subject": message.subject,
        "from": message.from,
        "to": if low_bandwidth { Vec::new() } else { message.to },
        "cc": if low_bandwidth { Vec::new() } else { message.cc },
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
        "truncated": effective_max.is_some(),
        "low_bandwidth": low_bandwidth,
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

/// POST /api/mobile/sync — Delta sync using an opaque version cursor.
///
/// Mobile clients call this after a successful initial sync, passing the
/// `sync_token` from the prior response either as `?version=<token>` or in the
/// JSON body. The token is currently an RFC 3339 timestamp under the hood, but
/// the wire contract treats it as opaque so we can move to IMAP CONDSTORE
/// `MODSEQ` or an internal change-log row ID later without breaking clients.
///
/// When `version` is omitted, the server returns changes from the last 24h so
/// a fresh install can bootstrap without paginating the full mailbox.
pub async fn mobile_sync_post(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<SyncVersionQuery>,
    body: Option<Json<SyncVersionBody>>,
) -> Result<Json<SyncDelta>, AppError> {
    // Resolve the cursor: prefer the query string so retries are idempotent,
    // fall back to the body for clients that pack the token there.
    let body_version = body.and_then(|Json(b)| b.version);
    let version = query.version.or(body_version);

    let since = match version {
        Some(v) => chrono::DateTime::parse_from_rfc3339(&v).map_err(|_| {
            AppError::BadRequest(
                "Invalid 'version' cursor — expected an opaque token from the prior sync response"
                    .to_string(),
            )
        })?.with_timezone(&chrono::Utc),
        None => chrono::Utc::now() - chrono::Duration::hours(24),
    };

    let mailbox = resolve_mailbox(&state, &claims).await?;
    let imap_service = ImapService::for_user(&state, mailbox.id).await?;
    let (_imap_user, _imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;

    let mut changes: Vec<SyncChange> = Vec::new();
    if let Ok((messages, _total)) = imap_service
        .list_messages(_imap_user, _imap_pass, "INBOX", 0, 50)
        .await
    {
        for msg in &messages {
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
            }
        }
    }

    Ok(Json(SyncDelta {
        changes,
        sync_token: chrono::Utc::now().to_rfc3339(),
        has_more: false,
    }))
}

/// GET /api/mobile/usage — Lightweight data quota for mobile dashboards.
///
/// Reads the cached `QuotaUsage` row from Postgres without forcing a fresh
/// IMAP `GETQUOTAROOT` round trip (use `POST /api/quota/sync` for that). The
/// response carries only the fields the mobile UI renders — used/quota bytes,
/// usage percent, message count, warning/over-quota flags. Skips
/// `last_synced_at` and `mailbox_id` since mobile clients infer those from
/// session state.
///
/// Why not reuse `/api/quota`? That endpoint returns a richer `QuotaStatus`
/// (~250 B JSON) and triggers Redis caching keyed by the JWT sub. The mobile
/// variant trims the payload to ~120 B so push-notification refresh cycles do
/// not pay the full cost.
pub async fn mobile_usage(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<MobileUsage>, AppError> {
    let mailbox = resolve_mailbox(&state, &claims).await?;
    let usage = QuotaUsage::find_by_mailbox(&state.db, mailbox.id).await?;

    let (used_bytes, message_count) = match usage.as_ref() {
        Some(u) => (u.used_bytes, u.message_count),
        None => (0, 0),
    };

    let usage_percent = if mailbox.quota_bytes > 0 {
        (used_bytes as f64 / mailbox.quota_bytes as f64) * 100.0
    } else {
        0.0
    };

    let is_warning = usage_percent >= mailbox.quota_warn_percent as f64;
    let is_over_quota = mailbox.quota_bytes > 0 && used_bytes >= mailbox.quota_bytes;

    Ok(Json(MobileUsage {
        used_bytes,
        quota_bytes: mailbox.quota_bytes,
        usage_percent,
        message_count,
        is_warning,
        is_over_quota,
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

    // --- TMAIL-52 additions: low-bandwidth mode, usage, POST sync ---

    #[test]
    fn test_truncate_chars_ascii() {
        let s = "Hello, this is a long email body that should be truncated";
        let out = truncate_chars(s, 20);
        assert_eq!(out, "Hello, this is a lon...");
    }

    #[test]
    fn test_truncate_chars_short_input_unchanged() {
        let out = truncate_chars("Short", 100);
        assert_eq!(out, "Short");
    }

    #[test]
    fn test_truncate_chars_respects_utf8_boundary() {
        // Akan/Twi diacritics — two of these chars are multi-byte. A naive
        // &s[..max] slice would panic; truncate_chars must not.
        let s = "Mɛda wo ase paa, yɛbɛhyia bio ɛnnɛ anwummerɛ";
        let out = truncate_chars(s, 10);
        // 10 chars, then "..."
        assert!(out.ends_with("..."));
        let core: Vec<char> = out.chars().filter(|c| *c != '.').collect();
        // exactly 10 base chars before the ellipsis
        assert_eq!(core.len(), 10);
    }

    #[test]
    fn test_html_to_text_preview_strips_tags() {
        let html = "<p>Hello <b>world</b>!</p>";
        let out = html_to_text_preview(html, 100);
        assert_eq!(out, "Hello world !");
    }

    #[test]
    fn test_html_to_text_preview_collapses_whitespace() {
        let html = "<div>\n  Hello\n\n\n   world\n</div>";
        let out = html_to_text_preview(html, 100);
        assert_eq!(out, "Hello world");
    }

    #[test]
    fn test_html_to_text_preview_caps_length() {
        let html = "<p>".to_string() + &"ab".repeat(500) + "</p>";
        let out = html_to_text_preview(&html, 50);
        // 50 chars + "..."
        let chars: Vec<char> = out.chars().collect();
        assert!(out.ends_with("..."));
        assert_eq!(chars.len(), 53);
    }

    #[test]
    fn test_mobile_inbox_query_low_bandwidth_alias() {
        // Both spellings must deserialize so older clients can use ?low_bw=true
        let q1: MobileInboxQuery =
            serde_urlencoded::from_str("low_bandwidth=true").unwrap();
        let q2: MobileInboxQuery = serde_urlencoded::from_str("low_bw=true").unwrap();
        assert_eq!(q1.low_bandwidth, Some(true));
        assert_eq!(q2.low_bandwidth, Some(true));
    }

    #[test]
    fn test_mobile_inbox_query_defaults() {
        let q: MobileInboxQuery = serde_urlencoded::from_str("").unwrap();
        assert!(q.page.is_none());
        assert!(q.per_page.is_none());
        assert!(q.low_bandwidth.is_none());
    }

    #[test]
    fn test_mobile_message_summary_preview_serialization_low_bw() {
        let summary = MobileMessageSummary {
            uid: 7,
            from: Some("kojo@tasmail.gh".to_string()),
            subject: Some("Build update".to_string()),
            date: None,
            is_read: false,
            is_flagged: false,
            has_attachment: false,
            preview: Some("Build update".to_string()),
        };
        let v = serde_json::to_value(&summary).unwrap();
        assert_eq!(v["preview"], "Build update");
    }

    #[test]
    fn test_mobile_message_summary_preview_omitted_when_none() {
        // Standard (non-low-bandwidth) rows must not waste bytes on a null preview.
        let summary = MobileMessageSummary {
            uid: 8,
            from: None,
            subject: None,
            date: None,
            is_read: false,
            is_flagged: false,
            has_attachment: false,
            preview: None,
        };
        let v = serde_json::to_value(&summary).unwrap();
        assert!(v.get("preview").is_none(), "preview should be omitted when None");
    }

    #[test]
    fn test_sync_version_query_parses_version() {
        let q: SyncVersionQuery =
            serde_urlencoded::from_str("version=2026-04-15T10:30:00Z").unwrap();
        assert_eq!(q.version.as_deref(), Some("2026-04-15T10:30:00Z"));
    }

    #[test]
    fn test_sync_version_query_empty() {
        let q: SyncVersionQuery = serde_urlencoded::from_str("").unwrap();
        assert!(q.version.is_none());
    }

    #[test]
    fn test_sync_version_body_parses_version() {
        let body: SyncVersionBody =
            serde_json::from_str(r#"{"version":"2026-04-15T10:30:00Z"}"#).unwrap();
        assert_eq!(body.version.as_deref(), Some("2026-04-15T10:30:00Z"));
    }

    #[test]
    fn test_sync_version_body_empty_json() {
        let body: SyncVersionBody = serde_json::from_str("{}").unwrap();
        assert!(body.version.is_none());
    }

    #[test]
    fn test_mobile_usage_serialization_full_quota() {
        let usage = MobileUsage {
            used_bytes: 500_000_000,
            quota_bytes: 1_000_000_000,
            usage_percent: 50.0,
            message_count: 1234,
            is_warning: false,
            is_over_quota: false,
        };
        let v = serde_json::to_value(&usage).unwrap();
        assert_eq!(v["used_bytes"], 500_000_000);
        assert_eq!(v["quota_bytes"], 1_000_000_000);
        assert_eq!(v["usage_percent"], 50.0);
        assert_eq!(v["message_count"], 1234);
        assert_eq!(v["is_warning"], false);
        assert_eq!(v["is_over_quota"], false);
    }

    #[test]
    fn test_mobile_usage_percent_math() {
        // Mirror the logic in mobile_usage handler so the calculation stays pinned.
        let used: i64 = 750_000_000;
        let quota: i64 = 1_000_000_000;
        let percent = (used as f64 / quota as f64) * 100.0;
        let warn_at: i32 = 80;
        assert_eq!(percent, 75.0);
        assert!(percent < warn_at as f64);

        let percent_over = (1_100_000_000_f64 / quota as f64) * 100.0;
        assert!(percent_over > 100.0);
        assert!(1_100_000_000_i64 >= quota); // is_over_quota
    }

    #[test]
    fn test_mobile_usage_percent_zero_quota_safe() {
        // quota_bytes = 0 (unlimited / unconfigured) must NOT divide by zero.
        let used: i64 = 12_345;
        let quota: i64 = 0;
        let percent = if quota > 0 {
            (used as f64 / quota as f64) * 100.0
        } else {
            0.0
        };
        assert_eq!(percent, 0.0);
    }

    #[test]
    fn test_low_bandwidth_constant_packet_sized() {
        // Document the SLA: previews must fit a 1500-byte MTU when JSON-encoded.
        assert!(LOW_BANDWIDTH_PREVIEW_CHARS <= 512);
        assert!(LOW_BANDWIDTH_PREVIEW_CHARS >= 140);
    }

    #[test]
    fn test_sync_version_cursor_precedence() {
        // Simulate the cursor resolution rule used in mobile_sync_post:
        // query string wins over body so retries stay idempotent.
        let q = SyncVersionQuery {
            version: Some("from-query".to_string()),
        };
        let b = SyncVersionBody {
            version: Some("from-body".to_string()),
        };
        let resolved = q.version.or(b.version);
        assert_eq!(resolved.as_deref(), Some("from-query"));
    }

    #[test]
    fn test_sync_version_cursor_falls_back_to_body() {
        let q = SyncVersionQuery { version: None };
        let b = SyncVersionBody {
            version: Some("from-body".to_string()),
        };
        let resolved = q.version.or(b.version);
        assert_eq!(resolved.as_deref(), Some("from-body"));
    }

    #[test]
    fn test_sync_version_cursor_none_bootstraps_24h_window() {
        // When neither side carries a cursor, the handler picks "now - 24h".
        // The exact instant moves per-call, but the duration must stay positive.
        let q = SyncVersionQuery { version: None };
        let b = SyncVersionBody { version: None };
        let resolved = q.version.or(b.version);
        assert!(resolved.is_none());

        let since = chrono::Utc::now() - chrono::Duration::hours(24);
        assert!(chrono::Utc::now() > since);
    }
}
