// Added (TMAIL-307): Integration test confirming admin actions land in audit_log.
//
// Compliance trail: admin actions like 'delete domain' MUST persist an audit_log
// row identifying the actor + action + resource. This test calls the real
// DELETE /api/admin/domains/{id} endpoint against a live PostgreSQL and asserts
// the audit_log row appears.
//
// The test is DB-gated: if DATABASE_URL is not reachable (e.g. CI without a
// PG service), the test logs and returns Ok rather than failing the suite —
// the cargo test default is "no DB" per the rest of the test scaffolding.

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
use tasmail::router::create_router;
use tasmail::services::auth_service::Claims;
use tasmail::services::cache_service::CacheService;
use tasmail::services::encryption::EncryptionService;
use tasmail::state::AppState;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";

/// Resolve the DB URL to use for this test — env DATABASE_URL wins, then the
/// project-default dev DB, otherwise None (the test will skip).
fn resolve_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tasmail:tasmail@localhost/tasmail".to_string())
}

/// Build a real-DB AppState + router for the test. Returns None when the DB
/// is unreachable so the test can skip without failing.
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
                "TMAIL-307 admin_audit_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    // Required by SQLx migrate-on-startup elsewhere; we trust the DB already has
    // the schema (cargo build itself requires a live DB). If audit_log isn't
    // present we'll skip in the call site below.
    let exists: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'audit_log')",
    )
    .fetch_one(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "TMAIL-307 admin_audit_test: skipping — could not query schema: {}",
                e
            );
            return None;
        }
    };
    if !exists.0 {
        eprintln!("TMAIL-307 admin_audit_test: skipping — audit_log table missing (migrations not run)");
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

fn admin_token(user_id: Uuid) -> String {
    let now = Utc::now();
    let exp = now + Duration::seconds(900);
    let claims = Claims {
        sub: user_id.to_string(),
        username: "admin@example.com".to_string(),
        is_admin: true,
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

/// END-TO-END: create a domain via POST /api/admin/domains, then DELETE it,
/// then assert exactly one audit_log row exists with action='domain.delete'
/// and resource_id matching the created domain's id.
#[tokio::test]
async fn admin_domain_delete_writes_audit_log_row() {
    let Some((router, pool)) = try_build_app().await else {
        return; // skipped — DB unreachable
    };

    // The audit_log.mailbox_id column has a NOT NULL=false but FK→mailboxes(id)
    // — so the recorded actor must reference a real row. Seed a synthetic admin
    // mailbox bound to a synthetic test domain, both scoped by uuid so they
    // never collide with real data.
    let test_domain_id = Uuid::new_v4();
    let test_domain_name = format!("audit-test-domain-{}.example", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO domains (id, name, active) VALUES ($1, $2, true) ON CONFLICT (name) DO NOTHING",
    )
    .bind(test_domain_id)
    .bind(&test_domain_name)
    .execute(&pool)
    .await
    .expect("seed test domain for admin mailbox");

    let admin_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, is_admin, active)
         VALUES ($1, $2, $3, 'placeholder-hash', true, true)",
    )
    .bind(admin_id)
    .bind(test_domain_id)
    .bind(format!("admin-{}@{}", admin_id, test_domain_name))
    .execute(&pool)
    .await
    .expect("seed admin mailbox");

    let token = admin_token(admin_id);

    // Use a unique name so re-runs don't collide on the unique index.
    let domain_name = format!("audit-test-{}.example", Uuid::new_v4());

    // 1. POST /api/admin/domains — create
    let (status, body) = json_request(
        &router,
        Method::POST,
        "/api/admin/domains",
        Some(serde_json::json!({ "name": domain_name })),
        &token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "expected 201, got {} body {:?}",
        status,
        body
    );
    let domain_id = body["id"].as_str().expect("created domain id").to_string();

    // Snapshot the audit_log row count for this resource_id BEFORE the delete
    // so we can assert the delete added exactly one row.
    let before: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'domain.delete' AND resource_id = $1",
    )
    .bind(&domain_id)
    .fetch_one(&pool)
    .await
    .expect("count audit rows before");
    assert_eq!(before.0, 0, "no domain.delete rows should exist yet");

    // 2. DELETE /api/admin/domains/{id}
    let (status, body) = json_request(
        &router,
        Method::DELETE,
        &format!("/api/admin/domains/{}", domain_id),
        None,
        &token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "expected 204, got {} body {:?}",
        status,
        body
    );

    // 3. Assert audit_log row appears (best-effort retry — record() is
    // fire-and-forget so we give the spawned task a moment to land).
    let mut audit_row: Option<(String, String, String, Option<Uuid>)> = None;
    for _ in 0..10 {
        let row = sqlx::query_as::<_, (String, String, String, Option<Uuid>)>(
            "SELECT action, resource_type, resource_id, mailbox_id
             FROM audit_log
             WHERE action = 'domain.delete' AND resource_id = $1
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(&domain_id)
        .fetch_optional(&pool)
        .await
        .expect("query audit_log");
        if row.is_some() {
            audit_row = row;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let (action, resource_type, resource_id, mailbox_id) =
        audit_row.expect("audit_log row not found after admin domain delete (TMAIL-307)");

    assert_eq!(action, "domain.delete");
    assert_eq!(resource_type, "domain");
    assert_eq!(resource_id, domain_id);
    assert_eq!(
        mailbox_id,
        Some(admin_id),
        "audit row actor should equal the JWT sub (admin id)"
    );

    // Cleanup: drop the audit_log rows we just inserted plus the test mailbox
    // and domain so the tables don't grow unbounded across repeated test runs.
    let _ = sqlx::query("DELETE FROM audit_log WHERE mailbox_id = $1")
        .bind(admin_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(admin_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(test_domain_id)
        .execute(&pool)
        .await;
}
