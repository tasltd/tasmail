// Added (TMAIL-314): integration coverage for the /metrics access-control
// gate. The handler is wired into the public router but must reject
// anonymous public scrapers. These tests drive the live router via
// `tower::ServiceExt::oneshot` so we exercise the same routing + extractor
// path that a real HTTP request takes.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

mod common;
use common::TestApp;

/// Without ConnectInfo (tower oneshot) and without an X-Forwarded-For
/// header, the handler can't resolve a client IP, so it must fall through
/// to the fail-closed branch — even though the test config leaves the
/// allowlist unset (default = loopback-only).
#[tokio::test]
async fn metrics_rejects_request_with_no_client_ip() {
    let app = TestApp::new().await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();

    let res = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "fail-closed: no IP and no token must return 403"
    );

    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"Forbidden");
}

/// A public-IP scraper (e.g. someone on the open internet) gets 403
/// because the default allowlist is loopback-only and there's no token
/// configured. This is the regression test for the original gap.
#[tokio::test]
async fn metrics_rejects_public_ip_with_no_token() {
    let app = TestApp::new().await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header("x-forwarded-for", "8.8.8.8")
        .body(Body::empty())
        .unwrap();

    let res = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "public-internet IP must not be able to scrape /metrics"
    );
}

/// Loopback (127.0.0.1) is in the default allowlist, so this request
/// passes the auth gate. The router returns 503 because the test fixture
/// doesn't install a Prometheus recorder — that 503 is from the inner
/// `metrics_handle.is_none()` branch and proves we got past the gate
/// instead of bailing at 403.
#[tokio::test]
async fn metrics_allows_loopback_by_default() {
    let app = TestApp::new().await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header("x-forwarded-for", "127.0.0.1")
        .body(Body::empty())
        .unwrap();

    let res = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "loopback passes auth; 503 is from the missing PrometheusHandle in test"
    );

    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"Metrics not available");
}

/// A valid Bearer token on a public IP unlocks the endpoint. Reuses the
/// same TestApp fixture but overrides the metrics_token in the cloned
/// config and rebuilds the router for this single test.
#[tokio::test]
async fn metrics_allows_valid_bearer_token_from_any_ip() {
    use sqlx::postgres::PgPoolOptions;
    use std::sync::{Arc, OnceLock};
    use tasmail::router::create_router;
    use tasmail::services::cache_service::CacheService;
    use tasmail::services::encryption::EncryptionService;
    use tasmail::services::queue_heartbeat::QueueHeartbeat;
    use tasmail::state::AppState;

    let mut cfg = common::test_config();
    cfg.metrics_token = Some("scrape-secret-123".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy(&cfg.database.url)
        .unwrap();

    let inner_router_holder: Arc<OnceLock<axum::Router>> = Arc::new(OnceLock::new());
    let state = AppState {
        db: pool,
        config: cfg,
        metrics_handle: None,
        cache: CacheService::disabled(),
        encryption: EncryptionService::from_jwt_secret(common::TEST_JWT_SECRET),
        inner_router: inner_router_holder.clone(),
        queue_heartbeat: QueueHeartbeat::new(),
    };
    let router = create_router(state);
    let _ = inner_router_holder.set(router.clone());

    let req = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header("x-forwarded-for", "8.8.8.8")
        .header(header::AUTHORIZATION, "Bearer scrape-secret-123")
        .body(Body::empty())
        .unwrap();

    let res = router.oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "valid token from public IP must pass auth (503 only because no recorder in tests)"
    );
}

/// A wrong Bearer token from a public IP is still rejected.
#[tokio::test]
async fn metrics_rejects_wrong_bearer_token() {
    use sqlx::postgres::PgPoolOptions;
    use std::sync::{Arc, OnceLock};
    use tasmail::router::create_router;
    use tasmail::services::cache_service::CacheService;
    use tasmail::services::encryption::EncryptionService;
    use tasmail::services::queue_heartbeat::QueueHeartbeat;
    use tasmail::state::AppState;

    let mut cfg = common::test_config();
    cfg.metrics_token = Some("scrape-secret-123".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy(&cfg.database.url)
        .unwrap();

    let inner_router_holder: Arc<OnceLock<axum::Router>> = Arc::new(OnceLock::new());
    let state = AppState {
        db: pool,
        config: cfg,
        metrics_handle: None,
        cache: CacheService::disabled(),
        encryption: EncryptionService::from_jwt_secret(common::TEST_JWT_SECRET),
        inner_router: inner_router_holder.clone(),
        queue_heartbeat: QueueHeartbeat::new(),
    };
    let router = create_router(state);
    let _ = inner_router_holder.set(router.clone());

    let req = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header("x-forwarded-for", "8.8.8.8")
        .header(header::AUTHORIZATION, "Bearer wrong-token")
        .body(Body::empty())
        .unwrap();

    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// An explicit allowlist that names a non-loopback Prometheus scraper
/// IP must let that IP through and still reject everyone else.
#[tokio::test]
async fn metrics_allows_explicitly_listed_ip() {
    use sqlx::postgres::PgPoolOptions;
    use std::sync::{Arc, OnceLock};
    use tasmail::router::create_router;
    use tasmail::services::cache_service::CacheService;
    use tasmail::services::encryption::EncryptionService;
    use tasmail::services::queue_heartbeat::QueueHeartbeat;
    use tasmail::state::AppState;

    let mut cfg = common::test_config();
    cfg.metrics_allowed_ips = Some("10.0.0.5".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy(&cfg.database.url)
        .unwrap();

    let inner_router_holder: Arc<OnceLock<axum::Router>> = Arc::new(OnceLock::new());
    let state = AppState {
        db: pool,
        config: cfg,
        metrics_handle: None,
        cache: CacheService::disabled(),
        encryption: EncryptionService::from_jwt_secret(common::TEST_JWT_SECRET),
        inner_router: inner_router_holder.clone(),
        queue_heartbeat: QueueHeartbeat::new(),
    };
    let router = create_router(state);
    let _ = inner_router_holder.set(router.clone());

    // 10.0.0.5 (listed) → passes the gate (503 from missing recorder).
    let req_listed = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header("x-forwarded-for", "10.0.0.5")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req_listed).await.unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);

    // 8.8.8.8 (not listed) → 403.
    let req_other = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header("x-forwarded-for", "8.8.8.8")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req_other).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // When the operator sets an explicit list, loopback is NOT auto-added.
    let req_loopback = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header("x-forwarded-for", "127.0.0.1")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req_loopback).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "explicit allowlist replaces the loopback default — operators must opt loopback back in"
    );
}
