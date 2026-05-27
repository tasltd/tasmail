// Added: Redis cache service for backend performance optimization
// PURPOSE: Provides typed caching for branding, quota, rate limiting, and session data
// EXTERNAL: Connects to Redis via redis crate with connection manager for pooling
// NOTE: All methods gracefully degrade — cache misses fall through to PostgreSQL/IMAP

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::RedisConfig;

/// Added: Cache key prefixes to namespace and prevent collisions
const PREFIX_BRANDING: &str = "tasmail:branding";
const PREFIX_QUOTA: &str = "tasmail:quota";
const PREFIX_RATE_LIMIT: &str = "tasmail:rl";
// Added (TMAIL-102): Per-user AI inference rate limit — 10 requests / 60s.
// Separate namespace from the global rate limit so that ordinary API traffic
// can't starve the AI quota and vice-versa.
const PREFIX_AI_RATE_LIMIT: &str = "tasmail:rl:ai";
const AI_RATE_LIMIT_WINDOW_SECS: i64 = 60;
const AI_RATE_LIMIT_MAX_REQUESTS: u64 = 10;
const PREFIX_SESSION: &str = "tasmail:session";
const PREFIX_BLACKLIST: &str = "tasmail:jwt_blacklist";
// Added (TMAIL-162): per-user IMAP/SMTP config caches.
// Cuts the DB+decrypt round trip on every page load that hits a BYOK route.
const PREFIX_IMAP_CFG: &str = "tasmail:imap_cfg";
const PREFIX_SMTP_CFG: &str = "tasmail:smtp_cfg";
// 5 minute TTL for per-user mail-server config — same convention as branding.
// Worst-case staleness window after a user updates their server credentials.
const TTL_USER_CFG_SECS: u64 = 300;

/// Added: Redis-backed cache service with graceful degradation
/// PURPOSE: Wraps Redis connection manager and provides typed get/set with TTL
/// NOTE: If Redis is unavailable, all operations return None/Ok — never block the request
#[derive(Clone)]
pub struct CacheService {
    conn: Arc<RwLock<Option<ConnectionManager>>>,
    config: RedisConfig,
}

impl CacheService {
    /// Connect to Redis. If connection fails, cache operates in passthrough mode.
    pub async fn new(config: &RedisConfig) -> Self {
        let conn = match redis::Client::open(config.url.as_str()) {
            Ok(client) => match ConnectionManager::new(client).await {
                Ok(mgr) => {
                    tracing::info!("Redis cache connected at {}", config.url);
                    Some(mgr)
                }
                Err(e) => {
                    tracing::warn!("Redis connection failed, caching disabled: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("Redis client creation failed, caching disabled: {}", e);
                None
            }
        };

        Self {
            conn: Arc::new(RwLock::new(conn)),
            config: config.clone(),
        }
    }

    /// Added: Create a CacheService in disabled/passthrough mode (for testing without Redis)
    pub fn disabled() -> Self {
        Self {
            conn: Arc::new(RwLock::new(None)),
            config: RedisConfig::default(),
        }
    }

    /// Added: Check if Redis connection is active
    pub async fn is_connected(&self) -> bool {
        let guard = self.conn.read().await;
        guard.is_some()
    }

    // --- Generic typed operations ---

    /// Added: Get a JSON-serialized value from cache
    async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let guard = self.conn.read().await;
        let conn = guard.as_ref()?;
        let mut conn = conn.clone();

        match conn.get::<_, Option<String>>(key).await {
            Ok(Some(data)) => serde_json::from_str(&data).ok(),
            _ => None,
        }
    }

    /// Added: Set a JSON-serialized value with TTL
    async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) -> bool {
        let guard = self.conn.read().await;
        let Some(conn) = guard.as_ref() else { return false };
        let mut conn = conn.clone();

        let json = match serde_json::to_string(value) {
            Ok(j) => j,
            Err(_) => return false,
        };

        conn.set_ex::<_, _, ()>(key, json, ttl_secs).await.is_ok()
    }

    /// Added: Delete a key from cache
    async fn del(&self, key: &str) -> bool {
        let guard = self.conn.read().await;
        let Some(conn) = guard.as_ref() else { return false };
        let mut conn = conn.clone();

        conn.del::<_, ()>(key).await.is_ok()
    }

