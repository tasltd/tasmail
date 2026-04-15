// Added: Cache management handlers for admin monitoring and control
// PURPOSE: Provides endpoints to check cache stats, connectivity, and flush cache
// CONSTRAINTS: All endpoints require admin auth via auth_middleware

use axum::{extract::State, Json};
use serde::Serialize;

use crate::error::AppError;
use crate::state::AppState;

/// Added: Cache status response for monitoring
#[derive(Serialize)]
pub struct CacheStatus {
    pub connected: bool,
    pub redis_url: String,
    pub branding_ttl_secs: u64,
    pub quota_ttl_secs: u64,
    pub session_ttl_secs: u64,
    pub rate_limit_window_secs: u64,
    pub rate_limit_max_requests: u64,
}

/// GET /api/admin/cache/status — Check cache connection and config
pub async fn get_cache_status(
    State(state): State<AppState>,
) -> Result<Json<CacheStatus>, AppError> {
    let connected = state.cache.is_connected().await;
    let config = &state.config.redis;

    Ok(Json(CacheStatus {
        connected,
        // NOTE: Mask credentials in URL for security
        redis_url: mask_redis_url(&config.url),
        branding_ttl_secs: config.branding_ttl_secs,
        quota_ttl_secs: config.quota_ttl_secs,
        session_ttl_secs: config.session_ttl_secs,
        rate_limit_window_secs: config.rate_limit_window_secs,
        rate_limit_max_requests: config.rate_limit_max_requests,
    }))
}

/// POST /api/admin/cache/flush — Flush all TASMail cache keys
pub async fn flush_cache(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let flushed = state.cache.flush_all().await;
    Ok(Json(serde_json::json!({
        "flushed": flushed,
        "message": if flushed { "Cache flushed successfully" } else { "Cache flush failed or Redis unavailable" }
    })))
}

/// GET /api/admin/cache/stats — Get Redis server stats
pub async fn get_cache_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.cache.get_stats().await {
        Some(info) => Ok(Json(serde_json::json!({
            "connected": true,
            "info": info,
        }))),
        None => Ok(Json(serde_json::json!({
            "connected": false,
            "info": null,
        }))),
    }
}

/// Added: Mask password in Redis URL for safe display
fn mask_redis_url(url: &str) -> String {
    // NOTE: Redis URLs can contain passwords like redis://:password@host:port
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            return format!("{}:***@{}", &url[..colon_pos], &url[at_pos + 1..]);
        }
    }
    url.to_string()
}
