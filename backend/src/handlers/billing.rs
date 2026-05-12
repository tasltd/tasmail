// Changed: Billing handlers now match PayPro's provider set — Paystack, Mastercard, Cybersource, BankTransfer (TMAIL-46).
// MoMo provider removed: TASMail mirrors PayPro, which does not use MoMo.
// Webhook endpoints are public (no auth) — verified via HMAC signature (Paystack/Mastercard).

use axum::{
    extract::{State, Json as AxumJson},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::error::AppError;
use crate::models::billing::{BillingPlan, Payment, Subscription, SubscribeRequest, SubscribeResponse};
// Added: DB-backed credential lookup (PayPro PaymentProviderConfig pattern). Replaces env-var config.
use crate::models::payment_provider_config::{DecryptedProviderConfig, PaymentProviderConfig};
use crate::services::auth_service::Claims;
use crate::services::payment_service::{
    verify_mastercard_webhook, verify_paystack_signature, BankInstructionConfig, CybersourceClient,
    CybersourceAmount, CybersourceCustomerInfo, CybersourceInvoiceInfo, CybersourceInvoiceRequest,
    CybersourceOrderInfo, MastercardClient, PaystackClient, PaystackInitRequest, PaystackWebhookEvent,
};
use crate::state::AppState;

/// PURPOSE: Load and decrypt the active config row for a provider, returning a 503 if no config exists.
/// Mirrors PayPro's `PaymentProviderConfigService.findEffectiveConfig(provider, tenantId)`.
async fn load_provider(
    state: &AppState,
    provider: &str,
) -> Result<DecryptedProviderConfig, AppError> {
    let row = PaymentProviderConfig::resolve(&state.db, provider, None)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("DB error loading {} config: {}", provider, e)))?
        .ok_or_else(|| AppError::ServiceUnavailable(format!(
            "{} payment provider is not configured. Add a row in payment_provider_config.",
            provider
        )))?;
    row.decrypt_with(&state.encryption)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to decrypt {} credentials: {}", provider, e)))
}

/// PURPOSE: List all active billing plans (public endpoint)
/// GET /api/billing/plans
pub async fn list_plans(
    State(state): State<AppState>,
) -> Result<Json<Vec<BillingPlan>>, AppError> {
    let plans = BillingPlan::list_active(&state.db).await?;
    Ok(Json(plans))
}

/// PURPOSE: Get current user's active subscription
/// GET /api/billing/subscription
pub async fn get_subscription(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let sub = Subscription::find_active_by_user(&state.db, user_id).await?;
    match sub {
        Some(s) => Ok(Json(serde_json::to_value(s).unwrap_or_default())),
        None => Ok(Json(serde_json::json!(null))),
    }
}

