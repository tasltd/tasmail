// Added (TMAIL-304): HTTP-layer integration coverage for the OIDC callback.
//
// Before this fix, the handler always returned a hard-coded
// `"OIDC callback token exchange not yet implemented..."` 400 — so every
// error path was effectively unreachable and `code` / `state` validation
// was the only thing that ever ran. These tests pin the wire contract
// introduced by the implementation:
//
// - missing body / missing required fields → 4xx at the extractor layer
// - missing `code` → 400 (was already enforced)
// - missing `state` → 400 (was already enforced)
// - missing `provider_id` → 400 (NEW guard required by the real flow)
// - non-UUID `provider_id` → 422 (extractor-level rejection)
// - unknown `provider_id` UUID → 404 or 500 (DB unreachable in this
//   harness; both prove the placeholder return is gone and the handler
//   now does real work)
//
// As with the other suites in tests/, the TestApp uses a non-existent DB
// — these tests verify routing, JSON validation, and the early-return
// guards. The end-to-end happy path (real IdP discovery + token exchange
// + JWKS verification) is covered by the pure-function suite in
// models/oidc_provider.rs and handlers/oidc.rs and is documented for
// manual smoke testing on the PM ticket.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn oidc_callback_with_missing_body_returns_client_error() {
    let app = common::TestApp::new().await;
    let (status, _) = app
        .request(Method::POST, "/api/auth/oidc/callback", None, None)
        .await;
    assert!(
        status == StatusCode::UNSUPPORTED_MEDIA_TYPE
            || status == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 415 or 422 for empty body, got {}",
        status
    );
}

#[tokio::test]
async fn oidc_callback_with_missing_code_returns_400() {
    let app = common::TestApp::new().await;
    let pid = Uuid::new_v4();
    let (status, body) = app
        .request(
            Method::POST,
            "/api/auth/oidc/callback",
            Some(json!({
                "code": "",
                "state": "csrf-token",
                "provider_id": pid.to_string()
            })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["error"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        msg.contains("authorization code"),
        "Expected 'authorization code' error, got {:?}",
        body
    );
}

#[tokio::test]
async fn oidc_callback_with_missing_state_returns_400() {
    let app = common::TestApp::new().await;
    let pid = Uuid::new_v4();
    let (status, body) = app
        .request(
            Method::POST,
            "/api/auth/oidc/callback",
            Some(json!({
                "code": "auth-code",
                "state": "",
                "provider_id": pid.to_string()
            })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["error"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        msg.contains("state"),
        "Expected 'state' error, got {:?}",
        body
    );
}

#[tokio::test]
async fn oidc_callback_without_provider_id_returns_400() {
    // TMAIL-304: provider_id is required so the backend can resolve the
    // IdP config (token_endpoint, client_secret). The SPA stores it in
    // sessionStorage alongside `state` when calling
    // /api/auth/oidc/{id}/authorize and re-sends it on the callback.
    let app = common::TestApp::new().await;
    let (status, body) = app
        .request(
            Method::POST,
            "/api/auth/oidc/callback",
            Some(json!({
                "code": "auth-code",
                "state": "csrf-token"
            })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["error"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        msg.contains("provider_id"),
        "Expected 'provider_id' error, got {:?}",
        body
    );
}

#[tokio::test]
async fn oidc_callback_with_garbage_provider_id_returns_422() {
    // A provider_id that doesn't parse as a UUID must fail JSON extraction
    // (the field is typed as Option<Uuid>) — handler never sees it.
    let app = common::TestApp::new().await;
    let (status, _) = app
        .request(
            Method::POST,
            "/api/auth/oidc/callback",
            Some(json!({
                "code": "auth-code",
                "state": "csrf-token",
                "provider_id": "not-a-uuid"
            })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn oidc_callback_with_unknown_provider_id_fails_at_db_layer() {
    // When provider_id is a well-formed UUID but the DB lookup fails
    // (no DB connection in this harness), the handler now reaches the
    // OidcProvider::get_by_id call. Either 404 (not found path) or 500
    // (connection refused) is acceptable — both prove the placeholder
    // return is gone and the handler does real work.
    let app = common::TestApp::new().await;
    let (status, _) = app
        .request(
            Method::POST,
            "/api/auth/oidc/callback",
            Some(json!({
                "code": "auth-code",
                "state": "csrf-token",
                "provider_id": Uuid::new_v4().to_string()
            })),
            None,
        )
        .await;
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR,
        "Expected 404 or 500, got {}",
        status
    );
}

#[tokio::test]
async fn oidc_callback_no_longer_returns_placeholder_string() {
    // Regression guard: the pre-fix handler returned a literal
    // `"OIDC callback token exchange not yet implemented..."` error body
    // for every request. After the fix, that string MUST NOT appear in
    // any of the early-return error responses — they're all about the
    // specific missing field (code / state / provider_id).
    let app = common::TestApp::new().await;
    let cases = [
        json!({"code":"","state":"x","provider_id": Uuid::new_v4().to_string()}),
        json!({"code":"x","state":"","provider_id": Uuid::new_v4().to_string()}),
        json!({"code":"x","state":"x"}),
    ];
    for body_in in cases {
        let (_, body) = app
            .request(Method::POST, "/api/auth/oidc/callback", Some(body_in), None)
            .await;
        let msg = body["error"].as_str().unwrap_or_default().to_lowercase();
        assert!(
            !msg.contains("not yet implemented"),
            "Placeholder string should be gone, got {:?}",
            body
        );
    }
}
