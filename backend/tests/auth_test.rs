// Added: Integration tests for authentication endpoints and auth middleware behavior
// NOTE: These tests verify HTTP-layer behavior (status codes, headers, JSON structure)
// without requiring a running database or mail servers

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;

// --- POST /api/auth/login ---

#[tokio::test]
async fn login_with_missing_body_returns_client_error() {
    // NOTE: No body and no content-type header — Axum returns 415 (Unsupported Media Type)
    // because the Json extractor requires application/json content type
    let app = common::TestApp::new().await;
    let (status, _body) = app.request(Method::POST, "/api/auth/login", None, None).await;

    // Added: Empty body without content-type triggers 415; with content-type triggers 422
    assert!(
        status == StatusCode::UNSUPPORTED_MEDIA_TYPE || status == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 415 or 422, got {}",
        status
    );
}

#[tokio::test]
async fn login_with_missing_fields_returns_422() {
    let app = common::TestApp::new().await;

    // NOTE: Missing "password" field
    let (status, _body) = app
        .request(
            Method::POST,
            "/api/auth/login",
            Some(json!({"username": "test@example.com"})),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn login_with_missing_username_returns_422() {
    let app = common::TestApp::new().await;

    let (status, _body) = app
        .request(
            Method::POST,
            "/api/auth/login",
            Some(json!({"password": "secret"})),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn login_with_empty_json_returns_422() {
    let app = common::TestApp::new().await;

    let (status, _body) = app
        .request(Method::POST, "/api/auth/login", Some(json!({})), None)
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn login_with_valid_body_fails_at_db_layer() {
    // NOTE: Valid JSON body but DB is unreachable — handler will attempt DB query and fail
    // Expected: 500 Internal Server Error (DB connection failure)
    let app = common::TestApp::new().await;

    let (status, _body) = app
        .request(
            Method::POST,
            "/api/auth/login",
            Some(json!({"username": "test@example.com", "password": "password123"})),
            None,
        )
        .await;

    // Added: DB query fails, but we verify the request passed validation and routing
    assert!(
        status == StatusCode::INTERNAL_SERVER_ERROR || status == StatusCode::UNAUTHORIZED,
        "Expected 500 or 401, got {}",
        status
    );
}

// --- POST /api/auth/refresh ---

#[tokio::test]
async fn refresh_with_missing_body_returns_client_error() {
    // NOTE: No body/content-type — same behavior as login: 415 or 422
    let app = common::TestApp::new().await;
    let (status, _body) = app
        .request(Method::POST, "/api/auth/refresh", None, None)
        .await;

    assert!(
        status == StatusCode::UNSUPPORTED_MEDIA_TYPE || status == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 415 or 422, got {}",
        status
    );
}

#[tokio::test]
async fn refresh_with_missing_token_field_returns_422() {
    let app = common::TestApp::new().await;

    let (status, _body) = app
        .request(
            Method::POST,
            "/api/auth/refresh",
            Some(json!({})),
            None,
        )
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn refresh_with_invalid_token_fails_at_db() {
    // NOTE: Valid JSON but fake refresh token — will fail at DB lookup
    let app = common::TestApp::new().await;

    let (status, _body) = app
        .request(
            Method::POST,
            "/api/auth/refresh",
            Some(json!({"refresh_token": "fake-refresh-token-abc"})),
            None,
        )
        .await;

    // Added: DB unreachable, so either 500 (connection error) or 401 (not found)
    assert!(
        status == StatusCode::INTERNAL_SERVER_ERROR || status == StatusCode::UNAUTHORIZED,
        "Expected 500 or 401, got {}",
        status
    );
}

// --- Protected route access without auth ---

#[tokio::test]
async fn protected_route_without_token_returns_401() {
    let app = common::TestApp::new().await;

    // NOTE: /api/folders is a protected route — requires Authorization header
    let (status, body) = app.request(Method::GET, "/api/folders", None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.get("error").is_some(), "Should return error JSON: {:?}", body);
}

#[tokio::test]
async fn protected_route_with_invalid_bearer_returns_401() {
    let app = common::TestApp::new().await;

    let (status, body) = app
        .request(Method::GET, "/api/folders", None, Some("not-a-valid-jwt"))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn protected_route_with_expired_token_returns_401() {
    let app = common::TestApp::new().await;
    let expired_token = common::create_expired_token();

    let (status, body) = app
        .request(Method::GET, "/api/folders", None, Some(&expired_token))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn protected_route_with_wrong_secret_returns_401() {
    // Added: Token signed with a different secret should be rejected
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, EncodingKey, Header};
    use tasmail::services::auth_service::Claims;

    let now = Utc::now();
    let claims = Claims {
        sub: uuid::Uuid::new_v4().to_string(),
        username: "wrong@example.com".to_string(),
        is_admin: false,
        exp: (now + Duration::seconds(900)).timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    let bad_token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"completely-wrong-secret"),
    )
    .unwrap();

    let app = common::TestApp::new().await;
    let (status, _body) = app
        .request(Method::GET, "/api/folders", None, Some(&bad_token))
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn multiple_protected_routes_require_auth() {
    // Added: Verify several protected endpoints all reject unauthenticated requests
    let app = common::TestApp::new().await;

    let protected_endpoints = vec![
        (Method::GET, "/api/folders"),
        (Method::GET, "/api/contacts"),
        (Method::GET, "/api/signatures"),
        (Method::GET, "/api/quota"),
        (Method::GET, "/api/groups"),
        (Method::GET, "/api/2fa/status"),
        (Method::GET, "/api/templates"),
        (Method::GET, "/api/filters"),
        (Method::POST, "/api/messages/send"),
        (Method::GET, "/api/admin/domains"),
    ];

    for (method, path) in protected_endpoints {
        let (status, _body) = app.request(method.clone(), path, None, None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "Expected 401 for {} {} but got {}",
            method,
            path,
            status
        );
    }
}
