use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// In-memory rate limiter using a sliding window counter per IP
#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<String, WindowState>>>,
    max_requests: u32,
    window_secs: u64,
}

struct WindowState {
    count: u32,
    window_start: Instant,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    async fn check(&self, key: &str) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();

        let entry = state.entry(key.to_string()).or_insert(WindowState {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(entry.window_start).as_secs() >= self.window_secs {
            entry.count = 0;
            entry.window_start = now;
        }

        if entry.count >= self.max_requests {
            return false;
        }

        entry.count += 1;
        true
    }

    /// Periodically clean up expired entries to prevent memory leaks
    pub fn start_cleanup(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let mut state = self.state.lock().await;
                let now = Instant::now();
                state.retain(|_, v| {
                    now.duration_since(v.window_start).as_secs() < self.window_secs * 2
                });
            }
        });
    }
}

/// Axum middleware function for rate limiting
pub async fn rate_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::Extension(limiter): axum::Extension<RateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let key = addr.ip().to_string();

    if !limiter.check(&key).await {
        tracing::warn!("Rate limit exceeded for {}", key);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(3, 60);
        assert!(limiter.check("127.0.0.1").await);
        assert!(limiter.check("127.0.0.1").await);
        assert!(limiter.check("127.0.0.1").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(2, 60);
        assert!(limiter.check("10.0.0.1").await);
        assert!(limiter.check("10.0.0.1").await);
        assert!(!limiter.check("10.0.0.1").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_different_ips_independent() {
        let limiter = RateLimiter::new(1, 60);
        assert!(limiter.check("1.1.1.1").await);
        assert!(limiter.check("2.2.2.2").await);
        assert!(!limiter.check("1.1.1.1").await);
        assert!(!limiter.check("2.2.2.2").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_window_reset() {
        // Use a very short window to test reset
        let limiter = RateLimiter::new(1, 0);
        assert!(limiter.check("3.3.3.3").await);
        // Window is 0 seconds so next check should reset
        assert!(limiter.check("3.3.3.3").await);
    }
}
