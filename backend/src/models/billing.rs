// Added: Billing models for Paystack/MoMo payment integration (TMAIL-46)
// PURPOSE: Data structs for billing plans, subscriptions, and payment records
// CONSTRAINTS: RLS enforced at DB level for subscriptions and payments

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: A subscription billing plan with pricing in GHS (Ghana Cedis)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BillingPlan {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    // NOTE: DECIMAL(10,2) maps to f64 for simplicity; use string formatting for display
    pub price_cedis: f64,
    pub interval: String,
    pub max_mailboxes: i32,
    pub storage_gb: i32,
    pub features: serde_json::Value,
    pub active: Option<bool>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: A user's active subscription linking them to a billing plan
/// NOTE: RLS ensures users can only see their own subscriptions
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub plan_id: Uuid,
    pub provider: String,
    pub provider_subscription_id: Option<String>,
    pub status: String,
    pub current_period_start: Option<chrono::DateTime<chrono::Utc>>,
    pub current_period_end: Option<chrono::DateTime<chrono::Utc>>,
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: A payment record for a subscription transaction
/// NOTE: RLS ensures users can only see their own payments
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subscription_id: Option<Uuid>,
    pub provider: String,
    pub provider_ref: String,
    pub amount_cedis: f64,
    pub currency: Option<String>,
    pub status: String,
    pub metadata: serde_json::Value,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Request body for subscribing to a plan
/// Changed: Provider whitelist is now {paystack, mastercard, cybersource, bank_transfer} (PayPro parity).
/// `phone_number` retained for backwards compatibility but no longer required.
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub plan_id: Uuid,
    pub provider: String,
    #[serde(default)]
    pub phone_number: Option<String>,
}

/// PURPOSE: Response from initializing a payment (Paystack authorization URL or MoMo reference)
#[derive(Debug, Serialize)]
pub struct SubscribeResponse {
    pub subscription_id: Uuid,
    pub payment_id: Uuid,
    pub provider: String,
    // Added: Paystack returns an authorization_url; MoMo returns a reference for USSD
    pub authorization_url: Option<String>,
    pub reference: String,
}

