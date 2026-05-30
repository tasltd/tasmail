// Added (TMAIL-308): Integration tests for multi-origin CORS — comma-separated
// list and `*.subdomain` wildcards.
//
// These tests bypass `create_router` and wire `build_allow_origin` straight
// into a minimal axum Router so they don't race against other tests that read
// the CORS_ORIGIN env var. Each test owns its own router with the exact
// `CORS_ORIGIN` value under test.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use http_body_util::BodyExt as _;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

use tasmail::cors::build_allow_origin;

/// Build a tiny test router with the CORS layer applied — just enough to
/// inspect `access-control-allow-origin` on real responses.
fn cors_only_router(raw_cors: &str) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(build_allow_origin(raw_cors))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .allow_credentials(true);

    Router::new()
        .route("/api/ping", get(|| async { "pong" }))
        .layer(cors)
}

async fn send_get_with_origin(router: Router, origin: &str) -> axum::http::Response<Body> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/ping")
        .header(header::ORIGIN, origin)
        .body(Body::empty())
        .unwrap();
    router.oneshot(req).await.unwrap()
}

async fn send_preflight(router: Router, origin: &str) -> axum::http::Response<Body> {
    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/ping")
        .header(header::ORIGIN, origin)
        .header("Access-Control-Request-Method", "GET")
        .body(Body::empty())
        .unwrap();
    router.oneshot(req).await.unwrap()
}

// --- single-origin (backward compatibility with TMAIL-37) ---

#[tokio::test]
async fn single_origin_still_works() {
    let router = cors_only_router("https://mail.techatscale.io");

    let resp = send_get_with_origin(router, "https://mail.techatscale.io").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("CORS header missing");
    assert_eq!(allow.to_str().unwrap(), "https://mail.techatscale.io");
}

#[tokio::test]
async fn single_origin_rejects_others() {
    let router = cors_only_router("https://mail.techatscale.io");

    let resp = send_get_with_origin(router, "https://evil.example.com").await;
    // Request still reaches the handler (CORS does NOT block server-side), but
    // the browser-enforcement header must NOT be set for a disallowed origin.
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "CORS header must be absent for disallowed origin: {:?}",
        resp.headers()
    );
}

// --- comma-separated list ---

#[tokio::test]
async fn comma_separated_allows_first_origin() {
    let router = cors_only_router(
        "https://mail.techatscale.io,https://app.techatscale.io,http://localhost:5173",
    );

    let resp = send_get_with_origin(router, "https://mail.techatscale.io").await;
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("CORS header missing for first origin");
    assert_eq!(allow.to_str().unwrap(), "https://mail.techatscale.io");
}

#[tokio::test]
async fn comma_separated_allows_middle_origin() {
    let router = cors_only_router(
        "https://mail.techatscale.io,https://app.techatscale.io,http://localhost:5173",
    );

    let resp = send_get_with_origin(router, "https://app.techatscale.io").await;
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("CORS header missing for middle origin");
    assert_eq!(allow.to_str().unwrap(), "https://app.techatscale.io");
}

#[tokio::test]
async fn comma_separated_allows_last_origin() {
    let router = cors_only_router(
        "https://mail.techatscale.io,https://app.techatscale.io,http://localhost:5173",
    );

    let resp = send_get_with_origin(router, "http://localhost:5173").await;
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("CORS header missing for last origin");
    assert_eq!(allow.to_str().unwrap(), "http://localhost:5173");
}

#[tokio::test]
async fn comma_separated_rejects_unlisted_origin() {
    let router = cors_only_router(
        "https://mail.techatscale.io,https://app.techatscale.io,http://localhost:5173",
    );

    let resp = send_get_with_origin(router, "https://evil.example.com").await;
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "CORS header must be absent for unlisted origin"
    );
}

