// Added: Mobile-optimized response models for lightweight API payloads (TMAIL-52)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Default cap on body preview length when the client opts into low-bandwidth mode.
// Picked to fit a single TCP packet (~1.5 KB MSS) so that the message-list payload
// stays under one round trip on 2G/Edge networks common in the Ghana market.
pub const LOW_BANDWIDTH_PREVIEW_CHARS: usize = 280;

/// Minimal message fields for mobile inbox listing — no body content by default.
/// When the client opts into low-bandwidth mode, a short `preview` snippet
/// (LOW_BANDWIDTH_PREVIEW_CHARS) is added so the list view can render meaningful
/// rows without a follow-up `/api/mobile/message/...` call per message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileMessageSummary {
    pub uid: u32,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub date: Option<String>,
    pub is_read: bool,
    pub is_flagged: bool,
    pub has_attachment: bool,
    /// Optional text-only snippet, populated only in low-bandwidth mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// Folder name with unread count only — no message totals or other metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileFolderSummary {
    pub name: String,
    pub unread_count: u32,
}

/// A single request within a batch call
#[derive(Debug, Clone, Deserialize)]
pub struct BatchRequestItem {
    pub method: String,
    pub path: String,
    pub body: Option<serde_json::Value>,
}

/// Wrapper for batch API request
#[derive(Debug, Clone, Deserialize)]
pub struct BatchRequest {
    pub requests: Vec<BatchRequestItem>,
}

/// A single response within a batch call
#[derive(Debug, Clone, Serialize)]
pub struct BatchResponseItem {
    pub status: u16,
    pub body: serde_json::Value,
}

/// Wrapper for batch API response
#[derive(Debug, Clone, Serialize)]
pub struct BatchResponse {
    pub responses: Vec<BatchResponseItem>,
}

/// Represents a single change in delta sync
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncChange {
    /// A new message arrived
    #[serde(rename = "new_message")]
    NewMessage {
        folder: String,
        uid: u32,
        from: Option<String>,
        subject: Option<String>,
        date: Option<String>,
    },
    /// Message flags changed (read, flagged, etc.)
    #[serde(rename = "flag_change")]
    FlagChange {
        folder: String,
        uid: u32,
        flags: Vec<String>,
    },
    /// A message was deleted/expunged
    #[serde(rename = "deletion")]
    Deletion { folder: String, uid: u32 },
}

/// Delta sync response containing all changes since a given timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDelta {
    pub changes: Vec<SyncChange>,
    pub sync_token: String,
    pub has_more: bool,
}

// Added: Pagination query params shared across mobile endpoints.
// `low_bandwidth=true` switches the inbox response to lighter rows that include a
// short text preview and skip optional fields the client can render from cache.
#[derive(Debug, Deserialize, Default)]
pub struct MobileInboxQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    #[serde(default, alias = "low_bw")]
    pub low_bandwidth: Option<bool>,
}

// Query params for the message detail endpoint.
// `low_bandwidth=true` forces text-only output (HTML stripped) and caps the body
// at LOW_BANDWIDTH_PREVIEW_CHARS regardless of `max_body`, so 2G/Edge clients pay
// a bounded cost even when a sender includes a 200 KB HTML newsletter.
#[derive(Debug, Deserialize, Default)]
pub struct MobileMessageQuery {
    pub max_body: Option<usize>,
    #[serde(default, alias = "low_bw")]
    pub low_bandwidth: Option<bool>,
}

// Query param for the GET form of delta sync — clients pass `since=<RFC3339>`
// and get back any changes after that timestamp.
#[derive(Debug, Deserialize)]
pub struct SyncQuery {
    pub since: String,
}

// Query/body for the POST form of delta sync — clients pass `version=<sync_token>`
// from the previous response. Wrapping the cursor in an opaque token (instead of
// a raw timestamp) lets the server change its internal representation later
// (CONDSTORE/QRESYNC MODSEQ, change-log row ID, etc.) without breaking clients.
#[derive(Debug, Deserialize, Default)]
pub struct SyncVersionQuery {
    pub version: Option<String>,
}

