// TMAIL-358 — Integration tests for the `/classic` CSRF middleware.
//
// The middleware sits between `classic_session_middleware` (which injects
// the `ClassicSession` row into request extensions) and the actual POST
// handler. To exercise it without a live DB we stand up a minimal Axum
// router that:
//   1. Inserts a synthetic `ClassicSession` into extensions via a
//      from_fn middleware (acts as `classic_session_middleware`'s output).
//   2. Layers `classic_csrf_middleware` AFTER the session injector.
//   3. Mounts a no-op POST handler that returns 200 only on success.
//
// Acceptance cases (from the issue):
//   * Happy path             — POST with matching `_csrf` → 200
//   * Missing token          — POST without `_csrf` field   → 403 + HTML
//   * Mismatched token       — POST with wrong `_csrf`      → 403 + HTML
//   * Expired / no session   — POST with no ClassicSession  → 403 + HTML
//
// Bonus coverage:
//   * GET passes through untouched (CSRF middleware is no-op on safe methods).
//   * Multipart happy path  — POST with matching `_csrf` part → 200
//   * Bad content type      — POST with JSON body → 403
//
// No DB or external services touched.

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{header, Method, Request as HttpRequest, StatusCode},
    middleware::{from_fn, Next},
    routing::post,
    Router,
};
use chrono::Utc;
use http_body_util::BodyExt;
use tasmail::middleware::classic_csrf::classic_csrf_middleware;
use tasmail::models::classic_session::ClassicSession;
use tower::ServiceExt;
use uuid::Uuid;

const KNOWN_TOKEN: &str = "tEsTcSrFtOkEnFiXeDfOrAsSeRtIoNs01234567890_-";

/// Build a synthetic `ClassicSession` carrying a fixed CSRF token so the
/// tests can construct matching / mismatching submissions.
fn fixed_session(token: &str) -> ClassicSession {
    let now = Utc::now();
    ClassicSession {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        csrf_token: token.to_string(),
        created_at: now,
        expires_at: now + chrono::Duration::hours(24),
        last_seen_at: now,
        last_seen_ip: None,
        last_seen_ua: None,
    }
}

/// Build a router that pretends `classic_session_middleware` ran upstream by
/// injecting a fixed session into request extensions. Pass `None` to skip
/// injection so we can hit the "expired session" branch.
fn test_router_with_session(session: Option<ClassicSession>) -> Router {
    let csrf_layer = from_fn(classic_csrf_middleware);

    let mut router = Router::new()
        .route(
            "/classic/test/sink",
            post(|| async { "ok" }).get(|| async { "got" }),
        )
        .layer(csrf_layer);

    if let Some(s) = session {
        // Inject the session AS THE OUTERMOST layer so its
        // `request.extensions_mut` mutation lands before csrf_middleware
        // runs. Layers in axum run outer → inner on the request side.
        router = router.layer(from_fn(move |mut req: Request, next: Next| {
            let session = s.clone();
            async move {
                req.extensions_mut().insert(session);
                next.run(req).await
            }
        }));
    }

    router
}

/// Helper to send a form-urlencoded POST through the router and return
/// (status, body_string).
async fn post_form(router: &Router, path: &str, body: &str) -> (StatusCode, String) {
    let req = HttpRequest::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body_bytes.to_vec()).unwrap())
}

// ----- Happy path -----

#[tokio::test]
async fn happy_path_form_urlencoded_with_matching_token_passes_through() {
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    let body = format!("_csrf={KNOWN_TOKEN}&email=user%40example.com&password=hunter2");
    let (status, body) = post_form(&router, "/classic/test/sink", &body).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "matching CSRF token must pass through; body={body}"
    );
    assert_eq!(body, "ok", "handler response body must be 'ok'");
}

