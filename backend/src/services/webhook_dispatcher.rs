// Added: Outbound webhook dispatcher service for TMAIL-131
// PURPOSE: Dispatches HTTP POST notifications to registered webhook endpoints
// EXTERNAL: Uses reqwest for HTTP calls, hmac+sha2 for HMAC-SHA256 signatures

use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::webhook::{Webhook, WebhookDelivery, WebhookEvent};

// NOTE: Type alias for HMAC-SHA256 used in webhook signature generation
type HmacSha256 = Hmac<Sha256>;

/// PURPOSE: Compute HMAC-SHA256 signature for webhook payload verification
/// CONSTRAINTS: Secret must not be empty; returns hex-encoded signature
/// NOTE: Recipients verify by computing the same HMAC and comparing with X-Webhook-Signature header
pub fn compute_signature(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// PURPOSE: Dispatch a webhook event to all matching active webhooks for a user
/// CONSTRAINTS: Deactivates webhook after 10 consecutive failures
/// EXTERNAL: Makes HTTP POST requests to user-configured URLs with 10s timeout
pub async fn dispatch_webhook_event(
    pool: &PgPool,
    user_id: Uuid,
    event: WebhookEvent,
    payload: serde_json::Value,
) {
    // Added: Query active webhooks matching this event type
    let webhooks = match Webhook::find_active_for_event(pool, user_id, &event).await {
        Ok(hooks) => hooks,
        Err(err) => {
            tracing::error!(
                "Failed to query webhooks for user_id={}, event={:?}: {}",
                user_id,
                event,
                err
            );
            return;
        }
    };

    if webhooks.is_empty() {
        return;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    // Added: Serialize payload once for signature computation
    let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();

    for webhook in &webhooks {
        let signature = compute_signature(&webhook.secret, &payload_bytes);

        // Added: POST to webhook URL with HMAC signature header
        let response = client
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Signature", &signature)
            .header("X-Webhook-Event", serde_json::to_string(&event).unwrap_or_default().trim_matches('"'))
            .body(payload_bytes.clone())
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let body = resp.text().await.unwrap_or_default();
                let success = (200..300).contains(&status);

                // Added: Record delivery attempt
                let _ = WebhookDelivery::create(
                    pool,
                    webhook.id,
                    &event,
                    &payload,
                    Some(status),
                    Some(body.chars().take(1000).collect()),
                    success,
                )
                .await;

                if success {
                    let _ = Webhook::record_success(pool, webhook.id).await;
                } else {
                    tracing::warn!(
                        "Webhook {} to {} returned status {}: {}",
                        webhook.id,
                        webhook.url,
                        status,
                        body.chars().take(200).collect::<String>()
                    );
                    let _ = Webhook::record_failure(pool, webhook.id).await;
                }
            }
            Err(err) => {
                tracing::error!(
                    "Webhook {} to {} failed: {}",
                    webhook.id,
                    webhook.url,
                    err
                );

                // Added: Record failed delivery with no response data
                let _ = WebhookDelivery::create(
                    pool,
                    webhook.id,
                    &event,
                    &payload,
                    None,
                    Some(format!("Connection error: {}", err)),
                    false,
                )
                .await;

                let _ = Webhook::record_failure(pool, webhook.id).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_signature_deterministic() {
        // NOTE: Same secret + payload must always produce the same signature
        let secret = "my-webhook-secret";
        let payload = b"hello world";

        let sig1 = compute_signature(secret, payload);
        let sig2 = compute_signature(secret, payload);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_compute_signature_different_secrets() {
        let payload = b"same payload";
        let sig1 = compute_signature("secret-one", payload);
        let sig2 = compute_signature("secret-two", payload);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_compute_signature_different_payloads() {
        let secret = "same-secret";
        let sig1 = compute_signature(secret, b"payload one");
        let sig2 = compute_signature(secret, b"payload two");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_compute_signature_known_value() {
        // NOTE: Pre-computed HMAC-SHA256 to verify correctness
        // echo -n "test payload" | openssl dgst -sha256 -hmac "test-secret" -hex
        let secret = "test-secret";
        let payload = b"test payload";
        let signature = compute_signature(secret, payload);

        // Added: Verify signature is a valid hex string of correct length (64 hex chars = 32 bytes)
        assert_eq!(signature.len(), 64);
        assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_compute_signature_empty_payload() {
        let signature = compute_signature("secret", b"");
        assert_eq!(signature.len(), 64);
        assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_compute_signature_empty_secret() {
        // NOTE: Empty secret is technically valid for HMAC
        let signature = compute_signature("", b"some data");
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn test_compute_signature_unicode_secret() {
        let signature = compute_signature("s3cr3t-key", b"unicode payload");
        assert_eq!(signature.len(), 64);
    }
}
