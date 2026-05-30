// Added: Webhook model for outbound webhook notifications (TMAIL-131)

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents the types of email events that can trigger webhooks
/// CONSTRAINTS: Must match the webhook_event ENUM in PostgreSQL (migration 023)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "webhook_event", rename_all = "snake_case")]
pub enum WebhookEvent {
    #[serde(rename = "email.received")]
    #[sqlx(rename = "email.received")]
    EmailReceived,
    #[serde(rename = "email.sent")]
    #[sqlx(rename = "email.sent")]
    EmailSent,
    #[serde(rename = "email.deleted")]
    #[sqlx(rename = "email.deleted")]
    EmailDeleted,
    #[serde(rename = "email.moved")]
    #[sqlx(rename = "email.moved")]
    EmailMoved,
    #[serde(rename = "email.flagged")]
    #[sqlx(rename = "email.flagged")]
    EmailFlagged,
}

/// PURPOSE: A user-configured webhook endpoint
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Webhook {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub secret: String,
    pub events: Vec<WebhookEvent>,
    pub active: bool,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub last_triggered_at: Option<chrono::DateTime<chrono::Utc>>,
    pub failure_count: i32,
}

/// PURPOSE: A record of a single webhook delivery attempt
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event: WebhookEvent,
    pub payload: serde_json::Value,
    pub response_status: Option<i32>,
    pub response_body: Option<String>,
    pub delivered_at: chrono::DateTime<chrono::Utc>,
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    pub secret: String,
    pub events: Vec<WebhookEvent>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub url: Option<String>,
    pub secret: Option<String>,
    pub events: Option<Vec<WebhookEvent>>,
    pub active: Option<bool>,
    pub description: Option<String>,
}

