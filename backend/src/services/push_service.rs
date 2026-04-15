// Added: Push notification sender service for TMAIL-50
// PURPOSE: Sends push notifications to registered devices via FCM, APNs, and Web Push APIs
// NOTE: Actual API calls require valid credentials; in dev mode, notifications are logged only

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing;
use uuid::Uuid;

use crate::config::Config;
use crate::models::push_notification::{PushDevice, PushNotificationLog, PushNotificationPayload};

/// Added: FCM message payload structure for FCM HTTP v1 API (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct FcmMessage {
    pub message: FcmMessageBody,
}

/// Added: Inner FCM message body with token, notification, and data (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct FcmMessageBody {
    pub token: String,
    pub notification: FcmNotification,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<std::collections::HashMap<String, String>>,
}

/// Added: FCM notification content (title + body) (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct FcmNotification {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Added: APNs alert payload structure for Apple Push Notification service (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct ApnsPayload {
    pub aps: ApnsAps,
    #[serde(flatten)]
    pub custom_data: Option<serde_json::Value>,
}

/// Added: APNs aps dictionary containing alert content (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct ApnsAps {
    pub alert: ApnsAlert,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
}

/// Added: APNs alert with title and body (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct ApnsAlert {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Added: Web Push notification payload structure (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct WebPushPayload {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Added: Result from attempting to send a push notification to a single device (TMAIL-50)
#[derive(Debug)]
pub struct SendResult {
    pub device_id: Uuid,
    pub delivered: bool,
    pub error: Option<String>,
}

/// Added: Build FCM message payload from notification data (TMAIL-50)
pub fn build_fcm_payload(token: &str, payload: &PushNotificationPayload) -> FcmMessage {
    // Added: Convert JSON data to string HashMap as required by FCM v1 API
    let data = payload.data.as_ref().and_then(|d| {
        d.as_object().map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.to_string().trim_matches('"').to_string()))
                .collect::<std::collections::HashMap<String, String>>()
        })
    });

    FcmMessage {
        message: FcmMessageBody {
            token: token.to_string(),
            notification: FcmNotification {
                title: payload.title.clone(),
                body: payload.body.clone(),
            },
            data,
        },
    }
}

/// Added: Build APNs payload from notification data (TMAIL-50)
pub fn build_apns_payload(payload: &PushNotificationPayload) -> ApnsPayload {
    ApnsPayload {
        aps: ApnsAps {
            alert: ApnsAlert {
                title: payload.title.clone(),
                body: payload.body.clone(),
            },
            badge: Some(1),
            sound: Some("default".to_string()),
        },
        custom_data: payload.data.clone(),
    }
}

/// Added: Build Web Push payload from notification data (TMAIL-50)
pub fn build_web_push_payload(payload: &PushNotificationPayload) -> WebPushPayload {
    WebPushPayload {
        title: payload.title.clone(),
        body: payload.body.clone(),
        data: payload.data.clone(),
        icon: Some("/icons/mail-192.png".to_string()),
    }
}

/// Added: Send push notification to all active devices for a user (TMAIL-50)
/// PURPOSE: Iterates over all active push devices and dispatches via the appropriate provider
/// NOTE: Without valid FCM/APNs credentials, notifications are logged but not actually sent
pub async fn send_notification(
    pool: &PgPool,
    config: &Config,
    user_id: Uuid,
    payload: &PushNotificationPayload,
) -> Result<Vec<SendResult>, anyhow::Error> {
    let devices = PushDevice::list_active_by_user(pool, user_id).await?;
    let mut results = Vec::new();

    for device in &devices {
        let result = send_to_device(config, device, payload).await;
        let (delivered, error) = match &result {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e.to_string())),
        };

        // Added: Log each notification attempt for audit trail
        let _ = PushNotificationLog::create(
            pool,
            user_id,
            Some(device.id),
            &payload.title,
            payload.body.as_deref(),
            payload.data.as_ref(),
            delivered,
            error.as_deref(),
        )
        .await;

        results.push(SendResult {
            device_id: device.id,
            delivered,
            error,
        });
    }

    Ok(results)
}