impl BillingPlan {
    /// PURPOSE: List all active billing plans (public endpoint)
    pub async fn list_active(pool: &PgPool) -> Result<Vec<BillingPlan>, sqlx::Error> {
        sqlx::query_as::<_, BillingPlan>(
            "SELECT * FROM billing_plans WHERE active = true ORDER BY price_cedis ASC",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Find a billing plan by ID
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<BillingPlan>, sqlx::Error> {
        sqlx::query_as::<_, BillingPlan>("SELECT * FROM billing_plans WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }
}

impl Subscription {
    /// PURPOSE: Get the current user's active subscription
    pub async fn find_active_by_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<Subscription>, sqlx::Error> {
        sqlx::query_as::<_, Subscription>(
            "SELECT * FROM subscriptions WHERE user_id = $1 AND status = 'active' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create a new subscription record
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        plan_id: Uuid,
        provider: &str,
    ) -> Result<Subscription, sqlx::Error> {
        sqlx::query_as::<_, Subscription>(
            "INSERT INTO subscriptions (user_id, plan_id, provider, status) VALUES ($1, $2, $3, 'pending') RETURNING *",
        )
        .bind(user_id)
        .bind(plan_id)
        .bind(provider)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Activate a subscription after successful payment
    pub async fn activate(
        pool: &PgPool,
        id: Uuid,
        provider_subscription_id: Option<&str>,
        period_start: chrono::DateTime<chrono::Utc>,
        period_end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<Subscription>, sqlx::Error> {
        sqlx::query_as::<_, Subscription>(
            "UPDATE subscriptions SET status = 'active', provider_subscription_id = $2, current_period_start = $3, current_period_end = $4, updated_at = now() WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(provider_subscription_id)
        .bind(period_start)
        .bind(period_end)
        .fetch_optional(pool)
        .await
    }
}

impl Payment {
    /// PURPOSE: Create a new payment record
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        subscription_id: Uuid,
        provider: &str,
        provider_ref: &str,
        amount_cedis: f64,
    ) -> Result<Payment, sqlx::Error> {
        sqlx::query_as::<_, Payment>(
            "INSERT INTO payments (user_id, subscription_id, provider, provider_ref, amount_cedis, status) VALUES ($1, $2, $3, $4, $5, 'pending') RETURNING *",
        )
        .bind(user_id)
        .bind(subscription_id)
        .bind(provider)
        .bind(provider_ref)
        .bind(amount_cedis)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update payment status after webhook/callback confirmation
    pub async fn update_status(
        pool: &PgPool,
        provider_ref: &str,
        status: &str,
        metadata: serde_json::Value,
    ) -> Result<Option<Payment>, sqlx::Error> {
        sqlx::query_as::<_, Payment>(
            "UPDATE payments SET status = $2::payment_status, metadata = $3 WHERE provider_ref = $1 RETURNING *",
        )
        .bind(provider_ref)
        .bind(status)
        .bind(metadata)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: List payments for a specific user
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Payment>, sqlx::Error> {
        sqlx::query_as::<_, Payment>(
            "SELECT * FROM payments WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Find a payment by provider reference
    pub async fn find_by_ref(
        pool: &PgPool,
        provider_ref: &str,
    ) -> Result<Option<Payment>, sqlx::Error> {
        sqlx::query_as::<_, Payment>("SELECT * FROM payments WHERE provider_ref = $1")
            .bind(provider_ref)
            .fetch_optional(pool)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_request_deserialize() {
        // Added: Verify request deserialization with Paystack provider
        let json = r#"{"plan_id": "550e8400-e29b-41d4-a716-446655440000", "provider": "paystack"}"#;
        let req: SubscribeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.provider, "paystack");
        assert!(req.phone_number.is_none());
    }

    #[test]
    fn test_subscribe_request_mastercard() {
        // Changed: MoMo replaced by Mastercard (PayPro provider parity).
        let json = r#"{"plan_id": "550e8400-e29b-41d4-a716-446655440000", "provider": "mastercard"}"#;
        let req: SubscribeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.provider, "mastercard");
        assert!(req.phone_number.is_none());
    }

    #[test]
    fn test_subscribe_request_bank_transfer() {
        let json = r#"{"plan_id": "550e8400-e29b-41d4-a716-446655440000", "provider": "bank_transfer"}"#;
        let req: SubscribeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.provider, "bank_transfer");
    }

    #[test]
    fn test_subscribe_response_serialize() {
        // Added: Verify response serialization includes all fields
        let resp = SubscribeResponse {
            subscription_id: Uuid::new_v4(),
            payment_id: Uuid::new_v4(),
            provider: "paystack".to_string(),
            authorization_url: Some("https://checkout.paystack.com/abc123".to_string()),
            reference: "TMAIL-PAY-001".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("authorization_url"));
        assert!(json.contains("TMAIL-PAY-001"));
    }

    #[test]
    fn test_billing_plan_serialize() {
        // Added: Verify BillingPlan serialization round-trip
        let plan = BillingPlan {
            id: Uuid::new_v4(),
            name: "Basic".to_string(),
            description: Some("Basic plan".to_string()),
            price_cedis: 19.99,
            interval: "monthly".to_string(),
            max_mailboxes: 1,
            storage_gb: 5,
            features: serde_json::json!({"custom_domain": false}),
            active: Some(true),
            created_at: None,
            updated_at: None,
        };
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("Basic"));
        assert!(json.contains("19.99"));
    }

    #[test]
    fn test_payment_serialize() {
        // Added: Verify Payment struct serialization
        let payment = Payment {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            subscription_id: Some(Uuid::new_v4()),
            provider: "mtn_momo".to_string(),
            provider_ref: "MOMO-REF-001".to_string(),
            amount_cedis: 49.99,
            currency: Some("GHS".to_string()),
            status: "success".to_string(),
            metadata: serde_json::json!({}),
            created_at: None,
        };
        let json = serde_json::to_string(&payment).unwrap();
        assert!(json.contains("mtn_momo"));
        assert!(json.contains("49.99"));
    }
}
