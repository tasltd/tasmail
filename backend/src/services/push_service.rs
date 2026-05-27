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
    // Added: TMAIL-50 — Android-specific options used for sender/thread grouping
    // (collapse_key) and to pass the badge count via data.badge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<FcmAndroidConfig>,
    // Added: TMAIL-50 — APNs-specific options when FCM proxies to APNs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apns: Option<FcmApnsConfig>,
}

/// Added: FCM android-specific options (TMAIL-50)
/// PURPOSE: collapse_key coalesces notifications with the same key into a single
/// entry in the system tray — used to group by sender or thread.
#[derive(Debug, Serialize)]
pub struct FcmAndroidConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapse_key: Option<String>,
}

/// Added: FCM apns-specific options (TMAIL-50)
/// PURPOSE: apns-collapse-id is the iOS equivalent of FCM android.collapse_key.
#[derive(Debug, Serialize)]
pub struct FcmApnsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
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
    // Added: TMAIL-50 — thread-id groups notifications by sender/thread in
    // iOS Notification Center. Stacked in the same summary card.
    #[serde(rename = "thread-id", skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
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
/// PURPOSE: badge_override falls through to FCM data.badge so the Flutter client
/// can update the in-app badge even when FCM does not natively carry one.
pub fn build_fcm_payload(
    token: &str,
    payload: &PushNotificationPayload,
    badge_override: Option<u32>,
) -> FcmMessage {
    // Added: Convert JSON data to string HashMap as required by FCM v1 API
    let mut data: std::collections::HashMap<String, String> = payload
        .data
        .as_ref()
        .and_then(|d| {
            d.as_object().map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.to_string().trim_matches('"').to_string()))
                    .collect()
            })
        })
        .unwrap_or_default();

    // Added: TMAIL-50 — surface badge in data so Flutter can read it cross-platform
    if let Some(b) = payload.badge.or(badge_override) {
        data.insert("badge".to_string(), b.to_string());
    }

    let android = payload.collapse_key.as_ref().map(|key| FcmAndroidConfig {
        collapse_key: Some(key.clone()),
    });

    // Added: TMAIL-50 — mirror collapse_key to apns-collapse-id when FCM proxies to APNs
    let apns = payload.collapse_key.as_ref().map(|key| {
        let mut headers = std::collections::HashMap::new();
        headers.insert("apns-collapse-id".to_string(), key.clone());
        FcmApnsConfig {
            headers: Some(headers),
        }
    });

    FcmMessage {
        message: FcmMessageBody {
            token: token.to_string(),
            notification: FcmNotification {
                title: payload.title.clone(),
                body: payload.body.clone(),
            },
            data: if data.is_empty() { None } else { Some(data) },
            android,
            apns,
        },
    }
}

