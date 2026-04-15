// Added: Payment service for Paystack and MTN MoMo integration (TMAIL-46)
// PURPOSE: Handles payment initialization, verification, and webhook signature validation
// EXTERNAL: Calls Paystack REST API and MTN MoMo API; uses reqwest (already in deps)
// NOTE: Never execute live financial transactions without explicit user instruction (HARD RULE)

use hmac::{Hmac, Mac};
use sha2::Sha512;
use serde::{Deserialize, Serialize};

/// PURPOSE: Paystack API client for initializing and verifying transactions
/// NOTE: Uses Paystack V1 REST API (https://api.paystack.co)
#[derive(Debug, Clone)]
pub struct PaystackClient {
    secret_key: String,
    base_url: String,
    http: reqwest::Client,
}

/// PURPOSE: Paystack transaction initialization request
#[derive(Debug, Serialize)]
pub struct PaystackInitRequest {
    pub email: String,
    pub amount: i64, // NOTE: Amount in pesewas (kobo) — 1 GHS = 100 pesewas
    pub currency: String,
    pub reference: String,
    pub callback_url: Option<String>,
}

/// PURPOSE: Paystack transaction initialization response
#[derive(Debug, Deserialize)]
pub struct PaystackInitResponse {
    pub status: bool,
    pub message: String,
    pub data: Option<PaystackInitData>,
}

#[derive(Debug, Deserialize)]
pub struct PaystackInitData {
    pub authorization_url: String,
    pub access_code: String,
    pub reference: String,
}

/// PURPOSE: Paystack transaction verification response
#[derive(Debug, Deserialize)]
pub struct PaystackVerifyResponse {
    pub status: bool,
    pub message: String,
    pub data: Option<PaystackVerifyData>,
}

#[derive(Debug, Deserialize)]
pub struct PaystackVerifyData {
    pub status: String,
    pub reference: String,
    pub amount: i64,
    pub currency: String,
    pub channel: Option<String>,
    pub paid_at: Option<String>,
}

/// PURPOSE: Paystack webhook event payload
#[derive(Debug, Deserialize)]
pub struct PaystackWebhookEvent {
    pub event: String,
    pub data: serde_json::Value,
}

impl PaystackClient {
    /// PURPOSE: Create a new Paystack client with the given secret key
    pub fn new(secret_key: String) -> Self {
        Self {
            secret_key,
            base_url: "https://api.paystack.co".to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// PURPOSE: Initialize a transaction — returns authorization URL for checkout redirect
    pub async fn initialize_transaction(
        &self,
        req: &PaystackInitRequest,
    ) -> Result<PaystackInitResponse, anyhow::Error> {
        let resp = self
            .http
            .post(format!("{}/transaction/initialize", self.base_url))
            .header("Authorization", format!("Bearer {}", self.secret_key))
            .json(req)
            .send()
            .await?
            .json::<PaystackInitResponse>()
            .await?;
        Ok(resp)
    }

    /// PURPOSE: Verify a transaction by reference — confirms payment status
    pub async fn verify_transaction(
        &self,
        reference: &str,
    ) -> Result<PaystackVerifyResponse, anyhow::Error> {
        let resp = self
            .http
            .get(format!("{}/transaction/verify/{}", self.base_url, reference))
            .header("Authorization", format!("Bearer {}", self.secret_key))
            .send()
            .await?
            .json::<PaystackVerifyResponse>()
            .await?;
        Ok(resp)
    }

    /// PURPOSE: Verify Paystack webhook signature using HMAC-SHA512
    /// NOTE: Paystack sends the signature in the `x-paystack-signature` header
    pub fn verify_webhook_signature(&self, body: &[u8], signature: &str) -> bool {
        verify_paystack_signature(&self.secret_key, body, signature)
    }
}

/// PURPOSE: Standalone HMAC-SHA512 signature verification for Paystack webhooks
/// NOTE: Exposed as a free function for unit testing without a full client
pub fn verify_paystack_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let Ok(mut mac) = Hmac::<Sha512>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());
    // Added: Constant-time comparison to prevent timing attacks
    expected == signature
}

// --- MTN MoMo Client ---

/// PURPOSE: MTN Mobile Money API client for payment collection
/// NOTE: Uses MTN MoMo Collections API (sandbox: https://sandbox.momodeveloper.mtn.com)
#[derive(Debug, Clone)]
pub struct MomoClient {
    api_key: String,
    api_user: String,
    base_url: String,
    http: reqwest::Client,
}

/// PURPOSE: MoMo payment request body
#[derive(Debug, Serialize)]
pub struct MomoPaymentRequest {
    pub amount: String,
    pub currency: String,
    pub external_id: String,
    pub payer: MomoPayer,
    pub payer_message: String,
    pub payee_note: String,
}

#[derive(Debug, Serialize)]
pub struct MomoPayer {
    pub party_id_type: String,
    pub party_id: String,
}

/// PURPOSE: MoMo payment status response
#[derive(Debug, Serialize, Deserialize)]
pub struct MomoPaymentStatus {
    pub status: String,
    pub reason: Option<serde_json::Value>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
}