#[tokio::test]
async fn happy_path_csrf_field_anywhere_in_body() {
    // The token doesn't have to be first; templates may render it last (e.g.
    // a sticky hidden field after the user-visible inputs).
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    let body = format!("email=user%40example.com&password=hunter2&_csrf={KNOWN_TOKEN}");
    let (status, _body) = post_form(&router, "/classic/test/sink", &body).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn happy_path_multipart_with_matching_token_passes_through() {
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    let boundary = "----TASMailTestBoundary12345";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"_csrf\"\r\n\
         \r\n\
         {KNOWN_TOKEN}\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"subject\"\r\n\
         \r\n\
         Hello world\r\n\
         --{boundary}--\r\n"
    );

    let req = HttpRequest::builder()
        .method(Method::POST)
        .uri("/classic/test/sink")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "multipart with matching CSRF token must pass through"
    );
}

// ----- Missing token -----

#[tokio::test]
async fn missing_csrf_field_returns_403_with_retry_link_page() {
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    // No `_csrf` field anywhere in the body.
    let body = "email=user%40example.com&password=hunter2";
    let (status, body) = post_form(&router, "/classic/test/sink", body).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "missing CSRF must produce 403; body={body}"
    );
    // The retry-link page should render — assert the marker classes from
    // base.html + csrf_error.html so we know the right template ran.
    assert!(
        body.contains("class=\"skip-link\""),
        "expected base layout's skip-link in 403 page; body: {body}"
    );
    assert!(
        body.contains("missing its security token") || body.contains("Missing"),
        "expected 'missing security token' phrasing in 403 page; body: {body}"
    );
    assert!(
        body.contains("href=\"/classic/test/sink\""),
        "expected retry link pointing at original path; body: {body}"
    );
}

// ----- Mismatched token -----

#[tokio::test]
async fn mismatched_csrf_value_returns_403_with_retry_link_page() {
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    let body = format!("_csrf=WRONG_VALUE&email=user%40example.com");
    let (status, body) = post_form(&router, "/classic/test/sink", &body).await;

    assert_eq!(status, StatusCode::FORBIDDEN, "mismatched token must produce 403");
    // Askama auto-escapes the apostrophe in "didn't" to `didn&#39;t` — match
    // on the escape-safe "security token" prefix instead of trying to predict
    // the exact entity Askama emits.
    assert!(
        body.contains("security token") && body.contains("match"),
        "expected 'token didn't match' phrasing in 403; body: {body}"
    );
    assert!(
        body.contains("href=\"/classic/test/sink\""),
        "retry link must point at original path; body: {body}"
    );
}

#[tokio::test]
async fn mismatched_csrf_same_length_but_different_returns_403() {
    // Defence in depth: a same-length-but-different token must fail even
    // when the constant-time compare can't bail on length.
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    // Same length as KNOWN_TOKEN, every char different.
    assert_eq!(KNOWN_TOKEN.len(), 44);
    let bad = "X".repeat(44);
    let body = format!("_csrf={bad}");
    let (status, _) = post_form(&router, "/classic/test/sink", &body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn empty_csrf_value_returns_403_as_missing() {
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    let body = "_csrf=&email=x%40y.com";
    let (status, body) = post_form(&router, "/classic/test/sink", body).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "empty _csrf treated as missing");
    assert!(
        body.contains("missing its security token") || body.contains("Missing"),
        "empty value should hit the 'missing' branch, not 'mismatch'; body: {body}"
    );
}

// ----- Expired / no session -----

#[tokio::test]
async fn no_session_in_extensions_returns_403_session_expired() {
    // Caller forgot to wire classic_session_middleware, OR the session
    // genuinely expired between page load and form submission. Either way
    // refuse — we don't have an expected token to compare against.
    let router = test_router_with_session(None);

    // Even WITH a token in the body, refusal is correct because we can't
    // validate it.
    let body = format!("_csrf={KNOWN_TOKEN}&x=1");
    let (status, body) = post_form(&router, "/classic/test/sink", &body).await;

    assert_eq!(status, StatusCode::FORBIDDEN, "no session must produce 403");
    assert!(
        body.contains("session has expired") || body.contains("expired"),
        "expected 'session expired' phrasing in 403; body: {body}"
    );
    // The Content-Type must be HTML (not JSON) so a browser actually renders
    // the retry page.
    // (status already checked; this body assertion proves HTML rendered.)
    assert!(
        body.starts_with("<!DOCTYPE html>"),
        "expired-session response must be HTML, not JSON; body: {body}"
    );
}

// ----- Safe methods bypass + bad content type -----

#[tokio::test]
async fn get_passes_through_without_csrf_check() {
    // Safe methods (GET/HEAD/OPTIONS) per RFC 9110 §9.2.1 are never
    // server-state-changing — middleware short-circuits before reading body
    // OR session.
    let router = test_router_with_session(None); // No session even.

    let req = HttpRequest::builder()
        .method(Method::GET)
        .uri("/classic/test/sink")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET must bypass CSRF check entirely"
    );
}

