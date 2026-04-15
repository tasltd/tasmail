// Added: Integration tests for GET /api/health endpoint
// NOTE: Health check queries the DB, which will fail with our dummy pool,
// so we expect a "degraded" status response (still 200 OK)

mod common;

use axum::http::{Method, StatusCode};

#[tokio::test]
async fn health_returns_200() {
    let app = common::TestApp::new().await;
    let (status, body) = app.request(Method::GET, "/api/health", None, None).await;

    assert_eq!(status, StatusCode::OK);
    // NOTE: Response should contain "status" field regardless of DB connectivity
    assert!(body.get("status").is_some(), "Response missing 'status' field: {:?}", body);
    assert!(body.get("version").is_some(), "Response missing 'version' field: {:?}", body);
    assert!(body.get("database").is_some(), "Response missing 'database' field: {:?}", body);
}

#[tokio::test]
async fn health_is_public_no_auth_needed() {
    // NOTE: Sending request without auth token — should still succeed (public route)
    let app = common::TestApp::new().await;
    let (status, _body) = app.request(Method::GET, "/api/health", None, None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn health_reports_degraded_without_db() {
    // NOTE: Our test pool points to a non-existent DB, so health should report "degraded"
    let app = common::TestApp::new().await;
    let (status, body) = app.request(Method::GET, "/api/health", None, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["database"], "disconnected");
}

#[tokio::test]
async fn health_returns_version() {
    let app = common::TestApp::new().await;
    let (status, body) = app.request(Method::GET, "/api/health", None, None).await;

    assert_eq!(status, StatusCode::OK);
    let version = body["version"].as_str().unwrap();
    // NOTE: Version comes from Cargo.toml — should be semver-ish
    assert!(!version.is_empty(), "Version should not be empty");
}