    // --- Branding cache ---

    /// Added: Get cached branding config (rarely changes, cached for 5 min)
    pub async fn get_branding<T: DeserializeOwned>(&self) -> Option<T> {
        self.get_json(PREFIX_BRANDING).await
    }

    /// Added: Cache branding config
    pub async fn set_branding<T: Serialize>(&self, branding: &T) -> bool {
        self.set_json(PREFIX_BRANDING, branding, self.config.branding_ttl_secs).await
    }

    /// Added: Invalidate branding cache (called on admin update/reset)
    pub async fn invalidate_branding(&self) -> bool {
        self.del(PREFIX_BRANDING).await
    }

    // --- Quota cache ---

    /// Added: Get cached quota for a mailbox
    pub async fn get_quota<T: DeserializeOwned>(&self, mailbox_id: &str) -> Option<T> {
        let key = format!("{}:{}", PREFIX_QUOTA, mailbox_id);
        self.get_json(&key).await
    }

    /// Added: Cache quota status for a mailbox
    pub async fn set_quota<T: Serialize>(&self, mailbox_id: &str, quota: &T) -> bool {
        let key = format!("{}:{}", PREFIX_QUOTA, mailbox_id);
        self.set_json(&key, quota, self.config.quota_ttl_secs).await
    }

    /// Added: Invalidate quota cache for a mailbox (called after sync)
    pub async fn invalidate_quota(&self, mailbox_id: &str) -> bool {
        let key = format!("{}:{}", PREFIX_QUOTA, mailbox_id);
        self.del(&key).await
    }

    // --- Per-user IMAP / SMTP config (BYOK) cache (TMAIL-162) ---

    /// Get the cached default IMAP config for a mailbox. Returns None if absent or Redis is down.
    pub async fn get_user_imap_config<T: DeserializeOwned>(&self, mailbox_id: &str) -> Option<T> {
        let key = format!("{}:{}", PREFIX_IMAP_CFG, mailbox_id);
        self.get_json(&key).await
    }

    /// Cache the default IMAP config for a mailbox.
    /// CONSTRAINTS: Caller must NOT cache the plaintext password — pass the encrypted ciphertext only.
    pub async fn set_user_imap_config<T: Serialize>(&self, mailbox_id: &str, cfg: &T) -> bool {
        let key = format!("{}:{}", PREFIX_IMAP_CFG, mailbox_id);
        self.set_json(&key, cfg, TTL_USER_CFG_SECS).await
    }

    /// Drop the cached IMAP config row. Called from POST/DELETE /api/imap-configs handlers.
    pub async fn invalidate_user_imap_config(&self, mailbox_id: &str) -> bool {
        let key = format!("{}:{}", PREFIX_IMAP_CFG, mailbox_id);
        self.del(&key).await
    }

    pub async fn get_user_smtp_config<T: DeserializeOwned>(&self, mailbox_id: &str) -> Option<T> {
        let key = format!("{}:{}", PREFIX_SMTP_CFG, mailbox_id);
        self.get_json(&key).await
    }

    pub async fn set_user_smtp_config<T: Serialize>(&self, mailbox_id: &str, cfg: &T) -> bool {
        let key = format!("{}:{}", PREFIX_SMTP_CFG, mailbox_id);
        self.set_json(&key, cfg, TTL_USER_CFG_SECS).await
    }

    pub async fn invalidate_user_smtp_config(&self, mailbox_id: &str) -> bool {
        let key = format!("{}:{}", PREFIX_SMTP_CFG, mailbox_id);
        self.del(&key).await
    }

    // --- Generic typed get/set/del (TMAIL-165 feature flags + future use cases) ---