/// Added: Build APNs payload from notification data (TMAIL-50)
/// PURPOSE: badge_override takes effect when the payload itself doesn't supply
/// one — typically the PushDevice.badge_count synced from the client.
pub fn build_apns_payload(
    payload: &PushNotificationPayload,
    badge_override: Option<u32>,
) -> ApnsPayload {
    ApnsPayload {
        aps: ApnsAps {
            alert: ApnsAlert {
                title: payload.title.clone(),
                body: payload.body.clone(),
            },
            // NOTE: payload.badge wins over device-level badge_override, which wins
            // over the legacy default of 1
            badge: Some(payload.badge.or(badge_override).unwrap_or(1)),
            sound: Some("default".to_string()),
            thread_id: payload.collapse_key.clone(),
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
/// PURPOSE: Iterates over all active push devices and dispatches via the appropriate provider.
/// Skips devices currently inside their quiet-hours window (logged with "quiet_hours" reason).
/// NOTE: Without valid FCM/APNs credentials, notifications are logged but not actually sent
pub async fn send_notification(
    pool: &PgPool,
    config: &Config,
    user_id: Uuid,
    payload: &PushNotificationPayload,
) -> Result<Vec<SendResult>, anyhow::Error> {
    let devices = PushDevice::list_active_by_user(pool, user_id).await?;
    let now = chrono::Utc::now();
    let mut results = Vec::new();

    for device in &devices {
        // Added: TMAIL-50 — honour quiet hours by skipping the send entirely
        if device.is_in_quiet_hours_now(now) {
            tracing::debug!(
                device_id = %device.id,
                "Skipping push notification — device is in quiet hours"
            );
            let _ = PushNotificationLog::create(
                pool,
                user_id,
                Some(device.id),
                &payload.title,
                payload.body.as_deref(),
                payload.data.as_ref(),
                false,
                Some("quiet_hours"),
            )
            .await;
            results.push(SendResult {
                device_id: device.id,
                delivered: false,
                error: Some("quiet_hours".to_string()),
            });
            continue;
        }

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
    // Added: TMAIL-50 — fall back to the device's synced badge count when the
    // caller didn't supply one in the payload.
    let device_badge = u32::try_from(device.badge_count).ok();
    match device.platform.as_str() {
        "fcm" => send_fcm(config, &device.device_token, payload, device_badge).await,
        "apns" => send_apns(config, &device.device_token, payload, device_badge).await,
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
    badge_override: Option<u32>,
) -> Result<(), anyhow::Error> {
    let push_config = config.push.as_ref();
    let project_id = push_config.and_then(|c| c.fcm_project_id.as_deref());

    if project_id.is_none() {
        tracing::debug!("FCM not configured — logging notification only");
        return Ok(());
    }

    let project_id = project_id.unwrap();
    let fcm_payload = build_fcm_payload(token, payload, badge_override);
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
    badge_override: Option<u32>,
) -> Result<(), anyhow::Error> {
    let push_config = config.push.as_ref();
    let has_apns = push_config
        .map(|c| c.apns_key_id.is_some() && c.apns_team_id.is_some())
        .unwrap_or(false);

    if !has_apns {
        tracing::debug!("APNs not configured — logging notification only");
        return Ok(());
    }

    let apns_payload = build_apns_payload(payload, badge_override);
    let url = format!("https://api.push.apple.com/3/device/{}", token);

    // Added: POST to APNs (JWT auth token would be generated from key in production)
    // NOTE: TMAIL-50 — if payload.collapse_key is set, mirror it as apns-collapse-id
    // so APNs coalesces same-thread/same-sender notifications.
    let client = reqwest::Client::new();
    let mut req = client.post(&url).json(&apns_payload);
    if let Some(key) = payload.collapse_key.as_deref() {
        req = req.header("apns-collapse-id", key);
    }
    let resp = req.send().await?;

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
/// PURPOSE: Called by the email scheduler or IMAP idle watcher when new mail arrives.
/// Groups by sender by default; pass `thread_id` to group by IMAP thread instead.
pub async fn notify_new_email(
    pool: &PgPool,
    config: &Config,
    user_id: Uuid,
    from: &str,
    subject: &str,
    thread_id: Option<&str>,
) -> Result<Vec<SendResult>, anyhow::Error> {
    // NOTE: TMAIL-50 — prefer thread grouping when we know the thread,
    // otherwise fall back to per-sender grouping.
    let collapse_key = thread_id
        .map(|t| format!("thread:{}", t))
        .unwrap_or_else(|| format!("sender:{}", from));

    let payload = PushNotificationPayload {
        title: format!("New email from {}", from),
        body: Some(subject.to_string()),
        data: Some(serde_json::json!({
            "type": "new_email",
            "from": from,
            "subject": subject,
            "thread_id": thread_id,
        })),
        collapse_key: Some(collapse_key),
        badge: None,
    };

    send_notification(pool, config, user_id, &payload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(title: &str) -> PushNotificationPayload {
        PushNotificationPayload {
            title: title.to_string(),
            body: Some("body".to_string()),
            data: None,
            collapse_key: None,
            badge: None,
        }
    }

    #[test]
    fn test_build_fcm_payload_basic() {
        let mut p = payload("Test Title");
        p.body = Some("Test Body".to_string());
        let fcm = build_fcm_payload("test-token", &p, None);
        assert_eq!(fcm.message.token, "test-token");
        assert_eq!(fcm.message.notification.title, "Test Title");
        assert_eq!(fcm.message.notification.body, Some("Test Body".to_string()));
        assert!(fcm.message.data.is_none());
        assert!(fcm.message.android.is_none());
        assert!(fcm.message.apns.is_none());
    }

    #[test]
    fn test_build_fcm_payload_with_data() {
        let mut p = payload("New Email");
        p.body = Some("From alice@example.com".to_string());
        p.data = Some(serde_json::json!({"folder": "INBOX", "uid": "42"}));

        let fcm = build_fcm_payload("token-abc", &p, None);
        assert_eq!(fcm.message.token, "token-abc");
        let data = fcm.message.data.as_ref().unwrap();
        assert_eq!(data.get("folder").unwrap(), "INBOX");
        assert_eq!(data.get("uid").unwrap(), "42");
    }

    #[test]
    fn test_build_fcm_payload_serializes_to_json() {
        let mut p = payload("Hello");
        p.body = None;
        let fcm = build_fcm_payload("tok", &p, None);
        let json = serde_json::to_value(&fcm).unwrap();
        assert_eq!(json["message"]["token"], "tok");
        assert_eq!(json["message"]["notification"]["title"], "Hello");
    }

    // Added: TMAIL-50 — collapse_key propagates to android.collapse_key and
    // apns.headers["apns-collapse-id"] so FCM-proxied iOS clients still group.
    #[test]
    fn test_build_fcm_payload_with_collapse_key() {
        let mut p = payload("Re: project");
        p.collapse_key = Some("sender:alice@example.com".to_string());
        let fcm = build_fcm_payload("tok", &p, None);
        let json = serde_json::to_value(&fcm).unwrap();
        assert_eq!(
            json["message"]["android"]["collapse_key"],
            "sender:alice@example.com"
        );
        assert_eq!(
            json["message"]["apns"]["headers"]["apns-collapse-id"],
            "sender:alice@example.com"
        );
    }

    // Added: TMAIL-50 — payload badge wins; device override is the fallback.
    #[test]
    fn test_build_fcm_payload_badge_in_data() {
        let mut p = payload("New mail");
        p.badge = Some(3);
        let fcm = build_fcm_payload("tok", &p, Some(99));
        let data = fcm.message.data.as_ref().unwrap();
        assert_eq!(data.get("badge").unwrap(), "3");
    }

    #[test]
    fn test_build_fcm_payload_badge_falls_back_to_device_override() {
        let p = payload("New mail");
        let fcm = build_fcm_payload("tok", &p, Some(7));
        let data = fcm.message.data.as_ref().unwrap();
        assert_eq!(data.get("badge").unwrap(), "7");
    }

    #[test]
    fn test_build_apns_payload_basic() {
        let mut p = payload("Alert");
        p.body = Some("You have mail".to_string());
        let apns = build_apns_payload(&p, None);
        assert_eq!(apns.aps.alert.title, "Alert");
        assert_eq!(apns.aps.alert.body, Some("You have mail".to_string()));
        assert_eq!(apns.aps.badge, Some(1));
        assert_eq!(apns.aps.sound, Some("default".to_string()));
        assert!(apns.aps.thread_id.is_none());
        assert!(apns.custom_data.is_none());
    }

    #[test]
    fn test_build_apns_payload_with_data() {
        let mut p = payload("Mail");
        p.body = None;
        p.data = Some(serde_json::json!({"type": "new_email"}));
        let apns = build_apns_payload(&p, None);
        assert!(apns.custom_data.is_some());
        assert_eq!(apns.custom_data.unwrap()["type"], "new_email");
    }

    #[test]
    fn test_build_apns_payload_serializes_to_json() {
        let mut p = payload("Test");
        p.body = Some("Body".to_string());
        let apns = build_apns_payload(&p, None);
        let json = serde_json::to_value(&apns).unwrap();
        assert_eq!(json["aps"]["alert"]["title"], "Test");
        assert_eq!(json["aps"]["alert"]["body"], "Body");
        assert_eq!(json["aps"]["badge"], 1);
    }

    // Added: TMAIL-50 — collapse_key surfaces as aps.thread-id.
    #[test]
    fn test_build_apns_payload_thread_id_from_collapse_key() {
        let mut p = payload("Re: budget");
        p.collapse_key = Some("thread:abc123".to_string());
        let apns = build_apns_payload(&p, None);
        assert_eq!(apns.aps.thread_id, Some("thread:abc123".to_string()));
        let json = serde_json::to_value(&apns).unwrap();
        assert_eq!(json["aps"]["thread-id"], "thread:abc123");
    }

    // Added: TMAIL-50 — APNs badge precedence: payload > device > default(1).
    #[test]
    fn test_build_apns_payload_badge_precedence() {
        let mut p = payload("Mail");
        p.badge = Some(9);
        assert_eq!(build_apns_payload(&p, Some(99)).aps.badge, Some(9));

        let p2 = payload("Mail");
        assert_eq!(build_apns_payload(&p2, Some(4)).aps.badge, Some(4));

        let p3 = payload("Mail");
        assert_eq!(build_apns_payload(&p3, None).aps.badge, Some(1));
    }

    #[test]
    fn test_build_web_push_payload_basic() {
        let mut p = payload("Web Alert");
        p.body = Some("Check your inbox".to_string());
        let web = build_web_push_payload(&p);
        assert_eq!(web.title, "Web Alert");
        assert_eq!(web.body, Some("Check your inbox".to_string()));
        assert!(web.data.is_none());
        assert_eq!(web.icon, Some("/icons/mail-192.png".to_string()));
    }

    #[test]
    fn test_build_web_push_payload_serializes_to_json() {
        let mut p = payload("Notify");
        p.body = None;
        p.data = Some(serde_json::json!({"action": "open_inbox"}));
        let web = build_web_push_payload(&p);
        let json = serde_json::to_value(&web).unwrap();
        assert_eq!(json["title"], "Notify");
        assert_eq!(json["data"]["action"], "open_inbox");
        assert_eq!(json["icon"], "/icons/mail-192.png");
    }
}