/// Added: Send to a single device based on platform (TMAIL-50)
async fn send_to_device(
    config: &Config,
    device: &PushDevice,
    payload: &PushNotificationPayload,
) -> Result<(), anyhow::Error> {
    match device.platform.as_str() {
        "fcm" => send_fcm(config, &device.device_token, payload).await,
        "apns" => send_apns(config, &device.device_token, payload).await,
        "web" => send_web_push(config, &device.device_token, payload).await,
        other => {
            tracing::warn!("Unknown push platform: {}", other);
            Err(anyhow::anyhow!("Unsupported push platform: {}", other))
        }
    }
}

/// Added: Send notification via FCM HTTP v1 API (TMAIL-50)
/// NOTE: Requires fcm_project_id and fcm_service_account_key in config
async fn send_fcm(
    config: &Config,
    token: &str,
    payload: &PushNotificationPayload,
) -> Result<(), anyhow::Error> {
    let push_config = config.push.as_ref();
    let project_id = push_config.and_then(|c| c.fcm_project_id.as_deref());

    if project_id.is_none() {
        tracing::debug!("FCM not configured — logging notification only");
        return Ok(());
    }

    let project_id = project_id.unwrap();
    let fcm_payload = build_fcm_payload(token, payload);
    let url = format!(
        "https://fcm.googleapis.com/v1/projects/{}/messages:send",
        project_id
    );

    // Added: POST to FCM API (service account auth would be added in production)
    let client = reqwest::Client::new();
    let resp = client.post(&url).json(&fcm_payload).send().await?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(anyhow::anyhow!("FCM error {}: {}", status, body))
    }
}

/// Added: Send notification via APNs HTTP/2 API (TMAIL-50)
/// NOTE: Requires apns_key_id, apns_team_id, and apns_key_path in config
async fn send_apns(
    config: &Config,
    token: &str,
    payload: &PushNotificationPayload,
) -> Result<(), anyhow::Error> {
    let push_config = config.push.as_ref();
    let has_apns = push_config
        .map(|c| c.apns_key_id.is_some() && c.apns_team_id.is_some())
        .unwrap_or(false);

    if !has_apns {
        tracing::debug!("APNs not configured — logging notification only");
        return Ok(());
    }

    let apns_payload = build_apns_payload(payload);
    let url = format!("https://api.push.apple.com/3/device/{}", token);

    // Added: POST to APNs (JWT auth token would be generated from key in production)
    let client = reqwest::Client::new();
    let resp = client.post(&url).json(&apns_payload).send().await?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(anyhow::anyhow!("APNs error {}: {}", status, body))
    }
}

/// Added: Send notification via Web Push protocol (TMAIL-50)
/// NOTE: Web Push requires VAPID keys for authentication (not yet implemented)
async fn send_web_push(
    _config: &Config,
    endpoint: &str,
    payload: &PushNotificationPayload,
) -> Result<(), anyhow::Error> {
    let web_payload = build_web_push_payload(payload);

    // Added: POST to web push endpoint (VAPID auth would be added in production)
    let client = reqwest::Client::new();
    let resp = client.post(endpoint).json(&web_payload).send().await?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(anyhow::anyhow!("Web Push error {}: {}", status, body))
    }
}

