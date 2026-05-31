// TMAIL-359 — Integration tests for the `/classic/login` GET + POST.
//
// Exercises the real Axum router (no DB queries hit, since the test pool
// points at a non-existent database — every test here covers a code path
// that returns BEFORE touching the database).
//
// Acceptance criteria from the issue:
//   * GET /classic/login → 200 with an HTML form + a pre-session CSRF
//     cookie scoped to /classic/login.
//   * GET /classic/login with an existing tasmail_classic_sid cookie →
//     303 to /classic/folders/INBOX (skip the form for logged-in users).
//   * POST /classic/login without the pre-session cookie → 400 + the
//     CSRF-specific error message in the re-rendered HTML.
//   * POST /classic/login with a mismatched cookie/form token → 400 +
//     CSRF error.
//   * POST /classic/login with empty email/password → 400 + the GENERIC
//     credential error (NOT the CSRF one — keeps the two failure branches
//     distinguishable for users while keeping the credential branch
//     account-existence-blind).
//   * Form layout: hidden _csrf input, autofocus on email, autocomplete
//     hints, no <script> tags, skip-link, inline <style nonce>.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use common::TestApp;
use http_body_util::BodyExt;

const LOGIN_CSRF_COOKIE: &str = "tasmail_classic_login_csrf";

/// Extract a single `Set-Cookie` header value by cookie name. Returns the
/// full Set-Cookie line so a caller can inspect attributes (Max-Age, Path).
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

/// Pull just the value portion of a `name=value; attr...` cookie string.
fn cookie_value<'a>(set_cookie: &'a str, name: &str) -> Option<&'a str> {
    set_cookie
        .strip_prefix(&format!("{name}="))?
        .split(';')
        .next()
}

async fn body_to_string(resp: axum::http::Response<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn get_login_renders_form_and_sets_csrf_cookie() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login")
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = body_to_string(app.raw_request(req).await).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.starts_with("text/html"))
            .unwrap_or(false),
        "Content-Type must be text/html, got {:?}",
        headers.get(header::CONTENT_TYPE)
    );

    // Pre-session CSRF cookie must be present with strict attributes.
    let set_cookie = find_set_cookie(&headers, LOGIN_CSRF_COOKIE)
        .expect("GET /classic/login must set the pre-session CSRF cookie");
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("Secure"));
    assert!(set_cookie.contains("SameSite=Strict"));
    assert!(set_cookie.contains("Path=/classic/login"));
    assert!(set_cookie.contains("Max-Age=900"));

    // Body must contain the login form scaffold.
    assert!(body.contains("action=\"/classic/login\""), "form action missing");
    assert!(body.contains("method=\"post\""), "POST method missing");
    assert!(body.contains("name=\"email\""), "email input missing");
    assert!(body.contains("name=\"password\""), "password input missing");
    assert!(body.contains("name=\"_csrf\""), "hidden _csrf field missing");

    // Per the gap analysis: no <script> tags on the no-JS surface.
    assert!(!body.contains("<script"), "login page must contain no <script> tags");
}

#[tokio::test]
async fn get_login_with_existing_session_cookie_redirects_to_inbox() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login")
        .header(header::COOKIE, "tasmail_classic_sid=opaque-session-id")
        .body(Body::empty())
        .unwrap();
    let resp = app.raw_request(req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/classic/folders/INBOX")
    );
}

#[tokio::test]
async fn get_login_token_matches_form_hidden_input() {
    // The cookie value and the form's `_csrf` hidden input MUST carry the
    // exact same token — that's the whole point of the double-submit
    // pattern. Catching a divergence here prevents a regression where a
    // refactor produces two tokens.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/login")
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = body_to_string(app.raw_request(req).await).await;
    assert_eq!(status, StatusCode::OK);

    let set_cookie = find_set_cookie(&headers, LOGIN_CSRF_COOKIE).expect("cookie set");
    let cookie_tok = cookie_value(&set_cookie, LOGIN_CSRF_COOKIE).expect("token in cookie");

    // The hidden input is `<input type="hidden" name="_csrf" value="...">`.
    let needle = format!("name=\"_csrf\" value=\"{cookie_tok}\"");
    assert!(
        body.contains(&needle),
        "form _csrf value must match cookie token. \
         expected substring: {needle}\n\
         body excerpt:\n{}",
        body.lines().filter(|l| l.contains("_csrf")).collect::<Vec<_>>().join("\n")
    );
}

