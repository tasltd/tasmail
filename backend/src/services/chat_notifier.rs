// Added: Chat notifier service for team chat webhook integrations (TMAIL-129)
// PURPOSE: Formats and sends notifications to Slack, Teams, Google Chat, Discord, and custom webhooks
// EXTERNAL: Uses reqwest for HTTP POST calls to platform webhook URLs

use crate::models::chat_integration::ChatPlatform;

/// PURPOSE: Format a Slack Block Kit message payload
/// CONSTRAINTS: Slack webhooks expect a JSON body with "blocks" or "text" field
/// EXTERNAL: https://api.slack.com/messaging/webhooks
pub fn format_slack_payload(subject: &str, from: &str, snippet: &str) -> serde_json::Value {
    serde_json::json!({
        "blocks": [
            {
                "type": "header",
                "text": {
                    "type": "plain_text",
                    "text": "📧 New Email Notification"
                }
            },
            {
                "type": "section",
                "fields": [
                    {
                        "type": "mrkdwn",
                        "text": format!("*From:*\n{}", from)
                    },
                    {
                        "type": "mrkdwn",
                        "text": format!("*Subject:*\n{}", subject)
                    }
                ]
            },
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*Preview:*\n{}", snippet)
                }
            }
        ]
    })
}

/// PURPOSE: Format a Microsoft Teams Adaptive Card payload
/// CONSTRAINTS: Teams webhooks expect Adaptive Card JSON format
/// EXTERNAL: https://learn.microsoft.com/en-us/microsoftteams/platform/webhooks-and-connectors/how-to/connectors-using
pub fn format_teams_payload(subject: &str, from: &str, snippet: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "attachments": [
            {
                "contentType": "application/vnd.microsoft.card.adaptive",
                "content": {
                    "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
                    "type": "AdaptiveCard",
                    "version": "1.4",
                    "body": [
                        {
                            "type": "TextBlock",
                            "text": "📧 New Email Notification",
                            "weight": "Bolder",
                            "size": "Medium"
                        },
                        {
                            "type": "FactSet",
                            "facts": [
                                { "title": "From:", "value": from },
                                { "title": "Subject:", "value": subject }
                            ]
                        },
                        {
                            "type": "TextBlock",
                            "text": snippet,
                            "wrap": true,
                            "maxLines": 3
                        }
                    ]
                }
            }
        ]
    })
}

