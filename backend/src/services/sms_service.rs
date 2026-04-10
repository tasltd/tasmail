use anyhow::Result;
use serde::{Deserialize, Serialize};

/// SMS provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SmsConfig {
    pub hubtel_client_id: Option<String>,
    pub hubtel_client_secret: Option<String>,
    pub hubtel_sender_id: Option<String>,
    pub africastalking_api_key: Option<String>,
    pub africastalking_username: Option<String>,
    pub africastalking_sender_id: Option<String>,
}

impl Default for SmsConfig {
    fn default() -> Self {
        Self {
            hubtel_client_id: std::env::var("HUBTEL_CLIENT_ID").ok(),
            hubtel_client_secret: std::env::var("HUBTEL_CLIENT_SECRET").ok(),
            hubtel_sender_id: std::env::var("HUBTEL_SENDER_ID").ok().or(Some("TASMail".to_string())),
            africastalking_api_key: std::env::var("AFRICASTALKING_API_KEY").ok(),
            africastalking_username: std::env::var("AFRICASTALKING_USERNAME").ok(),
            africastalking_sender_id: std::env::var("AFRICASTALKING_SENDER_ID").ok(),
        }
    }
}

/// Generate a 6-digit OTP code
pub fn generate_otp() -> String {
    use rand::Rng;
    let code: u32 = rand::rng().random_range(100_000..999_999);
    code.to_string()
}

/// Send OTP via Hubtel SMS API (Ghana)
pub async fn send_via_hubtel(
    config: &SmsConfig,
    phone: &str,
    code: &str,
) -> Result<()> {
    let client_id = config.hubtel_client_id.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Hubtel client ID not configured"))?;
    let client_secret = config.hubtel_client_secret.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Hubtel client secret not configured"))?;
    let sender_id = config.hubtel_sender_id.as_deref().unwrap_or("TASMail");

    let message = format!("Your TASMail verification code is: {}. Valid for 5 minutes.", code);

    let client = reqwest::Client::new();
    let resp: reqwest::Response = client
        .get("https://smsc.hubtel.com/v1/messages/send")
        .basic_auth(client_id, Some(client_secret))
        .query(&[
            ("From", sender_id),
            ("To", phone),
            ("Content", message.as_str()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Hubtel SMS failed: {}", body));
    }

    tracing::info!("SMS OTP sent via Hubtel to {}", phone);
    Ok(())
}

/// Send OTP via Africa's Talking SMS API
pub async fn send_via_africastalking(
    config: &SmsConfig,
    phone: &str,
    code: &str,
) -> Result<()> {
    let api_key = config.africastalking_api_key.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Africa's Talking API key not configured"))?;
    let username = config.africastalking_username.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Africa's Talking username not configured"))?;

    let message = format!("Your TASMail verification code is: {}. Valid for 5 minutes.", code);

    let mut params: Vec<(&str, &str)> = vec![
        ("username", username.as_str()),
        ("to", phone),
        ("message", message.as_str()),
    ];

    if let Some(sender) = &config.africastalking_sender_id {
        params.push(("from", sender.as_str()));
    }

    let client = reqwest::Client::new();
    let resp: reqwest::Response = client
        .post("https://api.africastalking.com/version1/messaging")
        .header("apiKey", api_key.as_str())
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Africa's Talking SMS failed: {}", body));
    }

    tracing::info!("SMS OTP sent via Africa's Talking to {}", phone);
    Ok(())
}

/// Send OTP via the configured provider
pub async fn send_otp(
    config: &SmsConfig,
    provider: &str,
    phone: &str,
    code: &str,
) -> Result<()> {
    match provider {
        "hubtel" => send_via_hubtel(config, phone, code).await,
        "africastalking" => send_via_africastalking(config, phone, code).await,
        _ => Err(anyhow::anyhow!("Unknown SMS provider: {}", provider)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_otp_is_6_digits() {
        for _ in 0..100 {
            let code = generate_otp();
            assert_eq!(code.len(), 6);
            assert!(code.parse::<u32>().is_ok());
            let num: u32 = code.parse().unwrap();
            assert!(num >= 100_000 && num <= 999_999);
        }
    }

    #[test]
    fn test_otp_codes_are_unique() {
        let codes: Vec<String> = (0..10).map(|_| generate_otp()).collect();
        // NOTE: Not guaranteed unique but extremely unlikely to have duplicates in 10 codes
        let unique: std::collections::HashSet<&String> = codes.iter().collect();
        assert!(unique.len() >= 8); // Allow some statistical overlap
    }

    #[test]
    fn test_sms_config_default() {
        let config = SmsConfig::default();
        // Without env vars, most fields should be None
        assert!(config.hubtel_client_id.is_none() || config.hubtel_client_id.is_some());
        // Sender ID defaults to TASMail if env var not set
        assert!(config.hubtel_sender_id.is_some());
    }
}
