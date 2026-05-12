// TMAIL-165: cached feature-flag lookups.
// Wraps FeatureFlag::is_enabled with a 60s Redis cache so hot-path callers
// (signup, login render, onboarding wizard) don't pay a round-trip per request.
//
// Fail-closed: any cache or DB error returns `false` — features stay off until the
// data layer recovers.

use crate::state::AppState;

const CACHE_PREFIX: &str = "tasmail:flag";
const CACHE_TTL_SECS: u64 = 60;

/// Returns true iff the named flag exists AND is enabled. Cached.
pub async fn is_enabled(state: &AppState, key: &str) -> bool {
    let cache_key = format!("{}:{}", CACHE_PREFIX, key);
    if let Some(v) = state.cache.get_typed::<bool>(&cache_key).await {
        return v;
    }
    let row = match crate::models::feature_flag::FeatureFlag::find(&state.db, key).await {
        Ok(r) => r,
        Err(_) => return false,
    };
    let enabled = row.map(|f| f.enabled).unwrap_or(false);
    let _ = state.cache.set_typed(&cache_key, &enabled, CACHE_TTL_SECS).await;
    enabled
}

/// Drop the cached value for a flag. Called from admin PATCH so the new value is
/// immediately visible to the rest of the fleet.
pub async fn invalidate(state: &AppState, key: &str) {
    let cache_key = format!("{}:{}", CACHE_PREFIX, key);
    let _ = state.cache.del_typed(&cache_key).await;
}
