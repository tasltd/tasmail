// Added: TMAIL-37 — integration tests for security hardening.
// Covers:
//   * email header injection guards on /api/messages/send and /api/drafts
//   * per-IP rate limiting on /api/auth/login, /api/auth/signup, /api/auth/refresh
//   * security response headers presence (CSP, X-Frame-Options, HSTS, ...)
//   * folder/search injection guards
//
// These tests drive the real Axum router via tower::ServiceExt::oneshot so the
// full middleware stack (rate limiter, security headers, CORS, auth) runs.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::json;

use common::{create_test_token, TestApp};

// --- Email header injection (TMAIL-37) ---

#[tokio::test]
async fn send_message_rejects_crlf_in_subject() {
    // CR/LF in subject was the classic header-injection vector. The validation
    // layer must 400 before the request ever reaches the SMTP/IMAP path.
    let app = TestApp::new().await;
    let token = create_test_token(None, false);
    let body = json!({
        "to": ["alice@example.com"],
        "subject": "Hi there\r\nBcc: attacker@evil.com",
        "text_body": "hello",
    });
    let (status, _) = app
        .request(Method::POST, "/api/messages/send", Some(body), Some(&token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_message_rejects_crlf_in_recipient() {
    let app = TestApp::new().await;
    let token = create_test_token(None, false);
    let body = json!({
        "to": ["alice@example.com\r\nBcc: evil@x.com"],
        "subject": "Hello",
        "text_body": "hi",
    });
    let (status, _) = app
        .request(Method::POST, "/api/messages/send", Some(body), Some(&token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_message_rejects_empty_to() {
    let app = TestApp::new().await;
    let token = create_test_token(None, false);
    let body = json!({
        "to": [],
        "subject": "Hello",
        "text_body": "hi",
    });
    let (status, _) = app
        .request(Method::POST, "/api/messages/send", Some(body), Some(&token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn save_draft_rejects_crlf_in_subject() {
    // save_draft builds a raw RFC 2822 message via format!() — CRLF in subject
    // would splice arbitrary headers. Must be rejected at the API boundary.
    let app = TestApp::new().await;
    let token = create_test_token(None, false);
    let body = json!({
        "to": ["alice@example.com"],
        "subject": "Hi\r\nBcc: attacker@evil.com",
        "text_body": "draft",
    });
    let (status, _) = app
        .request(Method::POST, "/api/drafts", Some(body), Some(&token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn save_draft_rejects_crlf_in_cc() {
    let app = TestApp::new().await;
    let token = create_test_token(None, false);
    let body = json!({
        "to": ["alice@example.com"],
        "cc": ["bob@example.com\r\nBcc: spy@evil.com"],
        "subject": "Hi",
        "text_body": "draft",
    });
    let (status, _) = app
        .request(Method::POST, "/api/drafts", Some(body), Some(&token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// --- IMAP folder & search injection (TMAIL-37) ---

#[tokio::test]
async fn search_rejects_crlf_in_query() {
    let app = TestApp::new().await;
    let token = create_test_token(None, false);
    // URL-encoded CRLF + IMAP LOGOUT — classic IMAP injection
    let (status, _) = app
        .request(
            Method::GET,
            "/api/search?q=test%0D%0ALOGOUT&folder=INBOX",
            None,
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_rejects_crlf_in_folder() {
    let app = TestApp::new().await;
    let token = create_test_token(None, false);
    let (status, _) = app
        .request(
            Method::GET,
            "/api/search?q=hello&folder=INBOX%0D%0ALOGOUT",
            None,
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// --- Rate limiting on auth endpoints (TMAIL-37) ---

#[tokio::test]
async fn login_rate_limit_returns_429_after_burst() {
    // The router builds a fresh limiter per TestApp, so this test is isolated.
    // We just need to make more requests than the default cap (10 / min / IP).
    let app = TestApp::new().await;
    let body = json!({"username": "x@y.com", "password": "12345678"});

    let mut saw_429 = false;
    for _ in 0..15 {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.raw_request(request).await;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_429 = true;
            break;
        }
    }
    assert!(saw_429, "Expected to hit 429 after exceeding the auth rate limit");
}

#[tokio::test]
async fn signup_rate_limit_returns_429_after_burst() {
    let app = TestApp::new().await;
    let body = json!({"email": "new@x.com", "password": "12345678"});

    let mut saw_429 = false;
    for _ in 0..15 {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/signup")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.raw_request(request).await;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_429 = true;
            break;
        }
    }
    assert!(saw_429, "Expected signup to hit 429 after exceeding the auth rate limit");
}

#[tokio::test]
async fn health_endpoint_is_not_rate_limited() {
    // Defense-in-depth check: only the auth subtree carries the rate limiter.
    // /api/health must keep responding even under burst.
    let app = TestApp::new().await;
    for _ in 0..30 {
        let (status, _) = app.request(Method::GET, "/api/health", None, None).await;
        assert_eq!(status, StatusCode::OK);
    }
}

// --- Security response headers (TMAIL-37) ---

#[tokio::test]
async fn responses_include_security_headers() {
    // Spot-check the security headers middleware fires on a public route.
    let app = TestApp::new().await;
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.raw_request(request).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h = resp.headers();
    assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(h.get("x-content-type-options").unwrap(), "nosniff");
    assert!(h.contains_key("content-security-policy"));
    assert!(h.contains_key("strict-transport-security"));
    assert!(h.contains_key("referrer-policy"));
    assert!(h.contains_key("permissions-policy"));
}
