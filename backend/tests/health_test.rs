// Integration tests for the /api/health family.
//
// NOTE: TestApp wires a CacheService in passthrough mode (no Redis) and a
// PgPool pointed at a non-existent DB. All readiness components therefore
// report as disconnected/error — which is exactly what we want to assert
// for the TMAIL-310 spec ("Redis down → /api/health?detail=full reports
// redis: disconnected").
//
// The back-compat tests at the top continue to assert the legacy shape so
// existing uptime monitors keep working.

mod common;

use axum::http::{Method, StatusCode};

// --- Back-compat shape (TMAIL-310 must preserve this) ---

#[tokio::test]
async fn health_returns_200() {
    let app = common::TestApp::new().await;
    let (status, body) = app.request(Method::GET, "/api/health", None, None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("status").is_some(), "Response missing 'status' field: {:?}", body);
    assert!(body.get("version").is_some(), "Response missing 'version' field: {:?}", body);
    assert!(body.get("database").is_some(), "Response missing 'database' field: {:?}", body);
}

#[tokio::test]
async fn health_is_public_no_auth_needed() {
    let app = common::TestApp::new().await;
    let (status, _body) = app.request(Method::GET, "/api/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn health_reports_degraded_without_db() {
    // The test pool points to a non-existent DB, so the back-compat shape
    // must still flag "degraded" / "disconnected".
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
    assert!(!version.is_empty(), "Version should not be empty");
}

// --- TMAIL-310: structured ?detail=full report ---

#[tokio::test]
async fn health_detail_full_returns_structured_components() {
    let app = common::TestApp::new().await;
    let (status, body) = app
        .request(Method::GET, "/api/health?detail=full", None, None)
        .await;

    // ?detail=full ALWAYS returns 200 from /api/health — the detailed mode
    // is informational. /api/health/ready is the gating probe (503 on
    // degradation). This split lets uptime monitors keep polling /api/health
    // without flipping red while still surfacing per-component status.
    assert_eq!(status, StatusCode::OK);

    // Top-level shape — version + per-component blocks.
    assert!(body["version"].is_string(), "missing version: {:?}", body);
    for comp in &["database", "mailboxes", "redis", "queue"] {
        assert!(
            body.get(*comp).is_some(),
            "structured report missing '{}' block: {:?}",
            comp,
            body
        );
        assert!(body[comp].get("ok").is_some(), "{} block missing 'ok'", comp);
        assert!(body[comp].get("status").is_some(), "{} block missing 'status'", comp);
    }
}

#[tokio::test]
async fn health_detail_full_reports_redis_disconnected_when_redis_down() {
    // This is the explicit acceptance test from TMAIL-310:
    //   "Integration test with Redis down → asserts /api/health?detail=full
    //    reports redis: disconnected."
    //
    // TestApp uses CacheService::disabled() — the same code path the runtime
    // takes when Redis is unreachable at boot.
    let app = common::TestApp::new().await;
    let (status, body) = app
        .request(Method::GET, "/api/health?detail=full", None, None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["redis"]["ok"], false,
        "redis must be marked unhealthy when disabled: {:?}",
        body["redis"]
    );
    assert_eq!(
        body["redis"]["status"], "disconnected",
        "redis status must be 'disconnected' when Redis is down: {:?}",
        body["redis"]
    );
    assert_eq!(body["status"], "degraded");
}

#[tokio::test]
async fn health_detail_full_reports_queue_not_started_before_first_tick() {
    let app = common::TestApp::new().await;
    let (status, body) = app
        .request(Method::GET, "/api/health?detail=full", None, None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["queue"]["ok"], false);
    assert_eq!(body["queue"]["status"], "not_started");
}

#[tokio::test]
async fn health_detail_full_reports_queue_ok_after_tick() {
    let app = common::TestApp::new().await;
    // Simulate the queue processor running a cycle.
    app.queue_heartbeat.record_tick();

    let (status, body) = app
        .request(Method::GET, "/api/health?detail=full", None, None)
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["queue"]["ok"], true, "queue should be ok after tick: {:?}", body["queue"]);
    assert_eq!(body["queue"]["status"], "ok");
}

// --- TMAIL-310: liveness probe ---

#[tokio::test]
async fn health_live_returns_503_when_db_unreachable() {
    // Liveness deliberately depends on the DB ping — if Postgres is gone,
    // restarting the process is the right call. (Redis being down is NOT
    // a liveness failure — see the readiness tests.)
    let app = common::TestApp::new().await;
    let (status, body) = app
        .request(Method::GET, "/api/health/live", None, None)
        .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "down");
    assert_eq!(body["database"]["ok"], false);
}

#[tokio::test]
async fn health_live_is_public() {
    let app = common::TestApp::new().await;
    let (status, _body) = app
        .request(Method::GET, "/api/health/live", None, None)
        .await;
    // No auth header required — probe still reaches the handler (returns 503
    // for the test pool, but the routing layer accepts it).
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// --- TMAIL-310: readiness probe ---

#[tokio::test]
async fn health_ready_returns_503_when_components_unhealthy() {
    let app = common::TestApp::new().await;
    let (status, body) = app
        .request(Method::GET, "/api/health/ready", None, None)
        .await;

    // TestApp has a dead DB pool + passthrough Redis + un-ticked queue, so
    // readiness must fail on every component and the HTTP code must be 503
    // (so load balancers actually drain traffic from a broken instance).
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["database"]["ok"], false);
    assert_eq!(body["mailboxes"]["ok"], false);
    assert_eq!(body["redis"]["ok"], false);
    assert_eq!(body["queue"]["ok"], false);
}

#[tokio::test]
async fn health_ready_redis_disconnected_message_is_actionable() {
    // The `detail` field is meant for on-call dashboards — assert it carries
    // a hint instead of being empty, so the operator knows WHY redis is down.
    let app = common::TestApp::new().await;
    let (_status, body) = app
        .request(Method::GET, "/api/health/ready", None, None)
        .await;

    let redis_detail = body["redis"]["detail"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        !redis_detail.is_empty(),
        "redis component should carry an actionable detail string: {:?}",
        body["redis"]
    );
}
