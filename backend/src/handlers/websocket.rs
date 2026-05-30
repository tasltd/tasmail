use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

use crate::services::auth_service;
use crate::services::imap_idle_bridge::{self, IdleSubscription, MAX_IDLE_PER_USER};
use crate::state::AppState;

/// Query parameters for WebSocket connection — token passed as query param
/// since WebSocket doesn't support Authorization headers during handshake
#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub token: String,
}

/// Events pushed to the client via WebSocket
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum WsEvent {
    /// New email arrived in a folder
    #[serde(rename = "new_mail")]
    NewMail { folder: String, count: u32 },
    /// Folder unread counts updated
    #[serde(rename = "unread_update")]
    UnreadUpdate { folder: String, unread: u32 },
    /// Mailbox quota changed
    #[serde(rename = "quota_update")]
    QuotaUpdate { used_bytes: u64, total_bytes: u64 },
    /// Heartbeat to keep connection alive
    #[serde(rename = "ping")]
    Ping { timestamp: i64 },
    /// Error notification
    #[serde(rename = "error")]
    Error { message: String },
}

/// Buffer size for the per-connection event channel that the IMAP IDLE
/// bridge tasks push events through. Small because consumers (the WS
/// sender loop) drain quickly; a backlog this large already means the
/// browser is offline and a reconnect will follow.
const EVENT_CHANNEL_BUFFER: usize = 64;