/// PURPOSE: Initialize a new subscription — calls Paystack or MoMo to start payment
/// POST /api/billing/subscribe
pub async fn subscribe(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SubscribeRequest>,
) -> Result<(StatusCode, Json<SubscribeResponse>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate the selected plan exists and is active
    let plan = BillingPlan::find_by_id(&state.db, body.plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan not found".to_string()))?;

    if plan.active != Some(true) {
        return Err(AppError::BadRequest("Plan is no longer active".to_string()));
    }

    // Changed: Provider whitelist matches PayPro's four supported providers.
    let allowed = ["paystack", "mastercard", "cybersource", "bank_transfer"];
    if !allowed.contains(&body.provider.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Provider must be one of: {}",
            allowed.join(", ")
        )));
    }

    // Added: Create subscription record in pending state
    let subscription = Subscription::create(&state.db, user_id, body.plan_id, &body.provider).await?;

    // Added: Generate unique payment reference
    let reference = format!("TMAIL-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("pay"));

    // Added: Convert plan price to pesewas (integer) for Paystack
    let amount_pesewas = (plan.price_cedis * 100.0) as i64;

    // Added: Create payment record
    let payment = Payment::create(
        &state.db,
        user_id,
        subscription.id,
        &body.provider,
        &reference,
        plan.price_cedis,
    )
    .await?;

    // Changed: Credentials are now loaded from the payment_provider_config DB table
    // (mirrors PayPro's PaymentProviderConfig pattern). No env-var fallback for production secrets.
    let authorization_url = match body.provider.as_str() {
        "paystack" => {
            let pcfg = load_provider(&state, "PAYSTACK").await?;
            let secret = pcfg.secret_key.ok_or_else(|| {
                AppError::ServiceUnavailable("Paystack secret_key missing in DB config".into())
            })?;
            let client = PaystackClient::new(secret);
            let init_req = PaystackInitRequest {
                email: claims.username.clone(),
                amount: amount_pesewas,
                currency: pcfg.currency.unwrap_or_else(|| "GHS".to_string()),
                reference: reference.clone(),
                callback_url: pcfg.callback_url,
            };
            match client.initialize_transaction(&init_req).await {
                Ok(resp) if resp.status => resp.data.map(|d| d.authorization_url),
                Ok(resp) => {
                    tracing::error!("Paystack init failed: {}", resp.message);
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Payment initialization failed: {}",
                        resp.message
                    )));
                }
                Err(e) => {
                    tracing::error!("Paystack API error: {}", e);
                    return Err(AppError::Internal(anyhow::anyhow!("Payment provider unavailable")));
                }
            }
        }
        "mastercard" => {
            let pcfg = load_provider(&state, "MASTERCARD").await?;
            let merchant_id = pcfg.merchant_id.ok_or_else(|| {
                AppError::ServiceUnavailable("Mastercard merchant_id missing in DB config".into())
            })?;
            let api_password = pcfg.api_password.ok_or_else(|| {
                AppError::ServiceUnavailable("Mastercard api_password missing in DB config".into())
            })?;
            let mut client = MastercardClient::new(merchant_id, api_password);
            if let Some(url) = pcfg.base_url { client = client.with_base_url(url); }
            if let Some(cur) = pcfg.currency { client = client.with_currency(cur); }

            let return_url = pcfg.callback_url.unwrap_or_else(|| {
                format!("https://mail.techatscale.io/billing/callback/mastercard?ref={}", reference)
            });
            match client.initialize_payment(&reference, plan.price_cedis, &return_url).await {
                Ok(resp) if resp.result == "SUCCESS" => {
                    resp.session.map(|s| format!("mpgs:session:{}", s.id))
                }
                Ok(resp) => {
                    tracing::error!("Mastercard init failed: result={}", resp.result);
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Mastercard payment init failed: {}",
                        resp.result
                    )));
                }
                Err(e) => {
                    tracing::error!("Mastercard API error: {}", e);
                    return Err(AppError::Internal(anyhow::anyhow!("Mastercard provider unavailable")));
                }
            }
        }
        "cybersource" => {
            let pcfg = load_provider(&state, "CYBERSOURCE").await?;
            let merchant_id = pcfg.merchant_id.ok_or_else(|| {
                AppError::ServiceUnavailable("Cybersource merchant_id missing in DB config".into())
            })?;
            let key_id = pcfg.key_id.ok_or_else(|| {
                AppError::ServiceUnavailable("Cybersource key_id missing in DB config".into())
            })?;
            let shared = pcfg.shared_secret_key.ok_or_else(|| {
                AppError::ServiceUnavailable("Cybersource shared_secret_key missing in DB config".into())
            })?;
            let mut client = CybersourceClient::new(merchant_id, key_id, shared);
            if let Some(url) = pcfg.base_url { client = client.with_base_url(url); }

            let due = (chrono::Utc::now() + chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
            let req = CybersourceInvoiceRequest {
                invoice_information: CybersourceInvoiceInfo {
                    invoice_number: reference.clone(),
                    description: "TASMail Subscription".to_string(),
                    due_date: due,
                    send_immediately: true,
                },
                customer_information: CybersourceCustomerInfo {
                    name: claims.username.clone(),
                    email: claims.username.clone(),
                },
                order_information: CybersourceOrderInfo {
                    amount_details: CybersourceAmount {
                        total_amount: format!("{:.2}", plan.price_cedis),
                        currency: pcfg.currency.unwrap_or_else(|| "GHS".to_string()),
                    },
                },
            };
            match client.initialize_payment(req).await {
                Ok(resp) => resp.id.map(|invoice_id| format!("cybersource:invoice:{}", invoice_id)),
                Err(e) => {
                    tracing::error!("Cybersource API error: {}", e);
                    return Err(AppError::Internal(anyhow::anyhow!("Cybersource provider unavailable")));
                }
            }
        }
        "bank_transfer" => {
            // Bank-transfer details live in pcfg.bank_details (JSONB), not in encrypted columns.
            let pcfg = load_provider(&state, "BANK_TRANSFER").await?;
            let details = pcfg.bank_details.clone().ok_or_else(|| {
                AppError::ServiceUnavailable(
                    "BANK_TRANSFER row is missing bank_details JSON".into(),
                )
            })?;
            let cfg = BankInstructionConfig {
                bank_name: details.get("bank_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                account_name: details.get("account_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                account_number: details.get("account_number").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                branch: details.get("branch").and_then(|v| v.as_str()).map(String::from),
                swift_code: details.get("swift_code").and_then(|v| v.as_str()).map(String::from),
                reference_prefix: details.get("reference_prefix").and_then(|v| v.as_str()).map(String::from),
            };
            let currency = pcfg.currency.unwrap_or_else(|| "GHS".to_string());
            let instructions = cfg.build_instructions(plan.price_cedis, &currency, &reference);
            Some(format!("bank_transfer:{}", instructions))
        }
        _ => unreachable!(),
    };

    Ok((
        StatusCode::CREATED,
        Json(SubscribeResponse {
            subscription_id: subscription.id,
            payment_id: payment.id,
            provider: body.provider,
            authorization_url,
            reference,
        }),
    ))
}

