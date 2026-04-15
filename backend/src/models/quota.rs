use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuotaUsage {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub used_bytes: i64,
    pub message_count: i32,
    pub last_synced_at: chrono::DateTime<chrono::Utc>,
}

/// Quota status returned to the API consumer
/// Changed: Added Deserialize for Redis cache serialization round-trip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub mailbox_id: Uuid,
    pub quota_bytes: i64,
    pub used_bytes: i64,
    pub message_count: i32,
    pub usage_percent: f64,
    pub quota_warn_percent: i32,
    pub is_over_quota: bool,
    pub is_warning: bool,
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl QuotaUsage {
    /// Upsert quota usage for a mailbox
    pub async fn upsert(
        pool: &PgPool,
        mailbox_id: Uuid,
        used_bytes: i64,
        message_count: i32,
    ) -> Result<QuotaUsage, sqlx::Error> {
        sqlx::query_as::<_, QuotaUsage>(
            "INSERT INTO quota_usage (mailbox_id, used_bytes, message_count, last_synced_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (mailbox_id) DO UPDATE
             SET used_bytes = $2, message_count = $3, last_synced_at = NOW()
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(used_bytes)
        .bind(message_count)
        .fetch_one(pool)
        .await
    }

    /// Get quota usage for a mailbox
    pub async fn find_by_mailbox(
        pool: &PgPool,
        mailbox_id: Uuid,
    ) -> Result<Option<QuotaUsage>, sqlx::Error> {
        sqlx::query_as::<_, QuotaUsage>(
            "SELECT * FROM quota_usage WHERE mailbox_id = $1"
        )
        .bind(mailbox_id)
        .fetch_optional(pool)
        .await
    }

    /// Build a QuotaStatus combining mailbox limits with current usage
    pub fn to_status(
        usage: Option<&QuotaUsage>,
        quota_bytes: i64,
        quota_warn_percent: i32,
        mailbox_id: Uuid,
    ) -> QuotaStatus {
        let (used_bytes, message_count, last_synced_at) = match usage {
            Some(u) => (u.used_bytes, u.message_count, Some(u.last_synced_at)),
            None => (0, 0, None),
        };

        let usage_percent = if quota_bytes > 0 {
            (used_bytes as f64 / quota_bytes as f64) * 100.0
        } else {
            0.0
        };

        QuotaStatus {
            mailbox_id,
            quota_bytes,
            used_bytes,
            message_count,
            usage_percent,
            quota_warn_percent,
            is_over_quota: used_bytes >= quota_bytes && quota_bytes > 0,
            is_warning: usage_percent >= quota_warn_percent as f64,
            last_synced_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_status_under_limit() {
        let mid = Uuid::new_v4();
        let usage = QuotaUsage {
            id: Uuid::new_v4(),
            mailbox_id: mid,
            used_bytes: 500_000_000,  // 500MB
            message_count: 1000,
            last_synced_at: chrono::Utc::now(),
        };

        let status = QuotaUsage::to_status(
            Some(&usage),
            1_073_741_824, // 1GB
            80,
            mid,
        );

        assert!(!status.is_over_quota);
        assert!(!status.is_warning);
        assert!(status.usage_percent < 50.0);
        assert_eq!(status.message_count, 1000);
    }

    #[test]
    fn test_quota_status_warning_threshold() {
        let mid = Uuid::new_v4();
        let usage = QuotaUsage {
            id: Uuid::new_v4(),
            mailbox_id: mid,
            used_bytes: 900_000_000,  // ~838MB of 1GB = ~84%
            message_count: 2000,
            last_synced_at: chrono::Utc::now(),
        };

        let status = QuotaUsage::to_status(
            Some(&usage),
            1_073_741_824, // 1GB
            80,
            mid,
        );

        assert!(!status.is_over_quota);
        assert!(status.is_warning);
        assert!(status.usage_percent > 80.0);
    }

    #[test]
    fn test_quota_status_over_quota() {
        let mid = Uuid::new_v4();
        let usage = QuotaUsage {
            id: Uuid::new_v4(),
            mailbox_id: mid,
            used_bytes: 1_200_000_000,
            message_count: 5000,
            last_synced_at: chrono::Utc::now(),
        };

        let status = QuotaUsage::to_status(
            Some(&usage),
            1_073_741_824, // 1GB
            80,
            mid,
        );

        assert!(status.is_over_quota);
        assert!(status.is_warning);
        assert!(status.usage_percent > 100.0);
    }

    #[test]
    fn test_quota_status_no_usage_data() {
        let mid = Uuid::new_v4();
        let status = QuotaUsage::to_status(None, 1_073_741_824, 80, mid);

        assert!(!status.is_over_quota);
        assert!(!status.is_warning);
        assert_eq!(status.used_bytes, 0);
        assert_eq!(status.message_count, 0);
        assert!(status.last_synced_at.is_none());
    }

    #[test]
    fn test_quota_status_zero_quota() {
        let mid = Uuid::new_v4();
        let usage = QuotaUsage {
            id: Uuid::new_v4(),
            mailbox_id: mid,
            used_bytes: 100,
            message_count: 1,
            last_synced_at: chrono::Utc::now(),
        };

        let status = QuotaUsage::to_status(Some(&usage), 0, 80, mid);

        assert!(!status.is_over_quota); // zero quota means unlimited
        assert_eq!(status.usage_percent, 0.0);
    }

    #[test]
    fn test_quota_status_exactly_at_limit() {
        let mid = Uuid::new_v4();
        let usage = QuotaUsage {
            id: Uuid::new_v4(),
            mailbox_id: mid,
            used_bytes: 1_073_741_824,
            message_count: 3000,
            last_synced_at: chrono::Utc::now(),
        };

        let status = QuotaUsage::to_status(
            Some(&usage),
            1_073_741_824,
            80,
            mid,
        );

        assert!(status.is_over_quota);
        assert!(status.is_warning);
        assert!((status.usage_percent - 100.0).abs() < 0.01);
    }
}