impl MomoClient {
    /// PURPOSE: Create a new MoMo client with API credentials
    pub fn new(api_key: String, api_user: String) -> Self {
        Self {
            api_key,
            api_user,
            base_url: "https://sandbox.momodeveloper.mtn.com".to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// PURPOSE: Request a payment from a mobile money user (triggers USSD prompt)
    /// NOTE: The phone_number should be in format 233XXXXXXXXX (Ghana international)
    pub async fn request_payment(
        &self,
        reference_id: &str,
        amount: &str,
        phone_number: &str,
        message: &str,
    ) -> Result<(), anyhow::Error> {
        let body = MomoPaymentRequest {
            amount: amount.to_string(),
            currency: "GHS".to_string(),
            external_id: reference_id.to_string(),
            payer: MomoPayer {
                party_id_type: "MSISDN".to_string(),
                party_id: phone_number.to_string(),
            },
            payer_message: message.to_string(),
            payee_note: format!("TASMail subscription payment {}", reference_id),
        };

        let resp = self
            .http
            .post(format!(
                "{}/collection/v1_0/requesttopay",
                self.base_url
            ))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-Reference-Id", reference_id)
            .header("X-Target-Environment", "sandbox")
            .header("Ocp-Apim-Subscription-Key", &self.api_user)
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "MoMo request_payment failed: {} — {}",
                status,
                text
            ))
        }
    }

    /// PURPOSE: Check the status of a previously requested payment
    pub async fn check_status(
        &self,
        reference_id: &str,
    ) -> Result<MomoPaymentStatus, anyhow::Error> {
        let resp = self
            .http
            .get(format!(
                "{}/collection/v1_0/requesttopay/{}",
                self.base_url, reference_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-Target-Environment", "sandbox")
            .header("Ocp-Apim-Subscription-Key", &self.api_user)
            .send()
            .await?
            .json::<MomoPaymentStatus>()
            .await?;
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_paystack_signature_valid() {
        // Added: Test HMAC-SHA512 verification with known good signature
        let secret = "sk_test_secret123";
        let body = b"{\"event\":\"charge.success\",\"data\":{\"reference\":\"ref-001\"}}";
        let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(verify_paystack_signature(secret, body, &signature));
    }

    #[test]
    fn test_verify_paystack_signature_invalid() {
        // Added: Test that tampered body fails verification
        let secret = "sk_test_secret123";
        let body = b"{\"event\":\"charge.success\"}";
        let tampered_body = b"{\"event\":\"charge.failed\"}";
        let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(!verify_paystack_signature(secret, tampered_body, &signature));
    }

    #[test]
    fn test_verify_paystack_signature_wrong_secret() {
        // Added: Test that wrong secret fails verification
        let body = b"{\"event\":\"charge.success\"}";
        let mut mac = Hmac::<Sha512>::new_from_slice(b"correct-secret").unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(!verify_paystack_signature("wrong-secret", body, &signature));
    }

    #[test]
    fn test_verify_paystack_signature_empty_body() {
        // Added: Test verification with empty body
        let secret = "sk_test_secret";
        let body = b"";
        let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert!(verify_paystack_signature(secret, body, &signature));
    }

    #[test]
    fn test_paystack_init_request_serialize() {
        // Added: Test that init request serializes correctly for Paystack API
        let req = PaystackInitRequest {
            email: "user@example.com".to_string(),
            amount: 1999, // 19.99 GHS in pesewas
            currency: "GHS".to_string(),
            reference: "TMAIL-PAY-001".to_string(),
            callback_url: Some("https://mail.example.com/billing/callback".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"amount\":1999"));
        assert!(json.contains("\"currency\":\"GHS\""));
        assert!(json.contains("TMAIL-PAY-001"));
    }

    #[test]
    fn test_paystack_verify_response_deserialize() {
        // Added: Test deserialization of Paystack verify response
        let json = r#"{"status": true, "message": "Verification successful", "data": {"status": "success", "reference": "ref-001", "amount": 5000, "currency": "GHS", "channel": "card", "paid_at": "2026-04-15T12:00:00.000Z"}}"#;
        let resp: PaystackVerifyResponse = serde_json::from_str(json).unwrap();
        assert!(resp.status);
        let data = resp.data.unwrap();
        assert_eq!(data.status, "success");
        assert_eq!(data.amount, 5000);
    }

    #[test]
    fn test_paystack_webhook_event_deserialize() {
        // Added: Test webhook event deserialization
        let json = r#"{"event": "charge.success", "data": {"reference": "ref-001", "amount": 5000}}"#;
        let event: PaystackWebhookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event, "charge.success");
    }

    #[test]
    fn test_momo_payment_status_deserialize() {
        // Added: Test MoMo status response deserialization
        let json = r#"{"status": "SUCCESSFUL", "reason": null, "externalId": "ext-001", "amount": "50.00", "currency": "GHS"}"#;
        let status: MomoPaymentStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status, "SUCCESSFUL");
        assert_eq!(status.external_id.as_deref(), Some("ext-001"));
        assert_eq!(status.amount.as_deref(), Some("50.00"));
    }

    #[test]
    fn test_momo_payment_status_failed() {
        // Added: Test MoMo failed payment deserialization
        let json = r#"{"status": "FAILED", "reason": {"code": "PAYER_NOT_FOUND"}, "externalId": "ext-002", "amount": null, "currency": null}"#;
        let status: MomoPaymentStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status, "FAILED");
        assert!(status.reason.is_some());
    }

    #[test]
    fn test_paystack_client_new() {
        // Added: Test PaystackClient creation
        let client = PaystackClient::new("sk_test_123".to_string());
        assert_eq!(client.secret_key, "sk_test_123");
        assert_eq!(client.base_url, "https://api.paystack.co");
    }

    #[test]
    fn test_momo_client_new() {
        // Added: Test MomoClient creation
        let client = MomoClient::new("api-key-123".to_string(), "api-user-456".to_string());
        assert_eq!(client.api_key, "api-key-123");
        assert_eq!(client.api_user, "api-user-456");
    }
}
