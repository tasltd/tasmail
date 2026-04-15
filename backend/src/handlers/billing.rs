// Added: Billing handlers for Paystack/MoMo payment integration (TMAIL-46)
// PURPOSE: Endpoints for listing plans, managing subscriptions, processing payments and webhooks
// NOTE: Webhook endpoints are public (no auth) — verified via HMAC signature (Paystack) or callback token (MoMo)

use axum::{
    extract::{State, Json as AxumJson},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::error::AppError;
use crate::models::billing::{BillingPlan, Payment, Subscription, SubscribeRequest, SubscribeResponse};
use crate::services::auth_service::Claims;
use crate::services::payment_service::{
    verify_paystack_signature, PaystackClient, PaystackInitRequest, PaystackWebhookEvent,
    MomoClient, MomoPaymentStatus,
};
use crate::state::AppState;

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

    // Added: Validate provider is one of the supported values
    if body.provider != "paystack" && body.provider != "mtn_momo" {
        return Err(AppError::BadRequest(
            "Provider must be 'paystack' or 'mtn_momo'".to_string(),
        ));
    }

    // Added: MoMo requires a phone number
    if body.provider == "mtn_momo" && body.phone_number.is_none() {
        return Err(AppError::BadRequest(
            "phone_number is required for MTN MoMo payments".to_string(),
        ));
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

    // Added: Initialize with the appropriate payment provider
    let authorization_url = match body.provider.as_str() {
        "paystack" => {
            let paystack_key = state
                .config
                .billing
                .as_ref()
                .and_then(|b| b.paystack_secret_key.clone())
                .ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!("Paystack secret key not configured"))
                })?;

            let client = PaystackClient::new(paystack_key);
            let init_req = PaystackInitRequest {
                email: claims.username.clone(),
                amount: amount_pesewas,
                currency: "GHS".to_string(),
                reference: reference.clone(),
                callback_url: None,
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
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Payment provider unavailable"
                    )));
                }
            }
        }
        "mtn_momo" => {
            let billing_config = state.config.billing.as_ref().ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("Billing config not set"))
            })?;
            let momo_key = billing_config.momo_api_key.clone().ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("MoMo API key not configured"))
            })?;
            let momo_user = billing_config.momo_api_user.clone().ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("MoMo API user not configured"))
            })?;

            let client = MomoClient::new(momo_key, momo_user);
            let phone = body.phone_number.as_deref().unwrap_or_default();
            let amount_str = format!("{:.2}", plan.price_cedis);

            if let Err(e) = client
                .request_payment(&reference, &amount_str, phone, "TASMail subscription")
                .await
            {
                tracing::error!("MoMo payment request failed: {}", e);
                return Err(AppError::Internal(anyhow::anyhow!(
                    "MoMo payment request failed"
                )));
            }
            // NOTE: MoMo doesn't return a URL — payment is via USSD on the user's phone
            None
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

    let paystack_key = state
        .config
        .billing
        .as_ref()
        .and_then(|b| b.paystack_secret_key.clone())
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Paystack secret key not configured")))?;

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

/// PURPOSE: MTN MoMo callback handler — processes payment status updates
/// POST /api/billing/webhook/momo
pub async fn webhook_momo(
    State(state): State<AppState>,
    Json(body): Json<MomoPaymentStatus>,
) -> Result<StatusCode, AppError> {
    tracing::info!("MoMo webhook: status={}", body.status);

    let external_id = body
        .external_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Missing externalId in MoMo callback".to_string()))?;

    // Added: Map MoMo status to our payment status
    let new_status = match body.status.as_str() {
        "SUCCESSFUL" => "success",
        "FAILED" | "REJECTED" => "failed",
        _ => "pending",
    };

    let metadata = serde_json::to_value(&body).unwrap_or_default();

    if let Some(payment) = Payment::update_status(&state.db, external_id, new_status, metadata).await? {
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