// Optional JSON body for POST /api/mobile/sync. Lets clients send the cursor in
// either the query string (?version=...) or the body so retries with large
// version tokens stay under URL length limits on proxies.
#[derive(Debug, Deserialize, Default)]
pub struct SyncVersionBody {
    pub version: Option<String>,
}

// Lightweight quota payload for mobile dashboards. Subset of the full
// `QuotaStatus` returned by `/api/quota`, with only the fields the mobile UI
// renders (used / quota / percent / warning flag / message count). Skips the
// `last_synced_at` timestamp because the mobile client refreshes on demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileUsage {
    pub used_bytes: i64,
    pub quota_bytes: i64,
    pub usage_percent: f64,
    pub message_count: i32,
    pub is_warning: bool,
    pub is_over_quota: bool,
}

// Added: Max number of sub-requests allowed in a single batch call
pub const MAX_BATCH_REQUESTS: usize = 10;

// Added: Allowed methods for batch sub-requests
pub const ALLOWED_BATCH_METHODS: &[&str] = &["GET", "POST", "PUT", "DELETE"];

// Added: Allowed path prefixes for batch sub-requests (security boundary)
pub const ALLOWED_BATCH_PATH_PREFIX: &str = "/api/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_message_summary_serialization() {
        let summary = MobileMessageSummary {
            uid: 42,
            from: Some("kwame@tasmail.gh".to_string()),
            subject: Some("Hello from mobile".to_string()),
            date: Some("2026-04-15T10:30:00Z".to_string()),
            is_read: false,
            is_flagged: true,
            has_attachment: false,
            preview: None,
        };

        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["uid"], 42);
        assert_eq!(json["from"], "kwame@tasmail.gh");
        assert_eq!(json["subject"], "Hello from mobile");
        assert_eq!(json["is_read"], false);
        assert_eq!(json["is_flagged"], true);
        assert_eq!(json["has_attachment"], false);
    }

    #[test]
    fn test_mobile_message_summary_with_nulls() {
        let summary = MobileMessageSummary {
            uid: 1,
            from: None,
            subject: None,
            date: None,
            is_read: true,
            is_flagged: false,
            has_attachment: false,
            preview: None,
        };

        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["uid"], 1);
        assert!(json["from"].is_null());
        assert!(json["subject"].is_null());
        assert_eq!(json["is_read"], true);
    }

    #[test]
    fn test_mobile_folder_summary_serialization() {
        let folder = MobileFolderSummary {
            name: "INBOX".to_string(),
            unread_count: 12,
        };

        let json = serde_json::to_value(&folder).unwrap();
        assert_eq!(json["name"], "INBOX");
        assert_eq!(json["unread_count"], 12);
    }

    #[test]
    fn test_batch_request_deserialization() {
        let json = serde_json::json!({
            "requests": [
                {"method": "GET", "path": "/api/mobile/folders"},
                {"method": "GET", "path": "/api/mobile/unread-count", "body": null}
            ]
        });

        let batch: BatchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(batch.requests.len(), 2);
        assert_eq!(batch.requests[0].method, "GET");
        assert_eq!(batch.requests[0].path, "/api/mobile/folders");
        assert!(batch.requests[0].body.is_none());
        assert_eq!(batch.requests[1].method, "GET");
    }

    #[test]
    fn test_batch_request_with_body() {
        let json = serde_json::json!({
            "requests": [
                {
                    "method": "POST",
                    "path": "/api/messages/send",
                    "body": {"to": ["ama@tasmail.gh"], "subject": "Test"}
                }
            ]
        });

        let batch: BatchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(batch.requests.len(), 1);
        assert!(batch.requests[0].body.is_some());
        let body = batch.requests[0].body.as_ref().unwrap();
        assert_eq!(body["subject"], "Test");
    }

    #[test]
    fn test_batch_response_serialization() {
        let response = BatchResponse {
            responses: vec![
                BatchResponseItem {
                    status: 200,
                    body: serde_json::json!({"folders": []}),
                },
                BatchResponseItem {
                    status: 404,
                    body: serde_json::json!({"error": "Not found"}),
                },
            ],
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["responses"].as_array().unwrap().len(), 2);
        assert_eq!(json["responses"][0]["status"], 200);
        assert_eq!(json["responses"][1]["status"], 404);
    }

    #[test]
    fn test_sync_change_new_message_serialization() {
        let change = SyncChange::NewMessage {
            folder: "INBOX".to_string(),
            uid: 100,
            from: Some("kofi@tasmail.gh".to_string()),
            subject: Some("New email".to_string()),
            date: Some("2026-04-15T12:00:00Z".to_string()),
        };

        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["type"], "new_message");
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["uid"], 100);
    }

    #[test]
    fn test_sync_change_flag_change_serialization() {
        let change = SyncChange::FlagChange {
            folder: "INBOX".to_string(),
            uid: 50,
            flags: vec!["\\Seen".to_string(), "\\Flagged".to_string()],
        };

        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["type"], "flag_change");
        assert_eq!(json["flags"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_sync_change_deletion_serialization() {
        let change = SyncChange::Deletion {
            folder: "INBOX".to_string(),
            uid: 25,
        };

        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["type"], "deletion");
        assert_eq!(json["uid"], 25);
    }

    #[test]
    fn test_sync_delta_serialization() {
        let delta = SyncDelta {
            changes: vec![
                SyncChange::NewMessage {
                    folder: "INBOX".to_string(),
                    uid: 200,
                    from: Some("sender@example.com".to_string()),
                    subject: Some("Test".to_string()),
                    date: None,
                },
                SyncChange::Deletion {
                    folder: "Trash".to_string(),
                    uid: 10,
                },
            ],
            sync_token: "2026-04-15T12:00:00Z".to_string(),
            has_more: false,
        };

        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["changes"].as_array().unwrap().len(), 2);
        assert_eq!(json["sync_token"], "2026-04-15T12:00:00Z");
        assert_eq!(json["has_more"], false);
    }

    #[test]
    fn test_mobile_inbox_query_defaults() {
        let json = serde_json::json!({});
        let query: MobileInboxQuery = serde_json::from_value(json).unwrap();
        assert!(query.page.is_none());
        assert!(query.per_page.is_none());
    }

    #[test]
    fn test_mobile_message_query_with_max_body() {
        let json = serde_json::json!({"max_body": 500});
        let query: MobileMessageQuery = serde_json::from_value(json).unwrap();
        assert_eq!(query.max_body.unwrap(), 500);
    }

    #[test]
    fn test_sync_query_deserialization() {
        let json = serde_json::json!({"since": "2026-04-10T00:00:00Z"});
        let query: SyncQuery = serde_json::from_value(json).unwrap();
        assert_eq!(query.since, "2026-04-10T00:00:00Z");
    }

    #[test]
    fn test_batch_request_empty_requests() {
        let json = serde_json::json!({"requests": []});
        let batch: BatchRequest = serde_json::from_value(json).unwrap();
        assert!(batch.requests.is_empty());
    }

    #[test]
    fn test_batch_request_validates_allowed_methods() {
        // NOTE: This tests the constant, not runtime validation (handler enforces this)
        assert!(ALLOWED_BATCH_METHODS.contains(&"GET"));
        assert!(ALLOWED_BATCH_METHODS.contains(&"POST"));
        assert!(ALLOWED_BATCH_METHODS.contains(&"PUT"));
        assert!(ALLOWED_BATCH_METHODS.contains(&"DELETE"));
        assert!(!ALLOWED_BATCH_METHODS.contains(&"PATCH"));
    }

    #[test]
    fn test_max_batch_requests_limit() {
        assert_eq!(MAX_BATCH_REQUESTS, 10);
    }

    #[test]
    fn test_batch_path_prefix() {
        let valid_path = "/api/mobile/folders";
        let invalid_path = "/internal/admin";
        assert!(valid_path.starts_with(ALLOWED_BATCH_PATH_PREFIX));
        assert!(!invalid_path.starts_with(ALLOWED_BATCH_PATH_PREFIX));
    }
}
