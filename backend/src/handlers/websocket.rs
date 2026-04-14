use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::interval;

use crate::services::auth_service;
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

/// Handle an active WebSocket connection
async fn handle_socket(socket: WebSocket, _state: AppState, user_id: String) {
    let (mut sender, mut receiver) = socket.split();

    tracing::info!("WebSocket connected for user {}", user_id);

    // Added: heartbeat interval to keep the connection alive (30s)
    let mut heartbeat = interval(Duration::from_secs(30));

    // Added: simulated IMAP IDLE poll interval (10s)
    // NOTE: In production, this would be replaced by actual IMAP IDLE connections
    // that push events when new mail arrives. For now, we send periodic heartbeats.
    let mut idle_poll = interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            // Handle incoming messages from client
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Client can send commands like "subscribe:INBOX"
                        if text.starts_with("subscribe:") {
                            let folder = text.trim_start_matches("subscribe:");
                            tracing::debug!("User {} subscribed to folder: {}", user_id, folder);
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
            // Send heartbeat
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
            // Poll for new mail (placeholder for IMAP IDLE bridge)
            _ = idle_poll.tick() => {
                // NOTE: In production, this would listen to IMAP IDLE notifications
                // from the Dovecot server and push new_mail events to the client.
                // The IMAP IDLE bridge would maintain a persistent connection to the
                // IMAP server per connected WebSocket client.
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
        let cmd = "subscribe:INBOX";
        assert!(cmd.starts_with("subscribe:"));
        assert_eq!(cmd.trim_start_matches("subscribe:"), "INBOX");
    }

    #[test]
    fn test_ws_subscribe_nested_folder() {
        let cmd = "subscribe:INBOX.Subfolder.Deep";
        assert_eq!(cmd.trim_start_matches("subscribe:"), "INBOX.Subfolder.Deep");
    }
}