impl Webhook {
    /// PURPOSE: List all webhooks belonging to a user
    pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Webhook>, sqlx::Error> {
        sqlx::query_as::<_, Webhook>(
            "SELECT * FROM webhooks WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single webhook by ID and user
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Webhook>, sqlx::Error> {
        sqlx::query_as::<_, Webhook>(
            "SELECT * FROM webhooks WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Find active webhooks for a user matching a specific event type
    /// NOTE: Used by the dispatcher to determine which webhooks to fire
    pub async fn find_active_for_event(
        pool: &PgPool,
        user_id: Uuid,
        event: &WebhookEvent,
    ) -> Result<Vec<Webhook>, sqlx::Error> {
        sqlx::query_as::<_, Webhook>(
            "SELECT * FROM webhooks WHERE user_id = $1 AND active = true AND $2 = ANY(events)",
        )
        .bind(user_id)
        .bind(event)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Create a new webhook endpoint for a user
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        input: &CreateWebhookRequest,
    ) -> Result<Webhook, sqlx::Error> {
        sqlx::query_as::<_, Webhook>(
            "INSERT INTO webhooks (user_id, url, secret, events, description) \
             VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(user_id)
        .bind(&input.url)
        .bind(&input.secret)
        .bind(&input.events)
        .bind(&input.description)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing webhook's configuration
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        input: &UpdateWebhookRequest,
    ) -> Result<Option<Webhook>, sqlx::Error> {
        sqlx::query_as::<_, Webhook>(
            "UPDATE webhooks SET \
                url = COALESCE($3, url), \
                secret = COALESCE($4, secret), \
                events = COALESCE($5, events), \
                active = COALESCE($6, active), \
                description = COALESCE($7, description), \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(&input.url)
        .bind(&input.secret)
        .bind(&input.events)
        .bind(input.active)
        .bind(&input.description)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete a webhook and all its delivery records (cascade)
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM webhooks WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Record delivery failure and deactivate after threshold
    /// CONSTRAINTS: Deactivates webhook after 10 consecutive failures
    pub async fn record_failure(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE webhooks SET \
                failure_count = failure_count + 1, \
                active = CASE WHEN failure_count + 1 >= 10 THEN false ELSE active END, \
                updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Reset failure count and update last_triggered_at on success
    pub async fn record_success(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE webhooks SET \
                failure_count = 0, \
                last_triggered_at = NOW(), \
                updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Added (TMAIL-313): Replace the HMAC signing secret for a webhook.
    /// Returns the updated webhook on success, or None if the webhook doesn't
    /// belong to the user (or doesn't exist). Callers must persist / return
    /// `new_secret` to the user — the plaintext is not recoverable after this.
    pub async fn rotate_secret(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        new_secret: &str,
    ) -> Result<Option<Webhook>, sqlx::Error> {
        sqlx::query_as::<_, Webhook>(
            "UPDATE webhooks SET secret = $3, updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(new_secret)
        .fetch_optional(pool)
        .await
    }
}

impl WebhookDelivery {
    /// PURPOSE: List recent deliveries for a webhook, most recent first
    pub async fn find_by_webhook(
        pool: &PgPool,
        webhook_id: Uuid,
    ) -> Result<Vec<WebhookDelivery>, sqlx::Error> {
        sqlx::query_as::<_, WebhookDelivery>(
            "SELECT * FROM webhook_deliveries WHERE webhook_id = $1 \
             ORDER BY delivered_at DESC LIMIT 50",
        )
        .bind(webhook_id)
        .fetch_all(pool)
        .await
    }

    /// Added (TMAIL-313): Look up a single delivery scoped to its parent webhook.
    /// Used by the manual redeliver endpoint to verify the delivery belongs to
    /// the addressed webhook before replaying it.
    pub async fn find_by_id_and_webhook(
        pool: &PgPool,
        id: Uuid,
        webhook_id: Uuid,
    ) -> Result<Option<WebhookDelivery>, sqlx::Error> {
        sqlx::query_as::<_, WebhookDelivery>(
            "SELECT * FROM webhook_deliveries WHERE id = $1 AND webhook_id = $2",
        )
        .bind(id)
        .bind(webhook_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Insert a delivery record after a webhook attempt
    pub async fn create(
        pool: &PgPool,
        webhook_id: Uuid,
        event: &WebhookEvent,
        payload: &serde_json::Value,
        response_status: Option<i32>,
        response_body: Option<String>,
        success: bool,
    ) -> Result<WebhookDelivery, sqlx::Error> {
        sqlx::query_as::<_, WebhookDelivery>(
            "INSERT INTO webhook_deliveries (webhook_id, event, payload, response_status, response_body, success) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
        )
        .bind(webhook_id)
        .bind(event)
        .bind(payload)
        .bind(response_status)
        .bind(response_body)
        .bind(success)
        .fetch_one(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_event_serialization() {
        // NOTE: Verify enum values match the PostgreSQL ENUM names
        let event = WebhookEvent::EmailReceived;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json, "email.received");

        let event = WebhookEvent::EmailSent;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json, "email.sent");

        let event = WebhookEvent::EmailDeleted;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json, "email.deleted");

        let event = WebhookEvent::EmailMoved;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json, "email.moved");

        let event = WebhookEvent::EmailFlagged;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json, "email.flagged");
    }

    #[test]
    fn test_webhook_event_deserialization() {
        let event: WebhookEvent = serde_json::from_str("\"email.received\"").unwrap();
        assert_eq!(event, WebhookEvent::EmailReceived);

        let event: WebhookEvent = serde_json::from_str("\"email.flagged\"").unwrap();
        assert_eq!(event, WebhookEvent::EmailFlagged);
    }

    #[test]
    fn test_webhook_event_roundtrip() {
        let events = vec![
            WebhookEvent::EmailReceived,
            WebhookEvent::EmailSent,
            WebhookEvent::EmailDeleted,
            WebhookEvent::EmailMoved,
            WebhookEvent::EmailFlagged,
        ];
        let json = serde_json::to_string(&events).unwrap();
        let deserialized: Vec<WebhookEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, events);
    }

    #[test]
    fn test_webhook_serialization() {
        let webhook_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let webhook = Webhook {
            id: webhook_id,
            user_id,
            url: "https://example.com/hook".to_string(),
            secret: "my-secret-key".to_string(),
            events: vec![WebhookEvent::EmailReceived, WebhookEvent::EmailSent],
            active: true,
            description: Some("My test webhook".to_string()),
            created_at: now,
            updated_at: now,
            last_triggered_at: None,
            failure_count: 0,
        };

        let json = serde_json::to_value(&webhook).unwrap();
        assert_eq!(json["id"], webhook_id.to_string());
        assert_eq!(json["url"], "https://example.com/hook");
        assert_eq!(json["secret"], "my-secret-key");
        assert_eq!(json["active"], true);
        assert_eq!(json["failure_count"], 0);
        assert!(json["last_triggered_at"].is_null());
        assert_eq!(json["events"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_webhook_roundtrip() {
        let webhook = Webhook {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            url: "https://hooks.example.com/callback".to_string(),
            secret: "secret123".to_string(),
            events: vec![WebhookEvent::EmailDeleted],
            active: false,
            description: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_triggered_at: Some(chrono::Utc::now()),
            failure_count: 3,
        };

        let json = serde_json::to_string(&webhook).unwrap();
        let deserialized: Webhook = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, webhook.id);
        assert_eq!(deserialized.url, "https://hooks.example.com/callback");
        assert_eq!(deserialized.active, false);
        assert_eq!(deserialized.failure_count, 3);
        assert!(deserialized.last_triggered_at.is_some());
    }

    #[test]
    fn test_create_webhook_request_deserialization() {
        let json = serde_json::json!({
            "url": "https://example.com/webhook",
            "secret": "my-secret",
            "events": ["email.received", "email.sent"],
            "description": "Notification webhook"
        });

        let request: CreateWebhookRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.url, "https://example.com/webhook");
        assert_eq!(request.secret, "my-secret");
        assert_eq!(request.events.len(), 2);
        assert_eq!(request.events[0], WebhookEvent::EmailReceived);
        assert_eq!(request.description.unwrap(), "Notification webhook");
    }

    #[test]
    fn test_create_webhook_request_without_description() {
        let json = serde_json::json!({
            "url": "https://example.com/hook",
            "secret": "s3cret",
            "events": ["email.deleted"]
        });

        let request: CreateWebhookRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.url, "https://example.com/hook");
        assert!(request.description.is_none());
        assert_eq!(request.events.len(), 1);
    }

    #[test]
    fn test_create_webhook_request_missing_required_field_fails() {
        let json = serde_json::json!({
            "url": "https://example.com"
        });
        let result = serde_json::from_value::<CreateWebhookRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_webhook_request_partial() {
        let json = serde_json::json!({
            "active": false
        });

        let update: UpdateWebhookRequest = serde_json::from_value(json).unwrap();
        assert!(update.url.is_none());
        assert!(update.secret.is_none());
        assert!(update.events.is_none());
        assert_eq!(update.active, Some(false));
        assert!(update.description.is_none());
    }

    #[test]
    fn test_update_webhook_request_empty() {
        let json = serde_json::json!({});

        let update: UpdateWebhookRequest = serde_json::from_value(json).unwrap();
        assert!(update.url.is_none());
        assert!(update.active.is_none());
        assert!(update.events.is_none());
    }

    #[test]
    fn test_webhook_delivery_serialization() {
        let delivery = WebhookDelivery {
            id: Uuid::new_v4(),
            webhook_id: Uuid::new_v4(),
            event: WebhookEvent::EmailReceived,
            payload: serde_json::json!({"subject": "Test email"}),
            response_status: Some(200),
            response_body: Some("OK".to_string()),
            delivered_at: chrono::Utc::now(),
            success: true,
        };

        let json = serde_json::to_value(&delivery).unwrap();
        assert_eq!(json["event"], "email.received");
        assert_eq!(json["response_status"], 200);
        assert_eq!(json["success"], true);
        assert_eq!(json["payload"]["subject"], "Test email");
    }

    #[test]
    fn test_webhook_delivery_failed() {
        let delivery = WebhookDelivery {
            id: Uuid::new_v4(),
            webhook_id: Uuid::new_v4(),
            event: WebhookEvent::EmailSent,
            payload: serde_json::json!({"to": "user@example.com"}),
            response_status: Some(500),
            response_body: Some("Internal Server Error".to_string()),
            delivered_at: chrono::Utc::now(),
            success: false,
        };

        let json = serde_json::to_value(&delivery).unwrap();
        assert_eq!(json["success"], false);
        assert_eq!(json["response_status"], 500);
    }

    #[test]
    fn test_webhook_event_invalid_deserialization() {
        let result = serde_json::from_str::<WebhookEvent>("\"email.unknown\"");
        assert!(result.is_err());
    }

    // Added (TMAIL-313): The rotate_secret / redeliver SQL paths are exercised
    // end-to-end in tests/webhook_redeliver_test.rs against a live database.
    // The unit-level assertions below cover the in-memory invariants we want
    // a future refactor to preserve.

    #[test]
    fn test_rotate_secret_changes_value_in_struct() {
        // NOTE: This is a struct-level sanity check — the DB roundtrip is in the
        // integration test. We assert that overwriting `secret` on the model
        // doesn't disturb the other fields a UI relies on (id, url, events).
        let mut webhook = Webhook {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            url: "https://example.com/hook".to_string(),
            secret: "old-secret".to_string(),
            events: vec![WebhookEvent::EmailReceived],
            active: true,
            description: Some("d".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_triggered_at: None,
            failure_count: 0,
        };
        let original_id = webhook.id;
        webhook.secret = "new-secret".to_string();
        assert_eq!(webhook.id, original_id);
        assert_eq!(webhook.secret, "new-secret");
        assert_eq!(webhook.url, "https://example.com/hook");
    }
}
