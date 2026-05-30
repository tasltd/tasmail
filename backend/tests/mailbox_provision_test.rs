// TMAIL-305: Integration tests for POST /api/mailbox/provision
//
// The endpoint MUST fail-closed (503 Service Unavailable) until the doveadm
// SSH bridge is implemented. The previous build wrote an imap_configurations
// row with a literal "REPLACE_ME_WITH_DOVEADM_GENERATED_PASSWORD" placeholder
// and returned 201, telling the user their mailbox was ready while guaranteeing
// they could not log in. These tests assert that regression never returns.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;

const PROVISION_PATH: &str = "/api/mailbox/provision";

#[tokio::test]
async fn provision_without_auth_returns_401() {
    // Sanity check: the endpoint is behind auth_middleware.
    let app = common::TestApp::new().await;
    let (status, _body) = app
        .request(Method::POST, PROVISION_PATH, Some(json!({"local_part": "alice"})), None)
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn provision_with_auth_returns_503_when_ssh_bridge_not_wired() {
    // TMAIL-305 regression guard: even when auth succeeds, the endpoint must
    // refuse to create an imap_configurations row because the doveadm SSH bridge
    // is not implemented. The fail-closed path may exit at any one of the
    // gates (feature flag DB unreachable, missing env vars, or the explicit
    // "bridge not wired" 503), but the response MUST be 503 in every case.
    let app = common::TestApp::new().await;
    let token = common::create_test_token(None, false);

    let (status, body) = app
        .request(
            Method::POST,
            PROVISION_PATH,
            Some(json!({"local_part": "alice"})),
            Some(&token),
        )
        .await;

    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "Expected 503 fail-closed, got {} — body: {:?}",
        status, body
    );

    // The body should carry an error message — never a 201-style ProvisionResponse.
    assert!(
        body.get("error").is_some(),
        "503 response should carry an error field, got: {:?}",
        body
    );
    // Negative assertion: must NOT look like a success ProvisionResponse.
    assert!(
        body.get("imap_config_id").is_none(),
        "503 response leaked a success payload (imap_config_id present): {:?}",
        body
    );
    assert!(
        body.get("email").is_none(),
        "503 response leaked a success payload (email present): {:?}",
        body
    );
}

#[tokio::test]
async fn provision_never_returns_201_with_placeholder_credential() {
    // The smoking-gun assertion: the response body must never contain the
    // REPLACE_ME placeholder marker. This catches the specific regression in
    // TMAIL-305 where the handler wrote an encrypted "REPLACE_ME..." password
    // into imap_configurations and acknowledged the request as 201 CREATED.
    let app = common::TestApp::new().await;
    let token = common::create_test_token(None, false);

    let (status, body) = app
        .request(
            Method::POST,
            PROVISION_PATH,
            Some(json!({"local_part": "alice"})),
            Some(&token),
        )
        .await;

    assert_ne!(
        status,
        StatusCode::CREATED,
        "Endpoint must NOT return 201 while the doveadm SSH bridge is unimplemented"
    );

    let body_str = body.to_string();
    assert!(
        !body_str.contains("REPLACE_ME"),
        "Response leaked the REPLACE_ME placeholder credential: {}",
        body_str
    );
}