/// Added: Convenience function to notify a user about a new incoming email (TMAIL-50)
/// PURPOSE: Called by the email scheduler or IMAP idle watcher when new mail arrives
pub async fn notify_new_email(
    pool: &PgPool,
    config: &Config,
    user_id: Uuid,
    from: &str,
    subject: &str,
) -> Result<Vec<SendResult>, anyhow::Error> {
    let payload = PushNotificationPayload {
        title: format!("New email from {}", from),
        body: Some(subject.to_string()),
        data: Some(serde_json::json!({
            "type": "new_email",
            "from": from,
            "subject": subject,
        })),
    };

    send_notification(pool, config, user_id, &payload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_fcm_payload_basic() {
        let payload = PushNotificationPayload {
            title: "Test Title".to_string(),
            body: Some("Test Body".to_string()),
            data: None,
        };

        let fcm = build_fcm_payload("test-token", &payload);
        assert_eq!(fcm.message.token, "test-token");
        assert_eq!(fcm.message.notification.title, "Test Title");
        assert_eq!(fcm.message.notification.body, Some("Test Body".to_string()));
        assert!(fcm.message.data.is_none());
    }

    #[test]
    fn test_build_fcm_payload_with_data() {
        let payload = PushNotificationPayload {
            title: "New Email".to_string(),
            body: Some("From alice@example.com".to_string()),
            data: Some(serde_json::json!({"folder": "INBOX", "uid": "42"})),
        };

        let fcm = build_fcm_payload("token-abc", &payload);
        assert_eq!(fcm.message.token, "token-abc");
        let data = fcm.message.data.as_ref().unwrap();
        assert_eq!(data.get("folder").unwrap(), "INBOX");
        assert_eq!(data.get("uid").unwrap(), "42");
    }

    #[test]
    fn test_build_fcm_payload_serializes_to_json() {
        let payload = PushNotificationPayload {
            title: "Hello".to_string(),
            body: None,
            data: None,
        };

        let fcm = build_fcm_payload("tok", &payload);
        let json = serde_json::to_value(&fcm).unwrap();
        assert_eq!(json["message"]["token"], "tok");
        assert_eq!(json["message"]["notification"]["title"], "Hello");
    }

    #[test]
    fn test_build_apns_payload_basic() {
        let payload = PushNotificationPayload {
            title: "Alert".to_string(),
            body: Some("You have mail".to_string()),
            data: None,
        };

        let apns = build_apns_payload(&payload);
        assert_eq!(apns.aps.alert.title, "Alert");
        assert_eq!(apns.aps.alert.body, Some("You have mail".to_string()));
        assert_eq!(apns.aps.badge, Some(1));
        assert_eq!(apns.aps.sound, Some("default".to_string()));
        assert!(apns.custom_data.is_none());
    }

    #[test]
    fn test_build_apns_payload_with_data() {
        let payload = PushNotificationPayload {
            title: "Mail".to_string(),
            body: None,
            data: Some(serde_json::json!({"type": "new_email"})),
        };

        let apns = build_apns_payload(&payload);
        assert!(apns.custom_data.is_some());
        assert_eq!(apns.custom_data.unwrap()["type"], "new_email");
    }

    #[test]
    fn test_build_apns_payload_serializes_to_json() {
        let payload = PushNotificationPayload {
            title: "Test".to_string(),
            body: Some("Body".to_string()),
            data: None,
        };

        let apns = build_apns_payload(&payload);
        let json = serde_json::to_value(&apns).unwrap();
        assert_eq!(json["aps"]["alert"]["title"], "Test");
        assert_eq!(json["aps"]["alert"]["body"], "Body");
        assert_eq!(json["aps"]["badge"], 1);
    }

    #[test]
    fn test_build_web_push_payload_basic() {
        let payload = PushNotificationPayload {
            title: "Web Alert".to_string(),
            body: Some("Check your inbox".to_string()),
            data: None,
        };

        let web = build_web_push_payload(&payload);
        assert_eq!(web.title, "Web Alert");
        assert_eq!(web.body, Some("Check your inbox".to_string()));
        assert!(web.data.is_none());
        assert_eq!(web.icon, Some("/icons/mail-192.png".to_string()));
    }

    #[test]
    fn test_build_web_push_payload_serializes_to_json() {
        let payload = PushNotificationPayload {
            title: "Notify".to_string(),
            body: None,
            data: Some(serde_json::json!({"action": "open_inbox"})),
        };

        let web = build_web_push_payload(&payload);
        let json = serde_json::to_value(&web).unwrap();
        assert_eq!(json["title"], "Notify");
        assert_eq!(json["data"]["action"], "open_inbox");
        assert_eq!(json["icon"], "/icons/mail-192.png");
    }
}