    /// Get any JSON-serialisable value at the literal key (no prefix munging).
    /// Use this for ad-hoc caches that already include their own namespace prefix.
    pub async fn get_typed<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get_json(key).await
    }

    /// Set any JSON-serialisable value at the literal key with the given TTL.
    pub async fn set_typed<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) -> bool {
        self.set_json(key, value, ttl_secs).await
    }

    /// Delete any literal key.
    pub async fn del_typed(&self, key: &str) -> bool {
        self.del(key).await
    }

    // --- Rate limiting ---

    /// Added: Check and increment rate limit counter. Returns true if within limit.
    /// Uses Redis INCR + EXPIRE for atomic sliding window counter.
    pub async fn check_rate_limit(&self, client_ip: &str) -> bool {
        let guard = self.conn.read().await;
        let Some(conn) = guard.as_ref() else {
            // NOTE: If Redis is down, allow the request (fail open for availability)
            return true;
        };
        let mut conn = conn.clone();

        let key = format!("{}:{}", PREFIX_RATE_LIMIT, client_ip);
        let window = self.config.rate_limit_window_secs;
        let max = self.config.rate_limit_max_requests;

        // Atomic increment + set expiry on first request in window
        let count: u64 = match redis::pipe()
            .atomic()
            .incr(&key, 1u64)
            .expire(&key, window as i64)
            .query_async::<Vec<u64>>(&mut conn)
            .await
        {
            Ok(results) if !results.is_empty() => results[0],
            _ => return true, // NOTE: Fail open on Redis error
        };

        count <= max
    }

    /// Added (TMAIL-102): Per-user AI inference rate limit (max 10 requests / 60s).
    /// PURPOSE: Keep local Ollama (and other AI providers) from being DoS'd by a
    /// single tenant or an SPA bug. Separate from the global IP rate limit so
    /// ordinary mail traffic doesn't burn the AI quota.
    /// EXTERNAL: Uses Redis INCR + EXPIRE atomically — fail-open if Redis is down.
    /// CONSTRAINTS: 10 req / 60s window, per `user_id`.
    pub async fn check_ai_rate_limit(&self, user_id: &str) -> bool {
        let guard = self.conn.read().await;
        let Some(conn) = guard.as_ref() else {
            // NOTE: Fail open when Redis is down — AI calls are still expensive
            // upstream, but availability beats strictness for a soft quota.
            return true;
        };
        let mut conn = conn.clone();

        let key = format!("{}:{}", PREFIX_AI_RATE_LIMIT, user_id);

        let count: u64 = match redis::pipe()
            .atomic()
            .incr(&key, 1u64)
            .expire(&key, AI_RATE_LIMIT_WINDOW_SECS)
            .query_async::<Vec<u64>>(&mut conn)
            .await
        {
            Ok(results) if !results.is_empty() => results[0],
            _ => return true, // NOTE: Fail open on Redis error
        };

        count <= AI_RATE_LIMIT_MAX_REQUESTS
    }

    /// Added (TMAIL-102): Remaining AI requests in the current 60s window.
    /// Returns `None` only when Redis is unreachable so the caller can decide
    /// whether to surface "unknown" or fall back to the configured max.
    pub async fn get_ai_rate_limit_remaining(&self, user_id: &str) -> Option<u64> {
        let guard = self.conn.read().await;
        let conn = guard.as_ref()?;
        let mut conn = conn.clone();

        let key = format!("{}:{}", PREFIX_AI_RATE_LIMIT, user_id);
        let count: Option<u64> = conn.get(&key).await.ok();
        Some(AI_RATE_LIMIT_MAX_REQUESTS.saturating_sub(count.unwrap_or(0)))
    }

    /// Added (TMAIL-102): Constant accessor for the AI rate-limit ceiling, exposed
    /// so handlers can include "max 10/min" in 429 messages without hard-coding.
    pub fn ai_rate_limit_max() -> u64 {
        AI_RATE_LIMIT_MAX_REQUESTS
    }

    /// Added: Get remaining rate limit requests for a client
    pub async fn get_rate_limit_remaining(&self, client_ip: &str) -> Option<u64> {
        let guard = self.conn.read().await;
        let conn = guard.as_ref()?;
        let mut conn = conn.clone();

        let key = format!("{}:{}", PREFIX_RATE_LIMIT, client_ip);
        let count: Option<u64> = conn.get(&key).await.ok();
        let max = self.config.rate_limit_max_requests;

        Some(max.saturating_sub(count.unwrap_or(0)))
    }

    // --- Session metadata cache ---

    /// Added: Cache user session metadata (avoid repeated DB queries for user info)
    pub async fn get_session<T: DeserializeOwned>(&self, user_id: &str) -> Option<T> {
        let key = format!("{}:{}", PREFIX_SESSION, user_id);
        self.get_json(&key).await
    }

    pub async fn set_session<T: Serialize>(&self, user_id: &str, data: &T) -> bool {
        let key = format!("{}:{}", PREFIX_SESSION, user_id);
        self.set_json(&key, data, self.config.session_ttl_secs).await
    }

    /// Added: Invalidate session cache on logout
    pub async fn invalidate_session(&self, user_id: &str) -> bool {
        let key = format!("{}:{}", PREFIX_SESSION, user_id);
        self.del(&key).await
    }

    // --- JWT blacklist (for immediate token revocation) ---

    /// Added: Blacklist a JWT token (on logout, the access token is blocked until natural expiry)
    pub async fn blacklist_token(&self, token_hash: &str, ttl_secs: u64) -> bool {
        let guard = self.conn.read().await;
        let Some(conn) = guard.as_ref() else { return false };
        let mut conn = conn.clone();

        let key = format!("{}:{}", PREFIX_BLACKLIST, token_hash);
        conn.set_ex::<_, _, ()>(&key, "1", ttl_secs).await.is_ok()
    }

    /// Added: Check if a JWT token has been blacklisted
    pub async fn is_token_blacklisted(&self, token_hash: &str) -> bool {
        let guard = self.conn.read().await;
        let Some(conn) = guard.as_ref() else { return false };
        let mut conn = conn.clone();

        let key = format!("{}:{}", PREFIX_BLACKLIST, token_hash);
        conn.exists::<_, bool>(&key).await.unwrap_or(false)
    }

    // --- Cache stats (for monitoring) ---

    /// Added: Get Redis INFO stats for monitoring dashboards
    pub async fn get_stats(&self) -> Option<String> {
        let guard = self.conn.read().await;
        let conn = guard.as_ref()?;
        let mut conn = conn.clone();

        redis::cmd("INFO")
            .arg("stats")
            .query_async::<String>(&mut conn)
            .await
            .ok()
    }

    /// Added: Flush all TASMail cache keys (admin operation)
    pub async fn flush_all(&self) -> bool {
        let guard = self.conn.read().await;
        let Some(conn) = guard.as_ref() else { return false };
        let mut conn = conn.clone();

        // NOTE: Only flush keys with our prefix, not the entire Redis instance
        let patterns = [
            format!("{}*", PREFIX_BRANDING),
            format!("{}:*", PREFIX_QUOTA),
            format!("{}:*", PREFIX_RATE_LIMIT),
            format!("{}:*", PREFIX_SESSION),
            format!("{}:*", PREFIX_BLACKLIST),
        ];

        for pattern in &patterns {
            let keys: Vec<String> = match conn.keys(pattern).await {
                Ok(k) => k,
                Err(_) => continue,
            };
            for key in keys {
                let _ = conn.del::<_, ()>(&key).await;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_service_disabled_returns_none() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let cache = CacheService::disabled();
            assert!(!cache.is_connected().await);

            // All reads return None
            let result: Option<String> = cache.get_branding().await;
            assert!(result.is_none());

            let result: Option<String> = cache.get_quota("test-id").await;
            assert!(result.is_none());

            let result: Option<String> = cache.get_session("test-id").await;
            assert!(result.is_none());

            // Rate limit allows when disabled (fail open)
            assert!(cache.check_rate_limit("127.0.0.1").await);

            // TMAIL-102: AI rate limit also fails open when Redis is unavailable —
            // otherwise a Redis outage would 429 every AI request.
            assert!(cache.check_ai_rate_limit("user-uuid").await);
            // get_ai_rate_limit_remaining returns None when Redis is down so the
            // caller can decide what to surface.
            assert!(cache.get_ai_rate_limit_remaining("user-uuid").await.is_none());

            // Blacklist check returns false when disabled
            assert!(!cache.is_token_blacklisted("some-hash").await);
        });
    }

    // Added (TMAIL-102): Lock the AI rate-limit ceiling at 10 — matches the
    // TMAIL-102 spec ("max 10 AI requests per user per minute"). If this number
    // changes, the issue acceptance criteria must be revisited.
    #[test]
    fn test_ai_rate_limit_ceiling_matches_spec() {
        assert_eq!(CacheService::ai_rate_limit_max(), 10);
        assert_eq!(AI_RATE_LIMIT_WINDOW_SECS, 60);
    }

    // Added (TMAIL-102): The AI rate-limit namespace must be disjoint from the
    // global IP rate-limit namespace; otherwise an SPA burst on /api/folders
    // could exhaust the AI quota and vice-versa.
    #[test]
    fn test_ai_rate_limit_namespace_is_separate() {
        assert!(PREFIX_AI_RATE_LIMIT.starts_with("tasmail:"));
        assert_ne!(PREFIX_AI_RATE_LIMIT, PREFIX_RATE_LIMIT);
        assert!(PREFIX_AI_RATE_LIMIT.starts_with(PREFIX_RATE_LIMIT));
    }

    #[test]
    fn test_cache_key_prefixes_are_namespaced() {
        assert!(PREFIX_BRANDING.starts_with("tasmail:"));
        assert!(PREFIX_QUOTA.starts_with("tasmail:"));
        assert!(PREFIX_RATE_LIMIT.starts_with("tasmail:"));
        assert!(PREFIX_SESSION.starts_with("tasmail:"));
        assert!(PREFIX_BLACKLIST.starts_with("tasmail:"));
        // TMAIL-158: per-user mail-server cache prefixes share the tasmail: namespace.
        assert!(PREFIX_IMAP_CFG.starts_with("tasmail:"));
        assert!(PREFIX_SMTP_CFG.starts_with("tasmail:"));
        // IMAP and SMTP must live in distinct keyspaces or they would collide on user_id.
        assert_ne!(PREFIX_IMAP_CFG, PREFIX_SMTP_CFG);
    }

    // TMAIL-158: 5-minute TTL chosen to match the branding cache convention.
    // If this changes, the issue rationale ("matches existing branding TTL convention")
    // must be revisited — that's why it's locked in by a test.
    #[test]
    fn test_user_cfg_ttl_matches_branding_convention() {
        assert_eq!(TTL_USER_CFG_SECS, 300);
        assert_eq!(TTL_USER_CFG_SECS, RedisConfig::default().branding_ttl_secs);
    }

    // TMAIL-158: in passthrough mode the user-config get/set/invalidate ops must
    // never panic and must signal "no cache" — the queue processor and send handler
    // rely on this to fall through to the DB.
    #[test]
    fn test_user_config_cache_passthrough_when_disabled() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let cache = CacheService::disabled();
            let mailbox_id = "11111111-1111-1111-1111-111111111111";

            // get returns None
            let imap_hit: Option<serde_json::Value> = cache.get_user_imap_config(mailbox_id).await;
            assert!(imap_hit.is_none());
            let smtp_hit: Option<serde_json::Value> = cache.get_user_smtp_config(mailbox_id).await;
            assert!(smtp_hit.is_none());

            // set returns false (no-op in passthrough) — caller must not rely on it succeeding
            let payload = serde_json::json!({"host": "smtp.example.com", "port": 587});
            assert!(!cache.set_user_imap_config(mailbox_id, &payload).await);
            assert!(!cache.set_user_smtp_config(mailbox_id, &payload).await);

            // invalidate returns false (no-op) without panicking — handlers ignore the return value
            assert!(!cache.invalidate_user_imap_config(mailbox_id).await);
            assert!(!cache.invalidate_user_smtp_config(mailbox_id).await);
        });
    }

    #[test]
    fn test_redis_config_defaults() {
        let config = RedisConfig::default();
        assert_eq!(config.url, "redis://127.0.0.1:6379");
        assert_eq!(config.branding_ttl_secs, 300);
        assert_eq!(config.quota_ttl_secs, 60);
        assert_eq!(config.session_ttl_secs, 900);
        assert_eq!(config.rate_limit_window_secs, 60);
        assert_eq!(config.rate_limit_max_requests, 100);
    }
}