#[tokio::test]
async fn comma_separated_handles_whitespace_around_commas() {
    let router = cors_only_router(
        "  https://a.example.com  ,  https://b.example.com  ",
    );

    let resp_a = send_get_with_origin(router.clone(), "https://a.example.com").await;
    assert!(
        resp_a
            .headers()
            .get("access-control-allow-origin")
            .is_some(),
        "whitespace-padded origin a must be accepted"
    );

    let resp_b = send_get_with_origin(router, "https://b.example.com").await;
    assert!(
        resp_b
            .headers()
            .get("access-control-allow-origin")
            .is_some(),
        "whitespace-padded origin b must be accepted"
    );
}

// --- wildcard ---

#[tokio::test]
async fn wildcard_accepts_subdomain_origin() {
    let router = cors_only_router("https://*.tenants.tasmail.io");

    let resp = send_get_with_origin(router, "https://acme.tenants.tasmail.io").await;
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("CORS header missing for wildcard subdomain");
    assert_eq!(allow.to_str().unwrap(), "https://acme.tenants.tasmail.io");
}

#[tokio::test]
async fn wildcard_rejects_bare_apex() {
    let router = cors_only_router("https://*.tenants.tasmail.io");

    // The apex itself (`tenants.tasmail.io`) must NOT be allowed by a `*.tenants.tasmail.io` pattern.
    let resp = send_get_with_origin(router, "https://tenants.tasmail.io").await;
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "wildcard must NOT accept the bare apex"
    );
}

#[tokio::test]
async fn wildcard_rejects_suffix_collision_attack() {
    let router = cors_only_router("https://*.tenants.tasmail.io");

    // `eviltenants.tasmail.io` ends with `tenants.tasmail.io` but is NOT a
    // subdomain of `tenants.tasmail.io`. This is the classic CORS suffix
    // confusion attack — must be rejected.
    let resp = send_get_with_origin(router, "https://eviltenants.tasmail.io").await;
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "wildcard must NOT accept a suffix-collision host"
    );
}

#[tokio::test]
async fn wildcard_rejects_wrong_scheme() {
    let router = cors_only_router("https://*.tenants.tasmail.io");

    let resp = send_get_with_origin(router, "http://acme.tenants.tasmail.io").await;
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "wildcard with explicit https must NOT accept http"
    );
}

#[tokio::test]
async fn mixed_exact_and_wildcard_accepts_both_styles() {
    let router = cors_only_router(
        "https://mail.techatscale.io,https://*.tenants.tasmail.io",
    );

    let resp_exact = send_get_with_origin(router.clone(), "https://mail.techatscale.io").await;
    assert!(
        resp_exact
            .headers()
            .get("access-control-allow-origin")
            .is_some(),
        "exact entry in mixed list must be accepted"
    );

    let resp_wild = send_get_with_origin(router, "https://customer-x.tenants.tasmail.io").await;
    assert!(
        resp_wild
            .headers()
            .get("access-control-allow-origin")
            .is_some(),
        "wildcard entry in mixed list must be accepted"
    );
}

// --- preflight ---

#[tokio::test]
async fn preflight_succeeds_for_listed_origin() {
    let router = cors_only_router(
        "https://mail.techatscale.io,https://app.techatscale.io",
    );

    let resp = send_preflight(router, "https://app.techatscale.io").await;
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NO_CONTENT,
        "preflight returned unexpected status: {}",
        resp.status()
    );
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("preflight missing access-control-allow-origin");
    assert_eq!(allow.to_str().unwrap(), "https://app.techatscale.io");
}

#[tokio::test]
async fn preflight_succeeds_for_wildcard_origin() {
    let router = cors_only_router("https://*.tenants.tasmail.io");

    let resp = send_preflight(router, "https://customer-y.tenants.tasmail.io").await;
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NO_CONTENT,
        "wildcard preflight returned unexpected status: {}",
        resp.status()
    );
    let allow = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("wildcard preflight missing access-control-allow-origin");
    assert_eq!(
        allow.to_str().unwrap(),
        "https://customer-y.tenants.tasmail.io"
    );
}

// --- response body sanity check (handler still runs) ---

#[tokio::test]
async fn allowed_origin_response_body_intact() {
    let router = cors_only_router("https://mail.techatscale.io");

    let resp = send_get_with_origin(router, "https://mail.techatscale.io").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"pong");
}
