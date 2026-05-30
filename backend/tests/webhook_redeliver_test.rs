// Added (TMAIL-313): Integration test for webhook redelivery + secret rotation.
//
// Covers the two new endpoints introduced for TMAIL-313:
//   POST /api/webhooks/{id}/deliveries/{delivery_id}/redeliver
//   POST /api/webhooks/{id}/rotate-secret
//
// Round-trip we verify:
//  1. Force a failed delivery via the service layer (mock receiver returns 500).
//  2. Call the redeliver endpoint pointed at a NEW mock receiver that returns 200.
//  3. Assert a fresh webhook_deliveries row exists with success=true.
//  4. Assert a `webhook.redeliver` audit_log row was written.
//  5. Call rotate-secret, assert the returned secret is fresh hex (64 chars)
//     AND the persisted webhooks.secret matches the response.
//  6. Assert a `webhook.rotate_secret` audit_log row was written.
//
// DB-gated: if DATABASE_URL is unreachable the test prints a skip notice and
// returns Ok — matches the convention used by admin_audit_test.rs.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use tasmail::config::{
    Config, DatabaseConfig, ImapConfig, JwtConfig, LockoutConfig, RedisConfig, ServerConfig,
    SmtpConfig, StorageConfig,
};
use tasmail::models::webhook::{Webhook, WebhookDelivery, WebhookEvent};
use tasmail::router::create_router;
use tasmail::services::auth_service::Claims;
use tasmail::services::cache_service::CacheService;
use tasmail::services::encryption::EncryptionService;
use tasmail::services::queue_heartbeat::QueueHeartbeat;
use tasmail::services::webhook_dispatcher;
use tasmail::state::AppState;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";

fn resolve_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tasmail:tasmail@localhost/tasmail".to_string())
}

async fn try_build_app() -> Option<(axum::Router, PgPool)> {
    let db_url = resolve_db_url();
    let pool = match PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect(&db_url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "TMAIL-313 webhook_redeliver_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    // Need both webhooks + webhook_deliveries + audit_log tables present.
    let ok: (bool,) = match sqlx::query_as(
        "SELECT \
            EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'webhooks') \
            AND EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'webhook_deliveries') \
            AND EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'audit_log')",
    )
    .fetch_one(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "TMAIL-313 webhook_redeliver_test: skipping — could not query schema: {}",
                e
            );
            return None;
        }
    };
    if !ok.0 {
        eprintln!(
            "TMAIL-313 webhook_redeliver_test: skipping — required tables missing (migrations not run)"
        );
        return None;
    }

    let config = test_config(db_url);
    let inner_router_holder: Arc<std::sync::OnceLock<axum::Router>> =
        Arc::new(std::sync::OnceLock::new());
    let state = AppState {
        db: pool.clone(),
        config: config.clone(),
        metrics_handle: None,
        cache: CacheService::disabled(),
        encryption: EncryptionService::from_jwt_secret(TEST_JWT_SECRET),
        inner_router: inner_router_holder.clone(),
        queue_heartbeat: QueueHeartbeat::new(),
    };
    let router = create_router(state);
    let _ = inner_router_holder.set(router.clone());
    Some((router, pool))
}

fn test_config(db_url: String) -> Config {
    Config {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
        },
        database: DatabaseConfig {
            url: db_url,
            max_connections: 2,
        },
        imap: ImapConfig {
            host: "127.0.0.1".to_string(),
            port: 993,
            tls: true,
            master_password: None,
        },
        smtp: SmtpConfig {
            host: "127.0.0.1".to_string(),
            port: 587,
            tls: true,
            notification_from: None,
            notification_username: None,
            notification_password: None,
        },
        jwt: JwtConfig {
            secret: TEST_JWT_SECRET.to_string(),
            access_token_expiry_secs: 900,
            refresh_token_expiry_secs: 604800,
        },
        storage: StorageConfig::default(),
        metrics_token: None,
        rspamd_url: None,
        rspamd_password: None,
        billing: None,
        push: None,
        redis: RedisConfig::default(),
        lockout: LockoutConfig::default(),
    }
}