/// PURPOSE: Format a Google Chat card message payload
/// CONSTRAINTS: Google Chat webhooks expect a "cards" JSON structure
/// EXTERNAL: https://developers.google.com/workspace/chat/api/reference/rest/v1/spaces.messages
pub fn format_google_chat_payload(subject: &str, from: &str, snippet: &str) -> serde_json::Value {
    serde_json::json!({
        "cards": [
            {
                "header": {
                    "title": "📧 New Email Notification",
                    "subtitle": subject
                },
                "sections": [
                    {
                        "widgets": [
                            {
                                "keyValue": {
                                    "topLabel": "From",
                                    "content": from
                                }
                            },
                            {
                                "keyValue": {
                                    "topLabel": "Subject",
                                    "content": subject
                                }
                            },
                            {
                                "textParagraph": {
                                    "text": snippet
                                }
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

/// PURPOSE: Format a Discord embed message payload
/// CONSTRAINTS: Discord webhooks accept embeds array with rich formatting
/// EXTERNAL: https://discord.com/developers/docs/resources/webhook#execute-webhook
pub fn format_discord_payload(subject: &str, from: &str, snippet: &str) -> serde_json::Value {
    serde_json::json!({
        "embeds": [
            {
                "title": "📧 New Email Notification",
                "color": 3447003,
                "fields": [
                    { "name": "From", "value": from, "inline": true },
                    { "name": "Subject", "value": subject, "inline": true }
                ],
                "description": snippet,
                "footer": {
                    "text": "TASMail"
                }
            }
        ]
    })
}

/// PURPOSE: Format a generic JSON notification payload for custom webhooks
/// CONSTRAINTS: Simple flat JSON structure for maximum compatibility
pub fn format_custom_payload(subject: &str, from: &str, snippet: &str) -> serde_json::Value {
    serde_json::json!({
        "event": "email.notification",
        "from": from,
        "subject": subject,
        "snippet": snippet,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// PURPOSE: Format notification payload based on the target platform
pub fn format_notification(
    platform: &ChatPlatform,
    subject: &str,
    from: &str,
    snippet: &str,
) -> serde_json::Value {
    match platform {
        ChatPlatform::Slack => format_slack_payload(subject, from, snippet),
        ChatPlatform::Teams => format_teams_payload(subject, from, snippet),
        ChatPlatform::GoogleChat => format_google_chat_payload(subject, from, snippet),
        ChatPlatform::Discord => format_discord_payload(subject, from, snippet),
        ChatPlatform::Custom => format_custom_payload(subject, from, snippet),
    }
}

/// PURPOSE: Send a notification to a chat platform webhook URL
/// CONSTRAINTS: Uses 10s timeout to avoid blocking on slow endpoints
/// EXTERNAL: Makes HTTP POST request to the webhook URL
pub async fn send_notification(
    platform: &ChatPlatform,
    webhook_url: &str,
    subject: &str,
    from: &str,
    snippet: &str,
) -> Result<(), String> {
    let payload = format_notification(platform, subject, from, snippet);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {}", err))?;

    let response = client
        .post(webhook_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|err| format!("Failed to send notification to {}: {}", webhook_url, err))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Webhook returned status {}: {}",
            status,
            body.chars().take(200).collect::<String>()
        ))
    }
}

/// PURPOSE: Send a test notification to verify webhook configuration
/// NOTE: Used by the POST /api/chat-integrations/:id/test endpoint
pub async fn send_test_notification(
    platform: &ChatPlatform,
    webhook_url: &str,
) -> Result<(), String> {
    send_notification(
        platform,
        webhook_url,
        "Test Notification from TASMail",
        "tasmail@example.com",
        "This is a test message to verify your chat integration is working correctly.",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_slack_payload_structure() {
        let payload = format_slack_payload("Hello", "alice@example.com", "Preview text");
        // NOTE: Slack payloads must have a "blocks" array
        assert!(payload["blocks"].is_array());
        let blocks = payload["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 3);
        // Added: Verify header block
        assert_eq!(blocks[0]["type"], "header");
        // Added: Verify section with from and subject fields
        assert_eq!(blocks[1]["type"], "section");
        let fields = blocks[1]["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 2);
        assert!(fields[0]["text"].as_str().unwrap().contains("alice@example.com"));
        assert!(fields[1]["text"].as_str().unwrap().contains("Hello"));
        // Added: Verify snippet section
        assert!(blocks[2]["text"]["text"].as_str().unwrap().contains("Preview text"));
    }

    #[test]
    fn test_format_teams_payload_structure() {
        let payload = format_teams_payload("Meeting", "bob@example.com", "Let's discuss");
        // NOTE: Teams payloads must have "type" and "attachments"
        assert_eq!(payload["type"], "message");
        let attachments = payload["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 1);
        let card_content = &attachments[0]["content"];
        assert_eq!(card_content["type"], "AdaptiveCard");
        // Added: Verify facts contain from and subject
        let body = card_content["body"].as_array().unwrap();
        let facts = body[1]["facts"].as_array().unwrap();
        assert_eq!(facts[0]["value"], "bob@example.com");
        assert_eq!(facts[1]["value"], "Meeting");
    }

    #[test]
    fn test_format_google_chat_payload_structure() {
        let payload = format_google_chat_payload("Update", "carol@example.com", "Status update");
        // NOTE: Google Chat payloads must have a "cards" array
        assert!(payload["cards"].is_array());
        let cards = payload["cards"].as_array().unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["header"]["subtitle"], "Update");
        let widgets = cards[0]["sections"][0]["widgets"].as_array().unwrap();
        assert!(widgets.len() >= 3);
        assert_eq!(widgets[0]["keyValue"]["content"], "carol@example.com");
    }

    #[test]
    fn test_format_discord_payload_structure() {
        let payload = format_discord_payload("Alert", "dave@example.com", "Something happened");
        // NOTE: Discord payloads must have an "embeds" array
        assert!(payload["embeds"].is_array());
        let embeds = payload["embeds"].as_array().unwrap();
        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0]["color"], 3447003);
        let fields = embeds[0]["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0]["value"], "dave@example.com");
        assert_eq!(fields[1]["value"], "Alert");
        assert_eq!(embeds[0]["description"], "Something happened");
        assert_eq!(embeds[0]["footer"]["text"], "TASMail");
    }

    #[test]
    fn test_format_custom_payload_structure() {
        let payload = format_custom_payload("Custom Event", "eve@example.com", "Custom snippet");
        assert_eq!(payload["event"], "email.notification");
        assert_eq!(payload["from"], "eve@example.com");
        assert_eq!(payload["subject"], "Custom Event");
        assert_eq!(payload["snippet"], "Custom snippet");
        // Added: Verify timestamp is present
        assert!(payload["timestamp"].is_string());
    }

    #[test]
    fn test_format_notification_dispatches_to_correct_platform() {
        // Added: Verify each platform gets the right format
        let slack = format_notification(&ChatPlatform::Slack, "S", "F", "P");
        assert!(slack.get("blocks").is_some());

        let teams = format_notification(&ChatPlatform::Teams, "S", "F", "P");
        assert_eq!(teams["type"], "message");

        let gchat = format_notification(&ChatPlatform::GoogleChat, "S", "F", "P");
        assert!(gchat.get("cards").is_some());

        let discord = format_notification(&ChatPlatform::Discord, "S", "F", "P");
        assert!(discord.get("embeds").is_some());

        let custom = format_notification(&ChatPlatform::Custom, "S", "F", "P");
        assert_eq!(custom["event"], "email.notification");
    }

    #[test]
    fn test_format_slack_payload_with_special_characters() {
        // Added: Verify special characters are preserved in payloads
        let payload = format_slack_payload(
            "Re: <script>alert('xss')</script>",
            "user@example.com",
            "Hello & goodbye <world>",
        );
        let blocks = payload["blocks"].as_array().unwrap();
        assert!(blocks[1]["fields"][1]["text"]
            .as_str()
            .unwrap()
            .contains("<script>"));
    }

    #[test]
    fn test_format_teams_payload_empty_strings() {
        let payload = format_teams_payload("", "", "");
        let card_content = &payload["attachments"][0]["content"];
        let facts = card_content["body"][1]["facts"].as_array().unwrap();
        assert_eq!(facts[0]["value"], "");
        assert_eq!(facts[1]["value"], "");
    }

    #[test]
    fn test_format_discord_payload_long_snippet() {
        // Added: Verify long text is passed through without truncation in the payload
        let long_snippet = "a".repeat(2000);
        let payload = format_discord_payload("Subject", "from@example.com", &long_snippet);
        assert_eq!(payload["embeds"][0]["description"].as_str().unwrap().len(), 2000);
    }
}