/// PURPOSE: Paystack webhook handler — verifies HMAC-SHA512 signature and processes payment events
/// POST /api/billing/webhook/paystack
pub async fn webhook_paystack(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, AppError> {
    // Added: Extract and verify Paystack webhook signature
    let signature = headers
        .get("x-paystack-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing Paystack signature".to_string()))?;

    // Changed: Load Paystack signing secret from payment_provider_config DB row (not env).
    let pcfg = load_provider(&state, "PAYSTACK").await?;
    let paystack_key = pcfg.secret_key.ok_or_else(|| {
        AppError::ServiceUnavailable("Paystack secret_key missing in DB config".into())
    })?;

    if !verify_paystack_signature(&paystack_key, &body, signature) {
        tracing::warn!("Invalid Paystack webhook signature");
        return Err(AppError::Unauthorized("Invalid signature".to_string()));
    }

    // Added: Parse webhook event payload
    let event: PaystackWebhookEvent = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("Invalid webhook payload: {}", e)))?;

    tracing::info!("Paystack webhook: event={}", event.event);

    // Added: Handle charge.success event to activate subscription
    if event.event == "charge.success" {
        if let Some(reference) = event.data.get("reference").and_then(|v| v.as_str()) {
            let new_status = "success";
            if let Some(payment) =
                Payment::update_status(&state.db, reference, new_status, event.data.clone()).await?
            {
                // Added: Activate the linked subscription
                if let Some(sub_id) = payment.subscription_id {
                    let now = chrono::Utc::now();
                    // NOTE: Default to 30-day period; adjust based on plan interval
                    let period_end = now + chrono::Duration::days(30);
                    let _ = Subscription::activate(&state.db, sub_id, None, now, period_end).await;
                }
            }
        }
    }

    Ok(StatusCode::OK)
}

/// PURPOSE: Mastercard MPGS webhook handler — verifies HMAC-SHA256 signature, activates subscription on PAYMENT_SUCCESS.
/// POST /api/billing/webhook/mastercard
/// Replaces the previous MoMo webhook (TASMail mirrors PayPro, which uses Mastercard not MoMo).
pub async fn webhook_mastercard(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, AppError> {
    let signature = headers
        .get("x-notification-secret")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing Mastercard signature".to_string()))?;

    // Changed: Load Mastercard webhook secret from payment_provider_config DB row.
    let pcfg = load_provider(&state, "MASTERCARD").await?;
    let secret = pcfg.webhook_secret.ok_or_else(|| {
        AppError::ServiceUnavailable("Mastercard webhook_secret missing in DB config".into())
    })?;

    if !verify_mastercard_webhook(&secret, &body, signature) {
        tracing::warn!("Invalid Mastercard webhook signature");
        return Err(AppError::Unauthorized("Invalid signature".to_string()));
    }

    let event: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("Invalid Mastercard webhook payload: {}", e)))?;

    let order_id = event
        .get("order")
        .and_then(|o| o.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing order.id in Mastercard webhook".to_string()))?;
    let result = event.get("result").and_then(|v| v.as_str()).unwrap_or("");
    let new_status = match result {
        "SUCCESS" | "CAPTURED" => "success",
        "FAILURE" | "DECLINED" => "failed",
        _ => "pending",
    };

    if let Some(payment) =
        Payment::update_status(&state.db, order_id, new_status, event.clone()).await?
    {
        if new_status == "success" {
            if let Some(sub_id) = payment.subscription_id {
                let now = chrono::Utc::now();
                let period_end = now + chrono::Duration::days(30);
                let _ = Subscription::activate(&state.db, sub_id, None, now, period_end).await;
            }
        }
    }

    Ok(StatusCode::OK)
}

/// PURPOSE: List payment history for the current user
/// GET /api/billing/payments
pub async fn list_payments(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Payment>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let payments = Payment::list_by_user(&state.db, user_id).await?;
    Ok(Json(payments))
}

/// PURPOSE: Parse UUID user_id from JWT claims
fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in token")))
}
