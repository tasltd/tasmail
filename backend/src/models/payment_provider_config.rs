// Added: Payment provider credential model — mirrors PayPro's PaymentProviderConfig domain.
// Sensitive fields (secret_key, api_password, etc.) are stored as encrypted ciphertext (AES-256-GCM).
// The model holds only ciphertext; callers use `decrypt_with()` to materialise plaintext on demand.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::services::encryption::EncryptionService;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PaymentProviderConfig {
    pub id: Uuid,
    pub provider: String, // PAYSTACK | MASTERCARD | CYBERSOURCE | BANK_TRANSFER
    pub tenant_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,

    // Encrypted ciphertext
    pub secret_key: Option<String>,
    pub public_key: Option<String>,
    pub webhook_secret: Option<String>,
    pub merchant_id: Option<String>,
    pub api_password: Option<String>,
    pub key_id: Option<String>,
    pub shared_secret_key: Option<String>,
    pub key_file_path: Option<String>,

    pub base_url: Option<String>,
    pub callback_url: Option<String>,
    pub currency: Option<String>,
    pub environment: Option<String>,
    pub enabled: bool,
    pub archived: bool,

    pub bank_details: Option<serde_json::Value>,
    pub split_code: Option<String>,
    pub notes: Option<String>,

    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// PURPOSE: Plaintext credentials, materialised on demand from a `PaymentProviderConfig` row.
/// Never persist this struct or log it.
#[derive(Debug, Clone, Default)]
pub struct DecryptedProviderConfig {
    pub provider: String,
    pub secret_key: Option<String>,
    pub public_key: Option<String>,
    pub webhook_secret: Option<String>,
    pub merchant_id: Option<String>,
    pub api_password: Option<String>,
    pub key_id: Option<String>,
    pub shared_secret_key: Option<String>,
    pub key_file_path: Option<String>,
    pub base_url: Option<String>,
    pub callback_url: Option<String>,
    pub currency: Option<String>,
    pub environment: Option<String>,
    pub bank_details: Option<serde_json::Value>,
    pub split_code: Option<String>,
}

impl PaymentProviderConfig {
    /// PURPOSE: Resolve effective config for a provider, preferring tenant-scoped over global.
    /// Returns `None` if no enabled, non-archived row exists for the provider.
    /// Mirrors PayPro's PaymentProviderConfigService priority order.
    pub async fn resolve(
        pool: &PgPool,
        provider: &str,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<Self>, sqlx::Error> {
        // Try tenant-scoped first
        if let Some(tid) = tenant_id {
            let tenant_row = sqlx::query_as::<_, PaymentProviderConfig>(
                "SELECT * FROM payment_provider_config
                 WHERE provider = $1 AND tenant_id = $2 AND enabled = true AND archived = false
                 ORDER BY updated_at DESC LIMIT 1",
            )
            .bind(provider)
            .bind(tid)
            .fetch_optional(pool)
            .await?;
            if tenant_row.is_some() {
                return Ok(tenant_row);
            }
        }

        // Fall back to global (tenant_id IS NULL)
        sqlx::query_as::<_, PaymentProviderConfig>(
            "SELECT * FROM payment_provider_config
             WHERE provider = $1 AND tenant_id IS NULL AND enabled = true AND archived = false
             ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(provider)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: List all configs (admin endpoint use)
    pub async fn list_all(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, PaymentProviderConfig>(
            "SELECT * FROM payment_provider_config WHERE archived = false ORDER BY provider, tenant_id NULLS FIRST",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Decrypt all sensitive fields into a plaintext struct for use by HTTP clients.
    /// Returns Err if any non-empty ciphertext fails to decrypt.
    pub fn decrypt_with(&self, enc: &EncryptionService) -> anyhow::Result<DecryptedProviderConfig> {
        let dec = |opt: &Option<String>| -> anyhow::Result<Option<String>> {
            match opt {
                Some(ct) if !ct.is_empty() => Ok(Some(enc.decrypt(ct)?)),
                _ => Ok(None),
            }
        };
        Ok(DecryptedProviderConfig {
            provider: self.provider.clone(),
            secret_key: dec(&self.secret_key)?,
            public_key: dec(&self.public_key)?,
            webhook_secret: dec(&self.webhook_secret)?,
            merchant_id: dec(&self.merchant_id)?,
            api_password: dec(&self.api_password)?,
            key_id: dec(&self.key_id)?,
            shared_secret_key: dec(&self.shared_secret_key)?,
            key_file_path: dec(&self.key_file_path)?,
            base_url: self.base_url.clone(),
            callback_url: self.callback_url.clone(),
            currency: self.currency.clone(),
            environment: self.environment.clone(),
            bank_details: self.bank_details.clone(),
            split_code: self.split_code.clone(),
        })
    }

    /// PURPOSE: Insert a new config row, encrypting sensitive fields with the given EncryptionService.
    /// All Option<&str> plaintext args are encrypted before storage.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        pool: &PgPool,
        enc: &EncryptionService,
        provider: &str,
        tenant_id: Option<Uuid>,
        name: Option<&str>,
        plaintext: PlaintextProviderConfig<'_>,
    ) -> anyhow::Result<Self> {
        let enc_opt = |opt: Option<&str>| -> anyhow::Result<Option<String>> {
            match opt {
                Some(p) if !p.is_empty() => Ok(Some(enc.encrypt(p)?)),
                _ => Ok(None),
            }
        };

        let row = sqlx::query_as::<_, PaymentProviderConfig>(
            "INSERT INTO payment_provider_config
                (provider, tenant_id, name, description,
                 secret_key, public_key, webhook_secret, merchant_id, api_password,
                 key_id, shared_secret_key, key_file_path,
                 base_url, callback_url, currency, environment, bank_details, split_code, notes)
             VALUES ($1,$2,$3,$4, $5,$6,$7,$8,$9, $10,$11,$12, $13,$14,$15,$16,$17,$18,$19)
             RETURNING *",
        )
        .bind(provider)
        .bind(tenant_id)
        .bind(name)
        .bind(plaintext.description)
        .bind(enc_opt(plaintext.secret_key)?)
        .bind(enc_opt(plaintext.public_key)?)
        .bind(enc_opt(plaintext.webhook_secret)?)
        .bind(enc_opt(plaintext.merchant_id)?)
        .bind(enc_opt(plaintext.api_password)?)
        .bind(enc_opt(plaintext.key_id)?)
        .bind(enc_opt(plaintext.shared_secret_key)?)
        .bind(enc_opt(plaintext.key_file_path)?)
        .bind(plaintext.base_url)
        .bind(plaintext.callback_url)
        .bind(plaintext.currency)
        .bind(plaintext.environment)
        .bind(plaintext.bank_details)
        .bind(plaintext.split_code)
        .bind(plaintext.notes)
        .fetch_one(pool)
        .await?;

        Ok(row)
    }
}

/// PURPOSE: Plaintext-input struct for inserts/updates. Fields are encrypted before persistence.
#[derive(Debug, Default)]
pub struct PlaintextProviderConfig<'a> {
    pub description: Option<&'a str>,
    pub secret_key: Option<&'a str>,
    pub public_key: Option<&'a str>,
    pub webhook_secret: Option<&'a str>,
    pub merchant_id: Option<&'a str>,
    pub api_password: Option<&'a str>,
    pub key_id: Option<&'a str>,
    pub shared_secret_key: Option<&'a str>,
    pub key_file_path: Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub callback_url: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub environment: Option<&'a str>,
    pub bank_details: Option<serde_json::Value>,
    pub split_code: Option<&'a str>,
    pub notes: Option<&'a str>,
}
