// Added: Criterion benchmarks for Axum router construction and request routing (TMAIL-38)
// PURPOSE: Measure the cost of building the full router and dispatching requests
// through the middleware stack (CORS, compression, security headers, auth).

use criterion::{criterion_group, criterion_main, Criterion};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use jsonwebtoken::{encode, EncodingKey, Header};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

use tasmail::config::{
    Config, DatabaseConfig, ImapConfig, JwtConfig, RedisConfig, ServerConfig, SmtpConfig, StorageConfig,
};
use tasmail::router::create_router;
use tasmail::services::auth_service::Claims;
use tasmail::services::cache_service::CacheService;
use tasmail::state::AppState;

// Added: Test JWT secret for router benchmarks — matches tests/common/mod.rs pattern
const BENCH_JWT_SECRET: &str = "bench-router-secret-key-do-not-use-in-prod";

// Added: Build a test config with dummy values — no real services needed
fn bench_config() -> Config {
    Config {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        database: DatabaseConfig {
            // NOTE: Lazy pool — no actual DB connection established
            url: "postgres://bench:bench@localhost:59999/nonexistent_bench_db".to_string(),
            max_connections: 1,
        },
        imap: ImapConfig {
            host: "127.0.0.1".to_string(),
            port: 993,
            tls: true,
            master_password: None,
        },
        smtp: SmtpConfig {
            host: "127.0.0.1".to_string(),
            port: 587,
            tls: true,
        },
        jwt: JwtConfig {
            secret: BENCH_JWT_SECRET.to_string(),
            access_token_expiry_secs: 900,
            refresh_token_expiry_secs: 604800,
        },
        storage: StorageConfig::default(),
        // Added: No metrics token needed for benchmarks
        metrics_token: None,
        rspamd_url: None,
        rspamd_password: None,
        billing: None,
        push: None,
        redis: RedisConfig::default(),
    }
}

// Added: Build AppState with a lazy (non-connecting) PgPool for benchmarks
fn bench_app_state() -> AppState {
    let config = bench_config();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy(&config.database.url)
        .unwrap();

    AppState {
        db: pool,
        config,
        // Added: No Prometheus metrics handle needed for bench routing tests
        metrics_handle: None,
        cache: CacheService::disabled(),
    }
}

// Added: Generate a valid JWT token for authenticated route benchmarks
fn bench_auth_token() -> String {
    let now = Utc::now();
    let exp = now + Duration::seconds(900);

    let claims = Claims {
        sub: Uuid::new_v4().to_string(),
        username: "bench@example.com".to_string(),
        is_admin: false,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(BENCH_JWT_SECRET.as_bytes()),
    )
    .unwrap()
}

// Added: Benchmark router construction — measures the cost of building the full
// Axum router with all routes, middleware layers, and state
fn bench_router_construction(c: &mut Criterion) {
    c.bench_function("router_construction", |b| {
        b.iter(|| {
            let state = bench_app_state();
            let _router = create_router(state);
        });
    });
}

// Added: Benchmark health check through the full middleware stack (public route)
// This measures: CORS layer -> compression layer -> security headers -> trace -> handler
fn bench_health_check_request(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // NOTE: Build router once outside the loop to isolate request dispatch cost
    let state = bench_app_state();
    let router = create_router(state);

    c.bench_function("health_check_full_stack", |b| {
        b.iter(|| {
            let router_clone = router.clone();
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::GET)
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap();

                let response = router_clone.oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);
            });
        });
    });
}

// Added: Benchmark 404 route miss — measures routing overhead when no route matches
fn bench_not_found_request(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let state = bench_app_state();
    let router = create_router(state);

    c.bench_function("route_not_found_404", |b| {
        b.iter(|| {
            let router_clone = router.clone();
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::GET)
                    .uri("/api/nonexistent/route/path")
                    .body(Body::empty())
                    .unwrap();

                let response = router_clone.oneshot(request).await.unwrap();
                // NOTE: Axum returns 404 for unmatched routes
                assert_eq!(response.status(), StatusCode::NOT_FOUND);
            });
        });
    });
}

// Added: Benchmark an authenticated route (auth middleware + JWT validation)
// NOTE: The handler itself will fail on DB access, but we measure the middleware cost
fn bench_authenticated_route(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let state = bench_app_state();
    let router = create_router(state);
    let token = bench_auth_token();

    c.bench_function("authenticated_route_folders", |b| {
        b.iter(|| {
            let router_clone = router.clone();
            let token_ref = &token;
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::GET)
                    .uri("/api/folders")
                    .header("authorization", format!("Bearer {}", token_ref))
                    .body(Body::empty())
                    .unwrap();

                let response = router_clone.oneshot(request).await.unwrap();
                // NOTE: Expect 500 because the lazy pool can't connect to the DB,
                // but this still exercises the full auth middleware path
                let _status = response.status();
            });
        });
    });
}

// Added: Benchmark unauthenticated request to a protected route (early rejection)
fn bench_unauthenticated_rejection(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let state = bench_app_state();
    let router = create_router(state);

    c.bench_function("unauthenticated_rejection_401", |b| {
        b.iter(|| {
            let router_clone = router.clone();
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::GET)
                    .uri("/api/folders")
                    .body(Body::empty())
                    .unwrap();

                let response = router_clone.oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            });
        });
    });
}

// Added: Benchmark reading the full response body from health check
// to measure serialization + compression overhead
fn bench_health_response_body(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let state = bench_app_state();
    let router = create_router(state);

    c.bench_function("health_check_read_body", |b| {
        b.iter(|| {
            let router_clone = router.clone();
            rt.block_on(async {
                let request = Request::builder()
                    .method(Method::GET)
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap();

                let response = router_clone.oneshot(request).await.unwrap();
                let _body = response.into_body().collect().await.unwrap().to_bytes();
            });
        });
    });
}

criterion_group!(
    benches,
    bench_router_construction,
    bench_health_check_request,
    bench_not_found_request,
    bench_authenticated_route,
    bench_unauthenticated_rejection,
    bench_health_response_body,
);
criterion_main!(benches);