#[tokio::test]
async fn post_login_without_csrf_cookie_is_rejected() {
    let app = TestApp::new().await;
    let body = "_csrf=anytoken&email=user@example.com&password=hunter2";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    let (status, _headers, body) = body_to_string(app.raw_request(req).await).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Re-rendered form carries the CSRF-specific error message.
    assert!(
        body.contains("session expired"),
        "CSRF error message missing from re-rendered form: {body}"
    );
    // And the form itself must still be present so the user can retry.
    assert!(body.contains("action=\"/classic/login\""));
    assert!(body.contains("role=\"alert\""));
}

#[tokio::test]
async fn post_login_with_mismatched_csrf_is_rejected() {
    let app = TestApp::new().await;
    let body = "_csrf=wrong-token-value&email=user@example.com&password=hunter2";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            "tasmail_classic_login_csrf=actual-token-from-cookie",
        )
        .body(Body::from(body))
        .unwrap();
    let (status, _headers, body) = body_to_string(app.raw_request(req).await).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("session expired"), "CSRF error not shown: {body}");
}

#[tokio::test]
async fn post_login_with_empty_email_shows_generic_credential_error() {
    // Empty inputs SHARE the generic credential error (not the CSRF one)
    // so the failure-message branching is symmetric with the
    // bad-credentials branch — an attacker can't distinguish.
    let app = TestApp::new().await;
    let body = "_csrf=match-token&email=&password=hunter2";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, "tasmail_classic_login_csrf=match-token")
        .body(Body::from(body))
        .unwrap();
    let (status, _headers, body) = body_to_string(app.raw_request(req).await).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("Incorrect email or password"),
        "generic credential error must show on empty input: {body}"
    );
    // MUST NOT mention "locked", "account does not exist", etc.
    let lower = body.to_lowercase();
    assert!(
        !lower.contains("locked"),
        "generic error must not leak lockout state: {body}"
    );
    assert!(
        !lower.contains("does not exist"),
        "generic error must not leak account existence: {body}"
    );
}

#[tokio::test]
async fn post_login_rerender_rotates_csrf_cookie() {
    // Every failed POST issues a FRESH pre-session CSRF cookie so the next
    // submission has a new token to match. Otherwise a stale cookie that
    // came along with the bad request would keep being acceptable.
    let app = TestApp::new().await;
    let body = "_csrf=match&email=&password=";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, "tasmail_classic_login_csrf=match")
        .body(Body::from(body))
        .unwrap();
    let (_status, headers, _body) = body_to_string(app.raw_request(req).await).await;

    let set_cookie = find_set_cookie(&headers, LOGIN_CSRF_COOKIE)
        .expect("failed POST must re-issue the pre-session CSRF cookie");
    let new_tok = cookie_value(&set_cookie, LOGIN_CSRF_COOKIE).expect("token value");
    assert_ne!(
        new_tok, "match",
        "re-issued CSRF token must differ from the consumed one"
    );
    // And it must still carry the strict attributes.
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
    assert!(set_cookie.contains("Path=/classic/login"));
}

#[tokio::test]
async fn login_404_does_not_swallow_post_to_login() {
    // Defence in depth: the `/classic/{*rest}` catch-all is GET-only so an
    // accidental POST to a non-existent classic path doesn't get hijacked
    // by the 404 page renderer.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/nonexistent")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(""))
        .unwrap();
    let resp = app.raw_request(req).await;
    // axum returns 405 Method Not Allowed when a route exists for GET but
    // not for POST. The exact status code matters less than "doesn't
    // silently 200 the login form" — anything 4xx is acceptable.
    assert!(
        resp.status().is_client_error(),
        "unknown classic path POST should be a 4xx, got {}",
        resp.status()
    );
}