/// GET /ws — WebSocket endpoint for real-time push notifications
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<WsParams>,
) -> impl IntoResponse {
    // Validate JWT token before upgrading the connection
    let claims = match auth_service::validate_access_token(&state.config.jwt, &params.token) {
        Ok(claims) => claims,
        Err(_) => {
            return axum::response::Response::builder()
                .status(401)
                .body(axum::body::Body::from("Invalid token"))
                .unwrap()
                .into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(socket, state, claims.sub))
}

/// Handle an active WebSocket connection.
///
/// Spawns IMAP IDLE bridge tasks for each `subscribe:<FOLDER>` command the
/// client sends (capped at `MAX_IDLE_PER_USER` concurrent folders). Events
/// from those tasks flow through an mpsc channel into the WS sender loop
/// below, so `WsEvent::NewMail` / `UnreadUpdate` actually fire when mail
/// lands on the user's BYOK IMAP server.
async fn handle_socket(socket: WebSocket, state: AppState, user_id: String) {
    let (mut sender, mut receiver) = socket.split();

    tracing::info!("WebSocket connected for user {}", user_id);

    // Parse the JWT subject into a Uuid up front — the IMAP bridge needs it
    // for ImapService::for_user. An invalid UUID here means the token claims
    // were malformed; we just close the socket politely.
    let user_uuid = match uuid::Uuid::parse_str(&user_id) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("WebSocket: invalid user_id UUID in JWT claims: {}", e);
            let err = WsEvent::Error {
                message: "Invalid user id in token".to_string(),
            };
            if let Ok(json) = serde_json::to_string(&err) {
                let _ = sender.send(Message::Text(json.into())).await;
            }
            return;
        }
    };

    // Event channel shared by all IDLE subscriptions for this WS connection.
    // The sender clone is handed to each spawned IDLE task; when the WS
    // closes we drop our last sender and the tasks observe `tx.closed()`.
    let (event_tx, mut event_rx) = mpsc::channel::<WsEvent>(EVENT_CHANNEL_BUFFER);

    // Active IDLE subscriptions keyed by folder name. Dropping an entry
    // cancels and aborts the corresponding task via IdleSubscription::Drop.
    let mut subs: HashMap<String, IdleSubscription> = HashMap::new();

    let mut heartbeat = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            // Inbound WS frames from the client (subscribe / unsubscribe / close).
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text = text.to_string();
                        if let Some(folder) = text.strip_prefix("subscribe:") {
                            handle_subscribe(
                                &state,
                                user_uuid,
                                folder.to_string(),
                                &event_tx,
                                &mut subs,
                                &mut sender,
                            ).await;
                        } else if let Some(folder) = text.strip_prefix("unsubscribe:") {
                            // Drop the subscription so its IdleSubscription::Drop
                            // signals cancel + aborts the task.
                            if subs.remove(folder).is_some() {
                                tracing::debug!(
                                    "WebSocket user {} unsubscribed from {}",
                                    user_id, folder
                                );
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!("WebSocket closed for user {}", user_id);
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!("WebSocket error for user {}: {}", user_id, e);
                        break;
                    }
                    _ => {}
                }
            }
            // Events from any IDLE bridge task — forward to the client.
            evt = event_rx.recv() => {
                match evt {
                    Some(event) => {
                        if let Ok(json) = serde_json::to_string(&event) {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    None => {
                        // event_tx clones are all gone — should not happen while
                        // the WS handler still holds the original.
                        break;
                    }
                }
            }
            // Heartbeat ping so proxies / browsers don't tear down the socket.
            _ = heartbeat.tick() => {
                let event = WsEvent::Ping {
                    timestamp: chrono::Utc::now().timestamp(),
                };
                if let Ok(json) = serde_json::to_string(&event) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    // Drop all subscriptions (cancels + aborts each task) before exiting,
    // so we don't leak IMAP connections to the upstream server.
    subs.clear();
}

/// Spawn an IDLE bridge task for `folder` and add it to `subs`.
///
/// Enforces `MAX_IDLE_PER_USER`, deduplicates re-subscribes, and forwards
/// any startup error (auth/config/folder-not-found) to the client as
/// `WsEvent::Error` so the SPA can surface it.
async fn handle_subscribe(
    state: &AppState,
    user_uuid: uuid::Uuid,
    folder: String,
    event_tx: &mpsc::Sender<WsEvent>,
    subs: &mut HashMap<String, IdleSubscription>,
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
) {
    // Idempotent: if an existing subscription is still healthy, leave it alone.
    if let Some(existing) = subs.get(&folder) {
        if existing.is_running() {
            tracing::debug!(
                "WebSocket user {} already subscribed to {}",
                user_uuid, folder
            );
            return;
        }
        // Stale entry (task died) — drop it before re-subscribing.
        subs.remove(&folder);
    }

    if subs.len() >= MAX_IDLE_PER_USER {
        let err = WsEvent::Error {
            message: format!(
                "Subscription limit reached ({}). Unsubscribe from another folder first.",
                MAX_IDLE_PER_USER
            ),
        };
        if let Ok(json) = serde_json::to_string(&err) {
            let _ = sender.send(Message::Text(json.into())).await;
        }
        return;
    }

    match imap_idle_bridge::subscribe(
        state.clone(),
        user_uuid,
        folder.clone(),
        event_tx.clone(),
    )
    .await
    {
        Ok(sub) => {
            tracing::debug!("WebSocket user {} subscribed to {}", user_uuid, folder);
            subs.insert(folder, sub);
        }
        Err(e) => {
            tracing::warn!(
                "WebSocket user {} subscribe to {} failed: {}",
                user_uuid, folder, e
            );
            let err = WsEvent::Error {
                message: format!("Failed to subscribe to {}: {}", folder, e),
            };
            if let Ok(json) = serde_json::to_string(&err) {
                let _ = sender.send(Message::Text(json.into())).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_params_deserialization() {
        let json = r#"{"token": "eyJhbGciOiJSUzI1NiJ9.test"}"#;
        let params: WsParams = serde_json::from_str(json).unwrap();
        assert!(params.token.starts_with("eyJ"));
    }

    #[test]
    fn test_ws_event_new_mail_serialization() {
        let event = WsEvent::NewMail {
            folder: "INBOX".to_string(),
            count: 3,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"new_mail\""));
        assert!(json.contains("\"folder\":\"INBOX\""));
        assert!(json.contains("\"count\":3"));
    }

    #[test]
    fn test_ws_event_unread_update_serialization() {
        let event = WsEvent::UnreadUpdate {
            folder: "INBOX".to_string(),
            unread: 5,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"unread_update\""));
        assert!(json.contains("\"unread\":5"));
    }

    #[test]
    fn test_ws_event_quota_update_serialization() {
        let event = WsEvent::QuotaUpdate {
            used_bytes: 1024 * 1024 * 500,
            total_bytes: 1024 * 1024 * 1024,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"quota_update\""));
        assert!(json.contains("\"used_bytes\":524288000"));
    }

    #[test]
    fn test_ws_event_ping_serialization() {
        let event = WsEvent::Ping { timestamp: 1712000000 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"ping\""));
        assert!(json.contains("\"timestamp\":1712000000"));
    }

    #[test]
    fn test_ws_event_error_serialization() {
        let event = WsEvent::Error {
            message: "Connection lost".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("Connection lost"));
    }

    #[test]
    fn test_ws_subscribe_command_parsing() {
        // The handler now uses str::strip_prefix; assert that it returns
        // the folder portion on a well-formed command and None otherwise.
        assert_eq!("subscribe:INBOX".strip_prefix("subscribe:"), Some("INBOX"));
        assert_eq!("garbage".strip_prefix("subscribe:"), None);
    }

    #[test]
    fn test_ws_unsubscribe_command_parsing() {
        // Added (TMAIL-302): the handler now also honours `unsubscribe:<folder>`
        // so the client can release an IDLE slot without disconnecting.
        assert_eq!(
            "unsubscribe:INBOX".strip_prefix("unsubscribe:"),
            Some("INBOX")
        );
        assert_eq!("subscribe:INBOX".strip_prefix("unsubscribe:"), None);
    }

    #[test]
    fn test_ws_subscribe_nested_folder() {
        let cmd = "subscribe:INBOX.Subfolder.Deep";
        assert_eq!(
            cmd.strip_prefix("subscribe:"),
            Some("INBOX.Subfolder.Deep")
        );
    }
}
