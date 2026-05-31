// TMAIL-355: Integration tests for the `/classic` no-JS surface scaffold.
//
// Covers the full routing round-trip through the real Axum router so this
// scaffold doesn't quietly regress when child tasks (login form, message
// list, compose) add their own routes under the same nest.
//
// Verifies:
//   * GET /classic/  (no cookie)               → 303 to /classic/login
//   * GET /classic/  (with session cookie)     → 303 to /classic/folders/INBOX
//   * GET /classic/does-not-exist              → 404 with HTML body
//   * The 404 path does NOT collide with /api/*: a miss under /api/* must
//     not be hijacked by the classic fallback.
//
// No DB or external services touched — these only exercise the routing
// + render layer.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use common::TestApp;
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn classic_index_without_session_redirects_to_login() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("redirect must set Location header")
        .to_str()
        .unwrap();
    assert_eq!(location, "/classic/login");
}

#[tokio::test]
async fn classic_index_with_session_redirects_to_inbox() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/")
        .header(header::COOKIE, "tasmail_classic_sid=opaque-session-id")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("redirect must set Location header")
        .to_str()
        .unwrap();
    assert_eq!(location, "/classic/folders/INBOX");
}

#[tokio::test]
async fn classic_index_with_empty_session_cookie_redirects_to_login() {
    // A cleared session cookie (Max-Age=0) typically leaves the name behind
    // with an empty value. We treat that as no session — without this, a
    // logged-out user would bounce to /folders/INBOX and then back to login,
    // wasting a round-trip.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/")
        .header(header::COOKIE, "tasmail_classic_sid=")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/classic/login");
}

#[tokio::test]
async fn classic_unknown_path_returns_html_404() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/this-route-does-not-exist")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // PURPOSE: confirm the body is HTML (not JSON) so the user actually sees
    // the rendered error page rather than the JSON AppError envelope that
    // /api/* routes emit.
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("404 must set Content-Type")
        .to_str()
        .unwrap();
    assert!(
        content_type.starts_with("text/html"),
        "expected text/html, got {content_type}"
    );

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(body.contains("Page not found"), "body: {body}");
    assert!(
        body.contains("/classic/this-route-does-not-exist"),
        "404 should echo the missing path; body: {body}"
    );
    assert!(
        body.contains("<a href=\"/classic/\""),
        "404 should link back to /classic/; body: {body}"
    );
}

#[tokio::test]
async fn classic_404_does_not_hijack_api_misses() {
    // SAFETY NET: if the classic fallback ever gets promoted to a top-level
    // router fallback by mistake, this test catches it — /api/* misses must
    // still return JSON (Axum's MethodNotAllowed / route-not-found behaviour),
    // never the classic HTML 404 page.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/this-route-does-not-exist")
        .body(Body::empty())
        .unwrap();

    let response = app.router.clone().oneshot(req).await.unwrap();

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        !content_type.starts_with("text/html"),
        "API miss must NOT return classic HTML 404; got Content-Type {content_type}"
    );
}