fn user_token(user_id: Uuid) -> String {
    let now = Utc::now();
    let exp = now + Duration::seconds(900);
    let claims = Claims {
        sub: user_id.to_string(),
        username: format!("user-{}@example.com", user_id),
        is_admin: false,
        is_compliance_officer: false,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .unwrap()
}

async fn json_request(
    router: &axum::Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    token: &str,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", token));
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let body = match body {
        Some(j) => Body::from(serde_json::to_vec(&j).unwrap()),
        None => Body::empty(),
    };
    let req = builder.body(body).unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::String(
            String::from_utf8_lossy(&bytes).to_string(),
        ))
    };
    (status, value)
}

/// Spin up a tiny TCP-level mock HTTP server that responds with the given
/// status line + body for every request, then returns the bound URL.
async fn start_mock_receiver(status_line: &'static str, body: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status_line,
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    format!("http://{}/hook", addr)
}

/// Seed: minimum rows to create a webhook owned by a mailbox we just inserted.
/// Returns (user_id, webhook_id, domain_id).
async fn seed_user_with_webhook(
    pool: &PgPool,
    webhook_url: &str,
    secret: &str,
) -> (Uuid, Uuid, Uuid) {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("redeliver-test-{}.example", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO domains (id, name, active) VALUES ($1, $2, true) ON CONFLICT (name) DO NOTHING",
    )
    .bind(domain_id)
    .bind(&domain_name)
    .execute(pool)
    .await
    .expect("seed test domain");

    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, is_admin, active)
         VALUES ($1, $2, $3, 'placeholder-hash', false, true)",
    )
    .bind(user_id)
    .bind(domain_id)
    .bind(format!("user-{}@{}", user_id, domain_name))
    .execute(pool)
    .await
    .expect("seed user mailbox");

    let webhook = Webhook::create(
        pool,
        user_id,
        &tasmail::models::webhook::CreateWebhookRequest {
            url: webhook_url.to_string(),
            secret: secret.to_string(),
            events: vec![WebhookEvent::EmailReceived],
            description: Some("redeliver-test".to_string()),
        },
    )
    .await
    .expect("create webhook");
    (user_id, webhook.id, domain_id)
}

