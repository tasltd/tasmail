// Added: Chat integration model for team chat webhook notifications (TMAIL-129)
// PURPOSE: Stores chat platform webhook configurations for email-to-chat forwarding
// CONSTRAINTS: Must match chat_platform ENUM in PostgreSQL (migration 030)

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Supported chat platforms for webhook integration
/// CONSTRAINTS: Must match the chat_platform ENUM in PostgreSQL (migration 030)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "chat_platform", rename_all = "snake_case")]
pub enum ChatPlatform {
    #[serde(rename = "slack")]
    Slack,
    #[serde(rename = "teams")]
    Teams,
    #[serde(rename = "google_chat")]
    GoogleChat,
    #[serde(rename = "discord")]
    Discord,
    #[serde(rename = "custom")]
    Custom,
}

/// PURPOSE: A user-configured chat integration for email notifications
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatIntegration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub platform: ChatPlatform,
    pub webhook_url: String,
    pub channel_name: Option<String>,
    pub notify_on_receive: bool,
    pub notify_on_send: bool,
    pub notify_on_mention: bool,
    pub filter_from: Option<String>,
    pub filter_subject: Option<String>,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChatIntegrationRequest {
    pub platform: ChatPlatform,
    pub webhook_url: String,
    pub channel_name: Option<String>,
    pub notify_on_receive: Option<bool>,
    pub notify_on_send: Option<bool>,
    pub notify_on_mention: Option<bool>,
    pub filter_from: Option<String>,
    pub filter_subject: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChatIntegrationRequest {
    pub webhook_url: Option<String>,
    pub channel_name: Option<String>,
    pub notify_on_receive: Option<bool>,
    pub notify_on_send: Option<bool>,
    pub notify_on_mention: Option<bool>,
    pub filter_from: Option<String>,
    pub filter_subject: Option<String>,
    pub active: Option<bool>,
}

impl ChatIntegration {
    /// PURPOSE: List all chat integrations belonging to a user
    pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<ChatIntegration>, sqlx::Error> {
        sqlx::query_as::<_, ChatIntegration>(
            "SELECT * FROM chat_integrations WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single chat integration by ID and user
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ChatIntegration>, sqlx::Error> {
        sqlx::query_as::<_, ChatIntegration>(
            "SELECT * FROM chat_integrations WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create a new chat integration for a user
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        input: &CreateChatIntegrationRequest,
    ) -> Result<ChatIntegration, sqlx::Error> {
        sqlx::query_as::<_, ChatIntegration>(
            "INSERT INTO chat_integrations (user_id, platform, webhook_url, channel_name, \
             notify_on_receive, notify_on_send, notify_on_mention, filter_from, filter_subject) \
             VALUES ($1, $2, $3, $4, COALESCE($5, true), COALESCE($6, false), COALESCE($7, true), $8, $9) \
             RETURNING *",
        )
        .bind(user_id)
        .bind(&input.platform)
        .bind(&input.webhook_url)
        .bind(&input.channel_name)
        .bind(input.notify_on_receive)
        .bind(input.notify_on_send)
        .bind(input.notify_on_mention)
        .bind(&input.filter_from)
        .bind(&input.filter_subject)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing chat integration's configuration
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        input: &UpdateChatIntegrationRequest,
    ) -> Result<Option<ChatIntegration>, sqlx::Error> {
        sqlx::query_as::<_, ChatIntegration>(
            "UPDATE chat_integrations SET \
                webhook_url = COALESCE($3, webhook_url), \
                channel_name = COALESCE($4, channel_name), \
                notify_on_receive = COALESCE($5, notify_on_receive), \
                notify_on_send = COALESCE($6, notify_on_send), \
                notify_on_mention = COALESCE($7, notify_on_mention), \
                filter_from = COALESCE($8, filter_from), \
                filter_subject = COALESCE($9, filter_subject), \
                active = COALESCE($10, active), \
                updated_at = NOW() \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(&input.webhook_url)
        .bind(&input.channel_name)
        .bind(input.notify_on_receive)
        .bind(input.notify_on_send)
        .bind(input.notify_on_mention)
        .bind(&input.filter_from)
        .bind(&input.filter_subject)
        .bind(input.active)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete a chat integration
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM chat_integrations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Find active integrations for a user that match notification criteria
    /// NOTE: Used by the chat notifier service to determine which integrations to fire
    pub async fn find_active_for_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<ChatIntegration>, sqlx::Error> {
        sqlx::query_as::<_, ChatIntegration>(
            "SELECT * FROM chat_integrations WHERE user_id = $1 AND active = true",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_platform_serialization() {
        // NOTE: Verify enum values match the PostgreSQL ENUM names
        let platform = ChatPlatform::Slack;
        let json = serde_json::to_value(&platform).unwrap();
        assert_eq!(json, "slack");

        let platform = ChatPlatform::Teams;
        let json = serde_json::to_value(&platform).unwrap();
        assert_eq!(json, "teams");

        let platform = ChatPlatform::GoogleChat;
        let json = serde_json::to_value(&platform).unwrap();
        assert_eq!(json, "google_chat");

        let platform = ChatPlatform::Discord;
        let json = serde_json::to_value(&platform).unwrap();
        assert_eq!(json, "discord");

        let platform = ChatPlatform::Custom;
        let json = serde_json::to_value(&platform).unwrap();
        assert_eq!(json, "custom");
    }

    #[test]
    fn test_chat_platform_deserialization() {
        let platform: ChatPlatform = serde_json::from_str("\"slack\"").unwrap();
        assert_eq!(platform, ChatPlatform::Slack);

        let platform: ChatPlatform = serde_json::from_str("\"teams\"").unwrap();
        assert_eq!(platform, ChatPlatform::Teams);

        let platform: ChatPlatform = serde_json::from_str("\"google_chat\"").unwrap();
        assert_eq!(platform, ChatPlatform::GoogleChat);

        let platform: ChatPlatform = serde_json::from_str("\"discord\"").unwrap();
        assert_eq!(platform, ChatPlatform::Discord);

        let platform: ChatPlatform = serde_json::from_str("\"custom\"").unwrap();
        assert_eq!(platform, ChatPlatform::Custom);
    }

    #[test]
    fn test_chat_platform_roundtrip() {
        let platforms = vec![
            ChatPlatform::Slack,
            ChatPlatform::Teams,
            ChatPlatform::GoogleChat,
            ChatPlatform::Discord,
            ChatPlatform::Custom,
        ];
        let json = serde_json::to_string(&platforms).unwrap();
        let deserialized: Vec<ChatPlatform> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, platforms);
    }

    #[test]
    fn test_chat_platform_invalid_deserialization() {
        let result = serde_json::from_str::<ChatPlatform>("\"unknown_platform\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_chat_integration_serialization() {
        let integration_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let integration = ChatIntegration {
            id: integration_id,
            user_id,
            platform: ChatPlatform::Slack,
            webhook_url: "https://hooks.slack.com/services/T00/B00/xxx".to_string(),
            channel_name: Some("#general".to_string()),
            notify_on_receive: true,
            notify_on_send: false,
            notify_on_mention: true,
            filter_from: None,
            filter_subject: Some("urgent".to_string()),
            active: true,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(&integration).unwrap();
        assert_eq!(json["id"], integration_id.to_string());
        assert_eq!(json["platform"], "slack");
        assert_eq!(json["channel_name"], "#general");
        assert_eq!(json["notify_on_receive"], true);
        assert_eq!(json["notify_on_send"], false);
        assert_eq!(json["notify_on_mention"], true);
        assert!(json["filter_from"].is_null());
        assert_eq!(json["filter_subject"], "urgent");
        assert_eq!(json["active"], true);
    }

    #[test]
    fn test_chat_integration_roundtrip() {
        let integration = ChatIntegration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            platform: ChatPlatform::Discord,
            webhook_url: "https://discord.com/api/webhooks/123/abc".to_string(),
            channel_name: None,
            notify_on_receive: false,
            notify_on_send: true,
            notify_on_mention: false,
            filter_from: Some("boss@example.com".to_string()),
            filter_subject: None,
            active: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&integration).unwrap();
        let deserialized: ChatIntegration = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, integration.id);
        assert_eq!(deserialized.platform, ChatPlatform::Discord);
        assert_eq!(deserialized.active, false);
        assert_eq!(deserialized.notify_on_send, true);
        assert_eq!(deserialized.filter_from.unwrap(), "boss@example.com");
    }

    #[test]
    fn test_create_request_deserialization() {
        let json = serde_json::json!({
            "platform": "slack",
            "webhook_url": "https://hooks.slack.com/services/T00/B00/xxx",
            "channel_name": "#alerts",
            "notify_on_receive": true,
            "notify_on_send": false,
            "notify_on_mention": true,
            "filter_subject": "important"
        });

        let request: CreateChatIntegrationRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.platform, ChatPlatform::Slack);
        assert_eq!(request.webhook_url, "https://hooks.slack.com/services/T00/B00/xxx");
        assert_eq!(request.channel_name.unwrap(), "#alerts");
        assert_eq!(request.notify_on_receive, Some(true));
        assert_eq!(request.filter_subject.unwrap(), "important");
    }

    #[test]
    fn test_create_request_minimal() {
        let json = serde_json::json!({
            "platform": "teams",
            "webhook_url": "https://outlook.office.com/webhook/xxx"
        });

        let request: CreateChatIntegrationRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.platform, ChatPlatform::Teams);
        assert!(request.channel_name.is_none());
        assert!(request.notify_on_receive.is_none());
        assert!(request.filter_from.is_none());
    }

    #[test]
    fn test_create_request_missing_required_field_fails() {
        let json = serde_json::json!({
            "platform": "slack"
        });
        let result = serde_json::from_value::<CreateChatIntegrationRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_request_partial() {
        let json = serde_json::json!({
            "active": false
        });

        let update: UpdateChatIntegrationRequest = serde_json::from_value(json).unwrap();
        assert!(update.webhook_url.is_none());
        assert_eq!(update.active, Some(false));
        assert!(update.notify_on_receive.is_none());
    }

    #[test]
    fn test_update_request_empty() {
        let json = serde_json::json!({});

        let update: UpdateChatIntegrationRequest = serde_json::from_value(json).unwrap();
        assert!(update.webhook_url.is_none());
        assert!(update.active.is_none());
        assert!(update.channel_name.is_none());
    }
}
