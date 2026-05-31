// TMAIL-361 — Integration tests for the `/classic/login/2fa` GET + POST and
// the `/classic/login` flash-param handling that surfaces the bounce reasons.
//
// Exercises the real Axum router (no DB queries hit, since the test pool
// points at a non-existent database — every test here covers a code path
// that returns BEFORE touching the database).
//
// Acceptance criteria from the issue:
//   * GET /classic/login/2fa with no pending cookie → 303 to
//     /classic/login?error=2fa_expired + Set-Cookie clearing the pending
//     cookie.
//   * GET /classic/login/2fa with a forged signature → 303 + clear cookie
//     (defence against UUID guess + signature swap).
//   * POST /classic/login/2fa with no cookie → 303 to login.
//   * POST /classic/login/2fa with empty CSRF → bounce.
//   * GET /classic/login?error=2fa_expired → renders the "your verification
//     session expired" flash inside role="alert".
//   * GET /classic/login?error=2fa_too_many → renders the "too many incorrect
//     codes" flash.
//   * GET /classic/login?error=garbage → ignored (no flash).
//
// We deliberately avoid the success path (real code accepted) because that
// requires a working database — covered by manual + Playwright E2E.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use common::TestApp;
use http_body_util::BodyExt;

const PENDING_2FA_COOKIE: &str = "tasmail_classic_pending_2fa";

fn find_set_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    for v in headers.get_all(header::SET_COOKIE) {
        if let Ok(s) = v.to_str() {
            if s.starts_with(&format!("{name}=")) {
                return Some(s.to_string());
            }
        }
    }
    None
}

async fn body_to_string(
    resp: axum::http::Response<Body>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

// ----- GET /classic/login/2fa -----

#[tokio::test]
async fn get_challenge_without_pending_cookie_bounces_to_login() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login/2fa")
        .body(Body::empty())
        .unwrap();
    let resp = app.raw_request(req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        loc.starts_with("/classic/login"),
        "should redirect to /classic/login, got {loc}"
    );
    assert!(
        loc.contains("error=2fa_expired"),
        "should carry the 2fa_expired flash, got {loc}"
    );
    let clear = find_set_cookie(resp.headers(), PENDING_2FA_COOKIE)
        .expect("bounce must include a clear-cookie header");
    assert!(clear.contains("Max-Age=0"));
}

#[tokio::test]
async fn get_challenge_with_forged_signature_bounces() {
    // A well-formed cookie shape (uuid.sig) but with a signature that
    // doesn't HMAC-verify must NOT leak whether the row exists — we want
    // the same bounce regardless. Defends against an attacker guessing
    // a valid pending-token UUID and pairing it with a junk signature.
    let app = TestApp::new().await;
    let forged_cookie = format!("{PENDING_2FA_COOKIE}=deadbeefdeadbeefdeadbeefdeadbeef.junksig");
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login/2fa")
        .header(header::COOKIE, forged_cookie)
        .body(Body::empty())
        .unwrap();
    let resp = app.raw_request(req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(loc.starts_with("/classic/login"));
    assert!(loc.contains("error=2fa_expired"));
}

#[tokio::test]
async fn get_challenge_with_malformed_cookie_bounces() {
    // No dot separator → cookie can't be parsed → bounce.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login/2fa")
        .header(header::COOKIE, format!("{PENDING_2FA_COOKIE}=just-no-dot"))
        .body(Body::empty())
        .unwrap();
    let resp = app.raw_request(req).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

// ----- POST /classic/login/2fa -----

#[tokio::test]
async fn post_challenge_without_cookie_bounces_to_login() {
    let app = TestApp::new().await;
    let body = "_csrf=anything&code=123456";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/login/2fa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    let resp = app.raw_request(req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(loc.starts_with("/classic/login"));
    assert!(loc.contains("error=2fa_expired"));
    let clear = find_set_cookie(resp.headers(), PENDING_2FA_COOKIE)
        .expect("bounce must include a clear-cookie header");
    assert!(clear.contains("Max-Age=0"));
}

#[tokio::test]
async fn post_challenge_with_forged_signature_bounces() {
    let app = TestApp::new().await;
    let body = "_csrf=anything&code=123456";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/login/2fa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("{PENDING_2FA_COOKIE}=deadbeefdeadbeefdeadbeefdeadbeef.forgedsig"),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = app.raw_request(req).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(loc.starts_with("/classic/login"));
    assert!(loc.contains("error=2fa_expired"));
}

// ----- GET /classic/login?error=... flash handling -----

#[tokio::test]
async fn login_flash_renders_2fa_expired_message() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login?error=2fa_expired")
        .body(Body::empty())
        .unwrap();
    let (status, _headers, body) = body_to_string(app.raw_request(req).await).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("role=\"alert\""),
        "expected role=\"alert\" on flashed login page: {body}"
    );
    assert!(
        body.to_lowercase().contains("verification session expired"),
        "expected expired-session message, got body: {body}"
    );
}

#[tokio::test]
async fn login_flash_renders_too_many_codes_message() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login?error=2fa_too_many")
        .body(Body::empty())
        .unwrap();
    let (status, _headers, body) = body_to_string(app.raw_request(req).await).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("role=\"alert\""));
    assert!(
        body.to_lowercase().contains("too many incorrect"),
        "expected too-many-codes message, got body: {body}"
    );
}

#[tokio::test]
async fn login_flash_unknown_error_is_dropped() {
    // Whitelist defence: anything unrecognised must NOT render a flash
    // (no reflected-XSS-by-error-string vector even though Askama escapes).
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login?error=%3Cscript%3Ealert(1)%3C/script%3E")
        .body(Body::empty())
        .unwrap();
    let (status, _headers, body) = body_to_string(app.raw_request(req).await).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains("role=\"alert\""),
        "unknown error param must not flash, body: {body}"
    );
    // Defence in depth: even if it had rendered, Askama escapes.
    assert!(!body.contains("<script>alert(1)</script>"));
}

// ----- 2FA challenge form layout (renders via the public router) -----
//
// The fresh-render path needs a working pending-2fa cookie, which we can't
// produce here without a real DB. The handler-level unit tests already
// exercise the template; this test pins the *route* mapping so the public
// sub-router actually routes /classic/login/2fa.

#[tokio::test]
async fn get_challenge_route_is_mounted_under_public_subrouter() {
    // Both the present-cookie and absent-cookie paths must reach the
    // handler, not the 404 catch-all. The "no cookie" path returns a 303
    // (from `bounce_to_login_with_reason`). A miss would return 404.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login/2fa")
        .body(Body::empty())
        .unwrap();
    let resp = app.raw_request(req).await;
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "route must be mounted (303 bounce), not 404"
    );
}
