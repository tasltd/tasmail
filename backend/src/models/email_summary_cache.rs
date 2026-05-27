// Added: TMAIL-103 — PostgreSQL cache for AI-generated email/thread summaries.
// PURPOSE: Avoid re-paying provider tokens for the same email body. Keyed on
// (user_id, kind, folder, uid, body_hash) so identical content returns the
// cached summary and any body change naturally invalidates it.
// CONSTRAINTS: RLS enforced at the DB level via app.current_user_id.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: One cached summary row. `kind` is 'single' or 'thread'.
/// `body_hash` is the SHA-256 hex digest of the source content; the cache
/// invalidates implicitly when the body changes.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailSummaryCache {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    pub folder: String,
    pub uid: i64,
    pub body_hash: String,
    pub summary: String,
    pub provider: String,
    pub model: String,
    pub message_count: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Compute the SHA-256 hex digest used as the cache key.
/// NOTE: Hex (not base64) so values are diffable in pg_dump output.
pub fn hash_body(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// PURPOSE: Build the canonical hash input for a thread summary from a uid set.
/// CONSTRAINTS: Sorts uids before hashing so [3,1,2] and [1,2,3] map to the
/// same cache entry — order shouldn't change the summary.
pub fn hash_thread_uids(uids: &[u32]) -> String {
    let mut sorted: Vec<u32> = uids.to_vec();
    sorted.sort_unstable();
    let joined: Vec<String> = sorted.iter().map(|u| u.to_string()).collect();
    hash_body(&joined.join(","))
}

impl EmailSummaryCache {
    /// PURPOSE: Look up a cached summary for a single email.
    /// Returns None on cache miss (which is the common path on first view).
    pub async fn find_single(
        db: &PgPool,
        user_id: Uuid,
        folder: &str,
        uid: i64,
        body_hash: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, EmailSummaryCache>(
            r#"
            SELECT id, user_id, kind, folder, uid, body_hash, summary,
                   provider, model, message_count, created_at
            FROM email_summary_cache
            WHERE user_id = $1
              AND kind = 'single'
              AND folder = $2
              AND uid = $3
              AND body_hash = $4
            "#,
        )
        .bind(user_id)
        .bind(folder)
        .bind(uid)
        .bind(body_hash)
        .fetch_optional(db)
        .await
    }

    /// PURPOSE: Look up a cached thread summary. `representative_uid` is the
    /// lowest uid in the thread; the full set is encoded in `body_hash`.
    pub async fn find_thread(
        db: &PgPool,
        user_id: Uuid,
        folder: &str,
        representative_uid: i64,
        body_hash: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, EmailSummaryCache>(
            r#"
            SELECT id, user_id, kind, folder, uid, body_hash, summary,
                   provider, model, message_count, created_at
            FROM email_summary_cache
            WHERE user_id = $1
              AND kind = 'thread'
              AND folder = $2
              AND uid = $3
              AND body_hash = $4
            "#,
        )
        .bind(user_id)
        .bind(folder)
        .bind(representative_uid)
        .bind(body_hash)
        .fetch_optional(db)
        .await
    }

    /// PURPOSE: Insert (or no-op on conflict) a cache row.
    /// CONSTRAINTS: ON CONFLICT DO NOTHING keeps the first-write winning so
    /// two concurrent summarize calls don't fight each other.
    pub async fn upsert(
        db: &PgPool,
        user_id: Uuid,
        kind: &str,
        folder: &str,
        uid: i64,
        body_hash: &str,
        summary: &str,
        provider: &str,
        model: &str,
        message_count: Option<i32>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO email_summary_cache
                (user_id, kind, folder, uid, body_hash, summary,
                 provider, model, message_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (user_id, kind, folder, uid, body_hash) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(kind)
        .bind(folder)
        .bind(uid)
        .bind(body_hash)
        .bind(summary)
        .bind(provider)
        .bind(model)
        .bind(message_count)
        .execute(db)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_body_is_deterministic() {
        let a = hash_body("hello world");
        let b = hash_body("hello world");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // SHA-256 hex is 64 chars
    }

    #[test]
    fn hash_body_differs_on_change() {
        let a = hash_body("draft v1");
        let b = hash_body("draft v2");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_body_empty_string_is_stable() {
        // Empty bodies shouldn't crash the hasher.
        let a = hash_body("");
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn hash_thread_uids_is_order_independent() {
        let a = hash_thread_uids(&[3, 1, 2]);
        let b = hash_thread_uids(&[1, 2, 3]);
        let c = hash_thread_uids(&[2, 3, 1]);
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn hash_thread_uids_changes_with_set() {
        let a = hash_thread_uids(&[1, 2, 3]);
        let b = hash_thread_uids(&[1, 2, 3, 4]);
        assert_ne!(a, b);
    }

    #[test]
    fn hash_thread_uids_single_element_matches_csv() {
        // Sanity: single-element list hashes the bare uid string.
        let a = hash_thread_uids(&[42]);
        let b = hash_body("42");
        assert_eq!(a, b);
    }
}
