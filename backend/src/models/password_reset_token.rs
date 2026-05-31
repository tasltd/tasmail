// Added (TMAIL-375): SQLx model for the `password_reset_tokens` table.
//
// Drives the /classic/password-reset/{request,confirm} flow — see
// `migrations/083_password_reset_tokens.sql` for the schema rationale.
//
// Convention:
//   * `create` returns BOTH the raw token (for emailing to the user) AND
//     the persisted row (which only stores SHA-256 of the token). The raw
//     value never round-trips through the DB.
//   * Lookups run on the raw pool — the user has NO session at this point
//     so RLS context can't be primed. The owner-scoped SELECT policy on
//     the table is defence in depth for any future admin endpoint.
//   * `mark_used` is the single-use guard: it atomically sets `used_at`
//     and only succeeds when the row is still pending, so two concurrent
//     submits can't both win.

use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};

/// 1-hour fixed TTL per the spec acceptance criteria for TMAIL-375. Centralised
/// here so the migration, the create path, and the validate path all agree on
/// one number.
pub const PASSWORD_RESET_TTL_SECS: i64 = 3600;

/// Length of the raw token in bytes before base64-encoding. 32 bytes → 256
/// bits of entropy, matching the per-session CSRF token guidance used by the
/// rest of the Classic UI surface.
pub const PASSWORD_RESET_TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PasswordResetToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub request_ip: Option<String>,
    pub request_ua: Option<String>,
}

/// Hash a raw reset token the same way the lookup does, so callers can
/// pre-compute the value when verifying inbound tokens from query strings.
/// SHA-256, hex-encoded (64 chars). Deterministic — same input always
/// produces the same hash so the lookup is a simple WHERE.
pub fn hash_reset_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a fresh 32-byte URL-safe random token and return its base64-encoded
/// string representation. Used by `create` and exposed for tests.
fn generate_raw_token() -> String {
    let mut bytes = [0u8; PASSWORD_RESET_TOKEN_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    B64URL.encode(bytes)
}

/// Bundle returned by `create` — the raw token (must reach the user via
/// email, never persisted anywhere else) and the database row (carries the
/// hash + audit metadata).
#[derive(Debug, Clone)]
pub struct IssuedPasswordResetToken {
    pub raw_token: String,
    pub row: PasswordResetToken,
}

impl PasswordResetToken {
    /// Insert a fresh reset token row for `user_id`. Returns the raw token
    /// (for the outbound email) alongside the persisted row.
    ///
    /// Best practice: callers should `delete_for_user` BEFORE calling this
    /// so a user re-requesting a reset doesn't accumulate pending rows. We
    /// don't do that internally because the caller (the request handler)
    /// runs `delete_for_user` regardless of whether the email resolves to
    /// a real mailbox — keeping the helper single-purpose.
    pub async fn create(
        pool: &sqlx::PgPool,
        user_id: Uuid,
        ip: Option<&str>,
        ua: Option<&str>,
    ) -> Result<IssuedPasswordResetToken, sqlx::Error> {
        let raw_token = generate_raw_token();
        let token_hash = hash_reset_token(&raw_token);
        let expires_at = Utc::now() + Duration::seconds(PASSWORD_RESET_TTL_SECS);
        let row = sqlx::query_as::<_, PasswordResetToken>(
            "INSERT INTO password_reset_tokens
                (id, user_id, token_hash, created_at, expires_at, used_at, request_ip, request_ua)
             VALUES (gen_random_uuid(), $1, $2, NOW(), $3, NULL, $4, $5)
             RETURNING *",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .bind(ip)
        .bind(ua)
        .fetch_one(pool)
        .await?;
        Ok(IssuedPasswordResetToken { raw_token, row })
    }

    /// Look up a reset token by its SHA-256 hash. Filters out used and
    /// expired rows so an attacker can't tell from response timing whether
    /// the token was ever valid — every invalid lookup returns `None`.
    pub async fn find_active_by_hash(
        pool: &sqlx::PgPool,
        token_hash: &str,
    ) -> Result<Option<PasswordResetToken>, sqlx::Error> {
        sqlx::query_as::<_, PasswordResetToken>(
            "SELECT * FROM password_reset_tokens
             WHERE token_hash = $1
               AND used_at IS NULL
               AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await
    }

    /// Atomically mark the row as used. Returns `true` if the UPDATE matched
    /// a row that was still pending (the caller can rely on this for the
    /// "is this the first successful confirm?" branch). Returns `false` if
    /// the row was already used / expired / deleted between lookup and
    /// confirm — the caller should treat that as a generic "invalid token"
    /// failure.
    pub async fn mark_used(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE password_reset_tokens
             SET used_at = NOW()
             WHERE id = $1
               AND used_at IS NULL
               AND expires_at > NOW()",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Invalidate every pending reset row for a user. Called by the
    /// request handler BEFORE inserting a fresh row so a user who clicks
    /// "Forgot password" twice only has the latest link live (closes the
    /// "older link kept working after a newer one was issued" hole).
    pub async fn delete_for_user(
        pool: &sqlx::PgPool,
        user_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM password_reset_tokens
             WHERE user_id = $1 AND used_at IS NULL",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Maintenance sweep — prune used + expired rows. Wired into the same
    /// background cleanup job that prunes `classic_sessions` and
    /// `pending_2fa_tokens` (or runs ad-hoc from an admin endpoint).
    #[allow(dead_code)]
    pub async fn cleanup_expired(pool: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM password_reset_tokens
             WHERE used_at IS NOT NULL OR expires_at < NOW()",
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_matches_spec() {
        // Lock the 1-hour TTL — if a future change drifts this we want
        // the test to scream rather than silently widen the attack window.
        assert_eq!(PASSWORD_RESET_TTL_SECS, 3600);
    }

    #[test]
    fn raw_token_is_url_safe_and_long_enough() {
        let t = generate_raw_token();
        // 32 bytes → ceil(32*4/3) = 43 chars URL-safe base64 no padding.
        assert_eq!(t.len(), 43, "expected 43-char token, got {:?}", t);
        for ch in t.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "non-URL-safe-base64 char {:?} in token {:?}",
                ch,
                t
            );
        }
    }

    #[test]
    fn raw_tokens_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1_000 {
            assert!(
                seen.insert(generate_raw_token()),
                "duplicate token within batch — RNG broken"
            );
        }
    }

    #[test]
    fn hash_is_deterministic_hex_sha256() {
        // Lock the hash function — a change here invalidates every live
        // pending row in production, so the test catches accidental drift.
        let h = hash_reset_token("hello-tasmail");
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars: {h}");
        assert!(
            h.chars().all(|c| c.is_ascii_hexdigit()),
            "hash must be hex only: {h}"
        );
        // Deterministic — same input always produces the same hash.
        assert_eq!(h, hash_reset_token("hello-tasmail"));
        // Different input produces a different hash.
        assert_ne!(h, hash_reset_token("hello-tasmail "));
    }

    #[test]
    fn hash_of_known_value_matches_external_oracle() {
        // SHA-256("test") known constant — guards against accidental hash
        // algorithm swaps (e.g. someone wires in sha-1 thinking it's fine).
        let h = hash_reset_token("test");
        assert_eq!(
            h,
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
    }
}
