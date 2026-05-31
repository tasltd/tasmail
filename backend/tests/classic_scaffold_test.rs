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

// ----- TMAIL-356: base.html layout integration assertions -----
//
// Verify the base layout features (skip-link, semantic landmarks, CSP nonce,
// no scripts, lang attribute) survive the round-trip through the real Axum
// router. These complement the in-process unit tests in
// `src/handlers/classic/mod.rs` by proving the rendered HTTP body — what an
// actual browser would see — carries the same structure.

#[tokio::test]
async fn classic_base_layout_renders_html5_with_lang_and_viewport() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/triggers-the-404-page-and-thus-renders-base-layout")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(
        body.starts_with("<!DOCTYPE html>"),
        "response body must start with HTML5 doctype; got: {}",
        &body[..body.len().min(60)]
    );
    assert!(
        body.contains("<html lang=\"en\">"),
        "<html> must declare a language for WCAG 3.1.1"
    );
    assert!(
        body.contains("<meta name=\"viewport\""),
        "<meta viewport> required for mobile rendering"
    );
}

#[tokio::test]
async fn classic_base_layout_has_semantic_landmarks_and_skip_link() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/x")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    // Landmarks
    for needle in ["<header", "<nav", "<main id=\"main\"", "<footer"] {
        assert!(
            body.contains(needle),
            "missing semantic landmark `{needle}` in rendered body"
        );
    }
    // Skip link is present and targets #main
    assert!(
        body.contains("class=\"skip-link\"") && body.contains("href=\"#main\""),
        "skip-link to #main is required for WCAG 2.4.1"
    );

    // Skip link comes BEFORE any other interactive element in <body>.
    // Locate the skip-link by its class (not by `href="#main"`, which sits
    // AFTER `<a ` in the same opening tag and would falsely register `<a `
    // as appearing earlier).
    let body_start = body.find("<body>").expect("must contain <body>");
    let in_body = &body[body_start + "<body>".len()..];
    let skip_class_at = in_body
        .find("class=\"skip-link\"")
        .expect("skip-link element not found inside <body>");
    let skip_link_tag_at = in_body[..skip_class_at]
        .rfind("<a ")
        .expect("class=\"skip-link\" must sit on an <a> tag");
    let before_skip = &in_body[..skip_link_tag_at];
    for needle in ["<a ", "<button", "<input", "<select", "<textarea"] {
        assert!(
            !before_skip.contains(needle),
            "found interactive element `{needle}` BEFORE the skip-link — \
             skip-link must be the first focusable element (WCAG 2.4.1)"
        );
    }
}

#[tokio::test]
async fn classic_base_layout_carries_csp_nonce_on_inline_style() {
    // The whole reason for this task: the inline <style> needs a nonce
    // attribute so the strict CSP planned in TMAIL-368 doesn't strip it.
    // We can't predict the nonce value here (it's per-request random) so we
    // assert the shape: <style nonce="<non-empty>">.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/x")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    let style_at = body
        .find("<style nonce=\"")
        .expect("inline <style> must have a nonce attribute");
    let after = &body[style_at + "<style nonce=\"".len()..];
    let end_quote = after.find('"').expect("nonce attribute must be quoted");
    let nonce = &after[..end_quote];
    assert!(
        !nonce.is_empty(),
        "nonce attribute must not be empty"
    );
    assert!(
        nonce.len() >= 16,
        "nonce should be at least 16 chars (base64 of 16 random bytes ≈ 24); \
         got {} chars: {:?}",
        nonce.len(),
        nonce
    );
}

#[tokio::test]
async fn classic_base_layout_emits_no_script_tags() {
    // Hard rule: /classic is a no-JS surface. Lock down that NO <script>
    // ever sneaks into the response — covers both inline and external.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/x")
        .body(Body::empty())
        .unwrap();

    let response = app.raw_request(req).await;
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(
        !body.contains("<script"),
        "Classic UI response contains a <script> tag — must be zero per spec"
    );
}

/// TMAIL-356: writes the rendered base layout (via the 404 page) to a
/// caller-supplied path so a text-mode browser like `lynx` / `w3m` can
/// dump it and prove the surface degrades gracefully. Ignored by default
/// because it's a developer convenience, not a regression gate.
///
/// Usage:
///   TASMAIL_DUMP_404=/tmp/classic-404.html \
///       cargo test --test classic_scaffold_test \
///       classic_dump_base_layout_for_text_browser_smoke_test -- --ignored
///   w3m -dump /tmp/classic-404.html
#[tokio::test]
#[ignore]
async fn classic_dump_base_layout_for_text_browser_smoke_test() {
    let path = std::env::var("TASMAIL_DUMP_404")
        .unwrap_or_else(|_| "/tmp/tasmail-classic-404.html".to_string());
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/text-browser-smoke")
        .body(Body::empty())
        .unwrap();
    let response = app.raw_request(req).await;
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    std::fs::write(&path, &body_bytes).expect("could not write dump file");
    eprintln!("classic 404 page written to {path} — try `w3m -dump {path}`");
}

#[tokio::test]
async fn classic_base_layout_nonce_is_unique_across_requests() {
    // Nonce reuse defeats CSP. Two consecutive requests to the same path
    // must produce two different nonces in the rendered <style>.
    let app = TestApp::new().await;

    let nonce_from = |body: &str| -> String {
        let at = body
            .find("<style nonce=\"")
            .expect("inline <style> nonce attribute missing");
        let after = &body[at + "<style nonce=\"".len()..];
        let end = after.find('"').expect("nonce attribute must be quoted");
        after[..end].to_string()
    };

    let req1 = Request::builder()
        .method(Method::GET)
        .uri("/classic/x")
        .body(Body::empty())
        .unwrap();
    let resp1 = app.raw_request(req1).await;
    let body1 = String::from_utf8(
        resp1.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
    .unwrap();
    let nonce1 = nonce_from(&body1);

    let req2 = Request::builder()
        .method(Method::GET)
        .uri("/classic/x")
        .body(Body::empty())
        .unwrap();
    let resp2 = app.raw_request(req2).await;
    let body2 = String::from_utf8(
        resp2.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
    .unwrap();
    let nonce2 = nonce_from(&body2);

    assert_ne!(
        nonce1, nonce2,
        "nonces from two separate requests must differ — reuse defeats CSP"
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