#[tokio::test]
async fn post_with_unsupported_content_type_returns_403() {
    // `/classic/*` never accepts JSON bodies — only standard form encodings.
    // A misrouted SPA call hitting a classic POST must be refused.
    let session = fixed_session(KNOWN_TOKEN);
    let router = test_router_with_session(Some(session));

    let req = HttpRequest::builder()
        .method(Method::POST)
        .uri("/classic/test/sink")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(r#"{{"_csrf":"{KNOWN_TOKEN}"}}"#)))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "JSON on /classic/ POST must be refused even with a 'valid' token in it"
    );
}

#[tokio::test]
async fn put_method_is_validated() {
    // PUT is state-changing per RFC 9110; same protection as POST.
    // Build a PUT handler ad-hoc — the existing sink only takes POST/GET.
    let router = Router::new()
        .route("/classic/test/put", axum::routing::put(|| async { "puttered" }))
        .layer(from_fn(classic_csrf_middleware))
        .layer(from_fn({
            let session = fixed_session(KNOWN_TOKEN);
            move |mut req: Request, next: Next| {
                let session = session.clone();
                async move {
                    req.extensions_mut().insert(session);
                    next.run(req).await
                }
            }
        }));

    // Missing token → 403
    let req = HttpRequest::builder()
        .method(Method::PUT)
        .uri("/classic/test/put")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("no_csrf_here=1"))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Matching token → 200
    let req = HttpRequest::builder()
        .method(Method::PUT)
        .uri("/classic/test/put")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("_csrf={KNOWN_TOKEN}")))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn delete_method_is_validated() {
    let router = Router::new()
        .route("/classic/test/delete", axum::routing::delete(|| async { "deleted" }))
        .layer(from_fn(classic_csrf_middleware))
        .layer(from_fn({
            let session = fixed_session(KNOWN_TOKEN);
            move |mut req: Request, next: Next| {
                let session = session.clone();
                async move {
                    req.extensions_mut().insert(session);
                    next.run(req).await
                }
            }
        }));

    let req = HttpRequest::builder()
        .method(Method::DELETE)
        .uri("/classic/test/delete")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("_csrf={KNOWN_TOKEN}")))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn body_is_re_attached_for_downstream_handler() {
    // After validating the CSRF token the middleware re-streams the body to
    // the next handler — handlers must still see the full submitted form.
    let session = fixed_session(KNOWN_TOKEN);

    let router = Router::new()
        .route(
            "/classic/test/echo",
            post(|req: Request| async move {
                let (_, body) = req.into_parts();
                let bytes = to_bytes(body, 1024).await.unwrap();
                String::from_utf8(bytes.to_vec()).unwrap()
            }),
        )
        .layer(from_fn(classic_csrf_middleware))
        .layer(from_fn({
            let session = session.clone();
            move |mut req: Request, next: Next| {
                let session = session.clone();
                async move {
                    req.extensions_mut().insert(session);
                    next.run(req).await
                }
            }
        }));

    let original_body = format!("_csrf={KNOWN_TOKEN}&subject=Hello&body=World");
    let req = HttpRequest::builder()
        .method(Method::POST)
        .uri("/classic/test/echo")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(original_body.clone()))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let echoed = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(
        echoed, original_body,
        "downstream handler must see the original body unchanged"
    );
}