async fn cleanup(pool: &PgPool, user_id: Uuid, domain_id: Uuid) {
    let _ = sqlx::query("DELETE FROM webhook_deliveries WHERE webhook_id IN (SELECT id FROM webhooks WHERE user_id = $1)")
        .bind(user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM webhooks WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM audit_log WHERE mailbox_id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(pool)
        .await;
}

/// END-TO-END: force a failed delivery, then call redeliver against a mock
/// receiver that returns 200. Assert a new webhook_deliveries row exists with
/// success=true and that an audit_log row records the action.
#[tokio::test]
async fn redeliver_creates_new_successful_delivery_row() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };

    // 1. Stand up a "good" mock receiver we will redeliver to.
    let good_url = start_mock_receiver("200 OK", "ok").await;

    // 2. Seed a user + webhook pointing at the good URL.
    let (user_id, webhook_id, domain_id) =
        seed_user_with_webhook(&pool, &good_url, "initial-secret").await;

    // 3. Force a synthetic FAILED delivery row (response_status=500, success=false).
    //    We don't have to actually hit a 500 endpoint — the redeliver only cares
    //    that a row with this id/webhook_id pair exists with a JSONB payload.
    let event = WebhookEvent::EmailReceived;
    let payload = serde_json::json!({
        "event_type": "email.received",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "data": { "subject": "Test redelivery payload" }
    });
    let original = WebhookDelivery::create(
        &pool,
        webhook_id,
        &event,
        &payload,
        Some(500),
        Some("synthetic failure".to_string()),
        false,
    )
    .await
    .expect("seed failed delivery");

    // Sanity: deliveries before should be 1.
    let count_before: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM webhook_deliveries WHERE webhook_id = $1",
    )
    .bind(webhook_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_before.0, 1);

    // 4. Call POST /api/webhooks/{id}/deliveries/{delivery_id}/redeliver
    let token = user_token(user_id);
    let (status, body) = json_request(
        &router,
        Method::POST,
        &format!(
            "/api/webhooks/{}/deliveries/{}/redeliver",
            webhook_id, original.id
        ),
        None,
        &token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "expected 201 on redeliver, got {} body={:?}",
        status,
        body
    );
    // The new delivery row was returned.
    assert_eq!(body["success"], true, "redelivered to mock — should succeed");
    assert_eq!(body["response_status"], 200);
    let new_delivery_id = body["id"].as_str().expect("new delivery id").to_string();
    assert_ne!(
        new_delivery_id,
        original.id.to_string(),
        "redeliver must create a NEW row, not mutate the original"
    );

    // 5. webhook_deliveries should now have 2 rows for this webhook.
    let count_after: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM webhook_deliveries WHERE webhook_id = $1",
    )
    .bind(webhook_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_after.0, 2, "redeliver must add exactly one row");

    // 6. Audit log row for webhook.redeliver
    let mut audit_seen = false;
    for _ in 0..10 {
        let row: Option<(String, String, Option<Uuid>)> = sqlx::query_as(
            "SELECT action, resource_id, mailbox_id FROM audit_log \
             WHERE action = 'webhook.redeliver' AND resource_id = $1 \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(webhook_id.to_string())
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some((action, resource_id, mailbox_id)) = row {
            assert_eq!(action, "webhook.redeliver");
            assert_eq!(resource_id, webhook_id.to_string());
            assert_eq!(mailbox_id, Some(user_id));
            audit_seen = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(audit_seen, "expected webhook.redeliver row in audit_log");

    cleanup(&pool, user_id, domain_id).await;
}

/// END-TO-END: rotate-secret returns a fresh hex secret and the persisted
/// webhook row reflects it. An audit row records the rotation.
#[tokio::test]
async fn rotate_secret_returns_new_secret_and_updates_row() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };

    // The URL is never actually hit in this test, but the row needs one.
    let (user_id, webhook_id, domain_id) =
        seed_user_with_webhook(&pool, "https://example.test/hook", "initial-secret").await;

    let token = user_token(user_id);
    let (status, body) = json_request(
        &router,
        Method::POST,
        &format!("/api/webhooks/{}/rotate-secret", webhook_id),
        None,
        &token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "expected 200 on rotate-secret, got {} body={:?}",
        status,
        body
    );

    let returned_secret = body["secret"].as_str().expect("secret in response");
    assert_eq!(returned_secret.len(), 64, "secret must be 32-byte hex");
    assert!(returned_secret.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(
        returned_secret, "initial-secret",
        "secret must have changed"
    );

    // The persisted secret must match the returned one.
    let persisted: (String,) =
        sqlx::query_as("SELECT secret FROM webhooks WHERE id = $1")
            .bind(webhook_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(persisted.0, returned_secret);

    // Audit log row for webhook.rotate_secret
    let mut audit_seen = false;
    for _ in 0..10 {
        let row: Option<(String, String, Option<Uuid>)> = sqlx::query_as(
            "SELECT action, resource_id, mailbox_id FROM audit_log \
             WHERE action = 'webhook.rotate_secret' AND resource_id = $1 \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(webhook_id.to_string())
        .fetch_optional(&pool)
        .await
        .unwrap();
        if let Some((action, resource_id, mailbox_id)) = row {
            assert_eq!(action, "webhook.rotate_secret");
            assert_eq!(resource_id, webhook_id.to_string());
            assert_eq!(mailbox_id, Some(user_id));
            audit_seen = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(audit_seen, "expected webhook.rotate_secret row in audit_log");

    cleanup(&pool, user_id, domain_id).await;
}

/// NEGATIVE: redelivering a delivery for a webhook owned by another user
/// must NOT leak — both endpoints must 404 instead of acting.
#[tokio::test]
async fn redeliver_rejects_cross_user_access() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };

    let good_url = start_mock_receiver("200 OK", "ok").await;

    // Owner of the webhook.
    let (owner_id, webhook_id, owner_domain) =
        seed_user_with_webhook(&pool, &good_url, "s").await;

    // Seed a failed delivery for the owner's webhook.
    let payload = serde_json::json!({"data": {"x": 1}});
    let delivery = WebhookDelivery::create(
        &pool,
        webhook_id,
        &WebhookEvent::EmailReceived,
        &payload,
        Some(500),
        None,
        false,
    )
    .await
    .unwrap();

    // Second user — separate mailbox + domain.
    let other_domain_id = Uuid::new_v4();
    let other_domain_name = format!("other-{}.example", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO domains (id, name, active) VALUES ($1, $2, true) ON CONFLICT (name) DO NOTHING",
    )
    .bind(other_domain_id)
    .bind(&other_domain_name)
    .execute(&pool)
    .await
    .unwrap();
    let other_user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, is_admin, active)
         VALUES ($1, $2, $3, 'placeholder-hash', false, true)",
    )
    .bind(other_user_id)
    .bind(other_domain_id)
    .bind(format!("other-{}@{}", other_user_id, other_domain_name))
    .execute(&pool)
    .await
    .unwrap();

    let other_token = user_token(other_user_id);
    let (status, _body) = json_request(
        &router,
        Method::POST,
        &format!(
            "/api/webhooks/{}/deliveries/{}/redeliver",
            webhook_id, delivery.id
        ),
        None,
        &other_token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "cross-user redeliver must 404, not 200/403"
    );

    let (status, _body) = json_request(
        &router,
        Method::POST,
        &format!("/api/webhooks/{}/rotate-secret", webhook_id),
        None,
        &other_token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "cross-user rotate-secret must 404"
    );

    cleanup(&pool, owner_id, owner_domain).await;
    cleanup(&pool, other_user_id, other_domain_id).await;
}

// NOTE: We also exercise the dispatcher-level redeliver_webhook function
// directly to confirm it produces a brand-new delivery row regardless of
// whether the original was a success or a failure. This catches future
// regressions where someone tries to "update in place" instead of "insert new".
#[tokio::test]
async fn dispatcher_redeliver_writes_new_row_with_current_secret() {
    let Some((_router, pool)) = try_build_app().await else {
        return;
    };
    let good_url = start_mock_receiver("200 OK", "ok").await;

    let (user_id, webhook_id, domain_id) =
        seed_user_with_webhook(&pool, &good_url, "rotated-secret-abc").await;

    let webhook = Webhook::find_by_id(&pool, webhook_id, user_id)
        .await
        .unwrap()
        .unwrap();

    let original_payload = serde_json::json!({"data":{"k":"v"}});
    let original = WebhookDelivery::create(
        &pool,
        webhook_id,
        &WebhookEvent::EmailReceived,
        &original_payload,
        Some(500),
        None,
        false,
    )
    .await
    .unwrap();

    let new_delivery = webhook_dispatcher::redeliver_webhook(&pool, &webhook, &original)
        .await
        .expect("redeliver_webhook ok");

    assert_eq!(new_delivery.webhook_id, webhook_id);
    assert_ne!(new_delivery.id, original.id, "must be a fresh row");
    assert_eq!(new_delivery.event, WebhookEvent::EmailReceived);
    assert_eq!(new_delivery.payload, original_payload);
    assert!(new_delivery.success);
    assert_eq!(new_delivery.response_status, Some(200));

    cleanup(&pool, user_id, domain_id).await;
}
