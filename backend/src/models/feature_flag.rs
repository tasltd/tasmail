// TMAIL-165: feature flags model.
// DB-backed runtime toggles surfaced in the admin dashboard. Cached in Redis for
// hot lookups (every signup/login render queries them).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FeatureFlag {
    pub key: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub value: Option<serde_json::Value>,
    pub is_public: bool,
    pub updated_at: Option<DateTime<Utc>>,
    pub updated_by: Option<Uuid>,
}

impl FeatureFlag {
    pub async fn list_all(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, FeatureFlag>("SELECT * FROM feature_flags ORDER BY key")
            .fetch_all(pool)
            .await
    }

    /// PURPOSE: Public-facing subset — used by the SPA's anonymous /signup page to
    /// know which onboarding paths to show.
    pub async fn list_public(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, FeatureFlag>(
            "SELECT * FROM feature_flags WHERE is_public = true ORDER BY key",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find(pool: &PgPool, key: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, FeatureFlag>("SELECT * FROM feature_flags WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await
    }

    /// PURPOSE: Convenience boolean check. Defaults to FALSE on missing key — always
    /// fail-closed for safety.
    pub async fn is_enabled(pool: &PgPool, key: &str) -> bool {
        Self::find(pool, key).await.ok().flatten().map(|f| f.enabled).unwrap_or(false)
    }

    pub async fn upsert(
        pool: &PgPool,
        key: &str,
        enabled: Option<bool>,
        value: Option<serde_json::Value>,
        actor: Option<Uuid>,
    ) -> Result<Self, sqlx::Error> {
        // Only update the columns the caller actually supplied. The COALESCE avoids
        // wiping `enabled`/`value` when the admin only changes one of them.
        sqlx::query_as::<_, FeatureFlag>(
            "UPDATE feature_flags
             SET enabled = COALESCE($2, enabled),
                 value   = COALESCE($3, value),
                 updated_by = $4
             WHERE key = $1
             RETURNING *",
        )
        .bind(key)
        .bind(enabled)
        .bind(value)
        .bind(actor)
        .fetch_one(pool)
        .await
    }
}
