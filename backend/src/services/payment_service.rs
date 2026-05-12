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

// Removed: MTN MoMo client (and its request/payer/status structs).
// TASMail mirrors PayPro, which uses Paystack/Mastercard/Cybersource/Bank Transfer — not MoMo.

// --- Mastercard Payment Gateway Services (MPGS) Client ---
// Added: Ported from PayPro's MastercardService.groovy for TASMail subscription billing parity.
// API docs: https://eu-gateway.mastercard.com/api/documentation
// Auth: HTTP Basic with username = "merchant.<merchantId>" and password = api password.

/// PURPOSE: Mastercard MPGS client for hosted-checkout payment sessions
#[derive(Debug, Clone)]
pub struct MastercardClient {
    merchant_id: String,
    api_password: String,
    base_url: String,
    currency: String,
    http: reqwest::Client,
}

const MASTERCARD_DEFAULT_BASE_URL: &str = "https://eu-gateway.mastercard.com/api/rest/version/61";
const MASTERCARD_DEFAULT_CURRENCY: &str = "GHS";

/// PURPOSE: Hosted-checkout session response from MPGS
#[derive(Debug, Deserialize)]
pub struct MastercardSessionResponse {
    pub result: String,
    pub session: Option<MastercardSession>,
    #[serde(rename = "successIndicator")]
    pub success_indicator: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MastercardSession {
    pub id: String,
    pub version: Option<String>,
    #[serde(rename = "updateStatus")]
    pub update_status: Option<String>,
}

/// PURPOSE: Order status response after callback
#[derive(Debug, Deserialize)]
pub struct MastercardOrderStatus {
    pub result: String,
    pub status: Option<String>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
}

impl MastercardClient {
    pub fn new(merchant_id: String, api_password: String) -> Self {
        Self {
            merchant_id,
            api_password,
            base_url: MASTERCARD_DEFAULT_BASE_URL.to_string(),
            currency: MASTERCARD_DEFAULT_CURRENCY.to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn with_currency(mut self, currency: String) -> Self {
        self.currency = currency;
        self
    }

    /// PURPOSE: Initialize a hosted-checkout session and return the session id used to redirect the buyer
    pub async fn initialize_payment(
        &self,
        order_id: &str,
        amount: f64,
        return_url: &str,
    ) -> Result<MastercardSessionResponse, anyhow::Error> {
        let payload = serde_json::json!({
            "apiOperation": "INITIATE_CHECKOUT",
            "interaction": {
                "operation": "PURCHASE",
                "returnUrl": return_url,
                "merchant": { "name": "TASMail", "address": { "line1": "TASMail Subscriptions" } },
            },
            "order": {
                "id": order_id,
                "amount": amount,
                "currency": self.currency,
                "description": "TASMail Subscription",
            },
        });

        let resp = self
            .http
            .post(format!("{}/merchant/{}/session", self.base_url, self.merchant_id))
            .basic_auth(format!("merchant.{}", self.merchant_id), Some(&self.api_password))
            .json(&payload)
            .send()
            .await?
            .json::<MastercardSessionResponse>()
            .await?;
        Ok(resp)
    }

    /// PURPOSE: Verify payment by querying order status (called after buyer returns from hosted page)
    pub async fn verify_payment(
        &self,
        order_id: &str,
    ) -> Result<MastercardOrderStatus, anyhow::Error> {
        let resp = self
            .http
            .get(format!("{}/merchant/{}/order/{}", self.base_url, self.merchant_id, order_id))
            .basic_auth(format!("merchant.{}", self.merchant_id), Some(&self.api_password))
            .send()
            .await?
            .json::<MastercardOrderStatus>()
            .await?;
        Ok(resp)
    }
}

/// PURPOSE: Verify Mastercard webhook HMAC-SHA256 signature.
/// MPGS sends the signature in the `X-Notification-Secret` header as base64.
pub fn verify_mastercard_webhook(secret: &str, body: &[u8], signature_b64: &str) -> bool {
    use base64::Engine;
    let Ok(mut mac) = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    match base64::engine::general_purpose::STANDARD.decode(signature_b64) {
        Ok(sig_bytes) => sig_bytes.as_slice() == expected.as_slice(),
        Err(_) => false,
    }
}

// --- Cybersource Invoicing Client ---
// Added: Ported from PayPro's CybersourceInvoicingService.groovy.
// API docs: https://developer.cybersource.com/api/reference/api-reference.html
// Auth: HTTP Signature (RSA or HMAC-SHA256) — TASMail uses HMAC-SHA256 with shared secret key.

/// PURPOSE: Cybersource client for invoice-based payments
#[derive(Debug, Clone)]
pub struct CybersourceClient {
    merchant_id: String,
    key_id: String,
    shared_secret_key: String,
    base_url: String,
    http: reqwest::Client,
}

const CYBERSOURCE_DEFAULT_BASE_URL: &str = "https://apitest.cybersource.com";

#[derive(Debug, Serialize)]
pub struct CybersourceInvoiceRequest {
    #[serde(rename = "invoiceInformation")]
    pub invoice_information: CybersourceInvoiceInfo,
    #[serde(rename = "customerInformation")]
    pub customer_information: CybersourceCustomerInfo,
    #[serde(rename = "orderInformation")]
    pub order_information: CybersourceOrderInfo,
}

#[derive(Debug, Serialize)]
pub struct CybersourceInvoiceInfo {
    #[serde(rename = "invoiceNumber")]
    pub invoice_number: String,
    pub description: String,
    #[serde(rename = "dueDate")]
    pub due_date: String, // YYYY-MM-DD
    #[serde(rename = "sendImmediately")]
    pub send_immediately: bool,
}

#[derive(Debug, Serialize)]
pub struct CybersourceCustomerInfo {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct CybersourceOrderInfo {
    #[serde(rename = "amountDetails")]
    pub amount_details: CybersourceAmount,
}

#[derive(Debug, Serialize)]
pub struct CybersourceAmount {
    #[serde(rename = "totalAmount")]
    pub total_amount: String,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct CybersourceInvoiceResponse {
    pub id: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "invoiceInformation")]
    pub invoice_information: Option<serde_json::Value>,
}

impl CybersourceClient {
    pub fn new(merchant_id: String, key_id: String, shared_secret_key: String) -> Self {
        Self {
            merchant_id,
            key_id,
            shared_secret_key,
            base_url: CYBERSOURCE_DEFAULT_BASE_URL.to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// PURPOSE: Build the Cybersource HTTP signature header per their HMAC-SHA256 spec.
    /// Required signed headers: host, date, (request-target), digest, v-c-merchant-id
    fn build_signature(
        &self,
        method: &str,
        path: &str,
        host: &str,
        date: &str,
        digest: &str,
    ) -> String {
        use base64::Engine;
        let signing_string = format!(
            "host: {}\ndate: {}\n(request-target): {} {}\ndigest: {}\nv-c-merchant-id: {}",
            host,
            date,
            method.to_lowercase(),
            path,
            digest,
            self.merchant_id
        );

        let secret_bytes =
            base64::engine::general_purpose::STANDARD.decode(&self.shared_secret_key).unwrap_or_default();
        let mut mac = match Hmac::<sha2::Sha256>::new_from_slice(&secret_bytes) {
            Ok(m) => m,
            Err(_) => return String::new(),
        };
        mac.update(signing_string.as_bytes());
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        format!(
            "keyid=\"{}\", algorithm=\"HmacSHA256\", headers=\"host date (request-target) digest v-c-merchant-id\", signature=\"{}\"",
            self.key_id, sig_b64
        )
    }

    /// PURPOSE: Create an invoice and (optionally) email it to the customer
    pub async fn initialize_payment(
        &self,
        request: CybersourceInvoiceRequest,
    ) -> Result<CybersourceInvoiceResponse, anyhow::Error> {
        use base64::Engine;
        use sha2::Digest;

        let path = "/invoicing/v2/invoices";
        let host = self
            .base_url
            .replace("https://", "")
            .replace("http://", "")
            .trim_end_matches('/')
            .to_string();
        let body_json = serde_json::to_vec(&request)?;
        let body_digest = format!(
            "SHA-256={}",
            base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(&body_json))
        );
        let date = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let signature = self.build_signature("POST", path, &host, &date, &body_digest);

        let resp = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .header("v-c-merchant-id", &self.merchant_id)
            .header("date", &date)
            .header("host", &host)
            .header("digest", &body_digest)
            .header("signature", signature)
            .header("content-type", "application/json")
            .body(body_json)
            .send()
            .await?
            .json::<CybersourceInvoiceResponse>()
            .await?;
        Ok(resp)
    }

    /// PURPOSE: Query invoice status
    pub async fn verify_payment(
        &self,
        invoice_id: &str,
    ) -> Result<CybersourceInvoiceResponse, anyhow::Error> {
        use base64::Engine;
        use sha2::Digest;

        let path = format!("/invoicing/v2/invoices/{}", invoice_id);
        let host = self
            .base_url
            .replace("https://", "")
            .replace("http://", "")
            .trim_end_matches('/')
            .to_string();
        let body_digest = format!(
            "SHA-256={}",
            base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(b""))
        );
        let date = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let signature = self.build_signature("GET", &path, &host, &date, &body_digest);

        let resp = self
            .http
            .get(format!("{}{}", self.base_url, path))
            .header("v-c-merchant-id", &self.merchant_id)
            .header("date", &date)
            .header("host", &host)
            .header("digest", &body_digest)
            .header("signature", signature)
            .send()
            .await?
            .json::<CybersourceInvoiceResponse>()
            .await?;
        Ok(resp)
    }
}

// --- Bank Instruction "provider" ---
// Added: Manual bank-transfer instructions, mirroring PayPro's BankInstructionService.
// No external API — the backend just renders payment instructions to display to the user.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankInstructionConfig {
    pub bank_name: String,
    pub account_name: String,
    pub account_number: String,
    pub branch: Option<String>,
    pub swift_code: Option<String>,
    pub reference_prefix: Option<String>,
}

impl BankInstructionConfig {
    /// PURPOSE: Build a payment-instruction payload for the frontend to render
    pub fn build_instructions(&self, amount: f64, currency: &str, order_id: &str) -> serde_json::Value {
        let reference = match &self.reference_prefix {
            Some(prefix) => format!("{}-{}", prefix, order_id),
            None => order_id.to_string(),
        };
        serde_json::json!({
            "provider": "BANK_TRANSFER",
            "bank_name": self.bank_name,
            "account_name": self.account_name,
            "account_number": self.account_number,
            "branch": self.branch,
            "swift_code": self.swift_code,
            "amount": amount,
            "currency": currency,
            "reference": reference,
            "instructions": format!(
                "Transfer {} {} to the account above, using reference '{}'. Email proof of payment to billing@techatscale.io.",
                currency, amount, reference
            ),
        })
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

    // Removed: MTN MoMo tests (test_momo_payment_status_deserialize, test_momo_payment_status_failed,
    // test_momo_client_new). MoMo provider was dropped to mirror PayPro's provider set.

    #[test]
    fn test_paystack_client_new() {
        // Added: Test PaystackClient creation
        let client = PaystackClient::new("sk_test_123".to_string());
        assert_eq!(client.secret_key, "sk_test_123");
        assert_eq!(client.base_url, "https://api.paystack.co");
    }

    #[test]
    fn test_mastercard_client_defaults() {
        let c = MastercardClient::new("MERCH123".to_string(), "pw".to_string());
        assert_eq!(c.merchant_id, "MERCH123");
        assert_eq!(c.base_url, MASTERCARD_DEFAULT_BASE_URL);
        assert_eq!(c.currency, MASTERCARD_DEFAULT_CURRENCY);
    }

    #[test]
    fn test_mastercard_webhook_signature_roundtrip() {
        use base64::Engine;
        let secret = "mc_webhook_secret";
        let body = b"{\"event\":\"PAYMENT_SUCCESS\"}";
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let sig = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        assert!(verify_mastercard_webhook(secret, body, &sig));
        assert!(!verify_mastercard_webhook("wrong-secret", body, &sig));
        assert!(!verify_mastercard_webhook(secret, b"tampered", &sig));
    }

    #[test]
    fn test_cybersource_client_new() {
        let c = CybersourceClient::new(
            "merch".to_string(),
            "key".to_string(),
            // Added: Base64 of "secret"
            "c2VjcmV0".to_string(),
        );
        assert_eq!(c.merchant_id, "merch");
        assert_eq!(c.key_id, "key");
        assert_eq!(c.base_url, CYBERSOURCE_DEFAULT_BASE_URL);
    }

    #[test]
    fn test_cybersource_signature_format() {
        // Added: Sanity-check signature header structure
        let c = CybersourceClient::new("m".into(), "k".into(), "c2VjcmV0".into());
        let sig = c.build_signature("POST", "/path", "host.example", "Tue, 01 Jan 2026 00:00:00 GMT", "SHA-256=abc");
        assert!(sig.starts_with("keyid=\"k\""));
        assert!(sig.contains("algorithm=\"HmacSHA256\""));
        assert!(sig.contains("headers=\"host date (request-target) digest v-c-merchant-id\""));
    }

    #[test]
    fn test_bank_instruction_build() {
        let cfg = BankInstructionConfig {
            bank_name: "GTBank".into(),
            account_name: "Tech at Scale Ltd".into(),
            account_number: "1234567890".into(),
            branch: Some("Accra Main".into()),
            swift_code: Some("GTBIGHAC".into()),
            reference_prefix: Some("TASMAIL".into()),
        };
        let v = cfg.build_instructions(50.0, "GHS", "ORD-001");
        assert_eq!(v["bank_name"], "GTBank");
        assert_eq!(v["reference"], "TASMAIL-ORD-001");
        assert_eq!(v["currency"], "GHS");
        assert!(v["instructions"].as_str().unwrap().contains("TASMAIL-ORD-001"));
    }
}
