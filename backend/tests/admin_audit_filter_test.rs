// TMAIL-352: integration test for paginated/date-filtered GET /api/admin/audit-log.
//
// Confirms the new query params (`from`, `to`, `offset`, `limit`) actually
// reach the SQL and that the `X-Total-Count` response header reports the
// pre-pagination count so the Modern UI's prev/next can render "Showing
// 1-N of TOTAL".
//
// DB-gated: when DATABASE_URL is unreachable, returns Ok without failing,
// matching the convention used by admin_audit_test.rs and the other
// real-DB integration tests in this folder.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
// Fix (TMAIL-360): `SubsecRound` is needed to truncate the test anchor to
// whole-second precision so the inclusive `from` filter doesn't lose a row
// to nanosecond rounding mismatches between the DateTime<Utc> binary bind
// path (insert) and the text-to-timestamptz cast path (filter SQL). See
// commit body for the trace.
use chrono::{Duration, SubsecRound, Utc};
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
use tasmail::services::queue_heartbeat::QueueHeartbeat;
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
                "TMAIL-352 admin_audit_filter_test: skipping — DB unreachable: {}",
                e
            );
            return None;
        }
    };

    let exists: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'audit_log')",
    )
    .fetch_one(&pool)
    .await
    {
        Ok(r) => r,
        Err(_) => return None,
    };
    if !exists.0 {
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
        // TMAIL-314: loopback-only fallback when None.
        metrics_allowed_ips: None,
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

async fn seed_admin(pool: &PgPool) -> (Uuid, Uuid) {
    let test_domain_id = Uuid::new_v4();
    let test_domain_name = format!("audit-filter-test-{}.example", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO domains (id, name, active) VALUES ($1, $2, true) ON CONFLICT (name) DO NOTHING",
    )
    .bind(test_domain_id)
    .bind(&test_domain_name)
    .execute(pool)
    .await
    .expect("seed test domain");

    let admin_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, is_admin, active)
         VALUES ($1, $2, $3, 'placeholder-hash', true, true)",
    )
    .bind(admin_id)
    .bind(test_domain_id)
    .bind(format!("admin-{}@{}", admin_id, test_domain_name))
    .execute(pool)
    .await
    .expect("seed admin mailbox");

    (admin_id, test_domain_id)
}

async fn seed_audit_rows(pool: &PgPool, mailbox_id: Uuid, action: &str, count: i32, base_ts: chrono::DateTime<Utc>) {
    // Audit inserts require app.is_admin=true on the connection (TMAIL-198).
    let mut conn = pool.acquire().await.expect("pool acquire");
    sqlx::query("SELECT set_config('app.is_admin', 'true', false)")
        .execute(&mut *conn)
        .await
        .expect("set is_admin");
    for i in 0..count {
        // Space each row 1 minute apart so the from/to filter in the
        // assertions has bite. Inserts must include explicit created_at so
        // we know which row should fall inside which window.
        let ts = base_ts + Duration::minutes(i as i64);
        sqlx::query(
            "INSERT INTO audit_log (mailbox_id, action, created_at)
             VALUES ($1, $2, $3)",
        )
        .bind(mailbox_id)
        .bind(action)
        .bind(ts)
        .execute(&mut *conn)
        .await
        .expect("insert audit row");
    }
}

async fn parse_logs(resp: axum::http::Response<Body>) -> (StatusCode, Option<i64>, Vec<Value>) {
    let status = resp.status();
    let total = resp
        .headers()
        .get("X-Total-Count")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    let arr = json.as_array().cloned().unwrap_or_default();
    (status, total, arr)
}

/// END-TO-END: seed 10 audit rows for a synthetic admin mailbox spread over
/// 10 consecutive minutes, then query GET /api/admin/audit-log with
/// `mailbox_id`, `action`, `from`, `to`, `limit`, and `offset` and assert:
///   * Filter narrows by date range
///   * Pagination (limit+offset) walks results in order
///   * X-Total-Count reports the full filtered count (pre-pagination)
#[tokio::test]
async fn audit_log_filter_pagination_and_total_count() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };

    let (admin_id, domain_id) = seed_admin(&pool).await;
    let token = admin_token(admin_id);

    let unique_action = format!("test.tmail352.{}", Uuid::new_v4().simple());
    // 10 rows, 1 minute apart, starting at a known anchor.
    //
    // Fix (TMAIL-360 follow-up): truncate the anchor to whole-second
    // precision before seeding. Without this, `Utc::now()` carries
    // nanosecond precision, and the inclusive `from` filter would
    // lose the boundary row to a sub-microsecond rounding mismatch:
    //   * `sqlx::query(...).bind(DateTime<Utc>)` (insert path) encodes
    //     the value in Postgres's binary protocol with microsecond
    //     precision — nanoseconds are TRUNCATED.
    //   * `from.to_rfc3339()` → `$N::timestamptz` (filter path) goes
    //     through Postgres's text-to-timestamptz parser which ROUNDS
    //     fractional seconds half-away-from-zero.
    //   So `anchor = 04:50:00.123456789` becomes stored as `04:50:00.123456`
    //   but `from = anchor + 5min` becomes `04:55:00.123457` after the
    //   text cast — and the row at exactly `+5min` (stored as
    //   `04:55:00.123456`) fails the `>= 04:55:00.123457` filter.
    //   That's the 5-vs-4 discrepancy the test was catching.
    //
    //   Truncating to whole seconds removes the ambiguity without
    //   touching the audit-log filter logic, which is correct: both
    //   the insert and the filter agree on the boundary value to the
    //   microsecond, so `>=` does the right thing.
    let anchor = (Utc::now() - Duration::hours(1)).trunc_subsecs(0);
    seed_audit_rows(&pool, admin_id, &unique_action, 10, anchor).await;

    // 1. List ALL with action filter — expect 10 rows, X-Total-Count=10.
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/admin/audit-log?action={}&limit=50",
            unique_action
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, total, rows) = parse_logs(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total, Some(10), "X-Total-Count should be 10");
    assert_eq!(rows.len(), 10, "should return all 10 seeded rows");

    // 2. Pagination — limit=4, offset=4 should return rows 4..8 (in DESC
    //    order, so the rows 5 minutes before the latest).
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/admin/audit-log?action={}&limit=4&offset=4",
            unique_action
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, total, rows) = parse_logs(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total, Some(10), "total count unchanged by pagination");
    assert_eq!(rows.len(), 4, "page size should be honoured");

    // 3. Date range — `from` set to anchor+5min should only see the last 5
    //    rows (anchor+5, +6, +7, +8, +9).
    let from = (anchor + Duration::minutes(5)).to_rfc3339();
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/admin/audit-log?action={}&from={}&limit=50",
            unique_action,
            urlencoding::encode(&from)
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, total, rows) = parse_logs(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total, Some(5), "date filter should reduce total to 5");
    assert_eq!(rows.len(), 5);

    // 4. mailbox_id filter — same admin → still 10 total.
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/admin/audit-log?action={}&mailbox_id={}&limit=50",
            unique_action, admin_id
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, total, _) = parse_logs(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total, Some(10));

    // 5. mailbox_id filter — random uuid → 0 rows.
    let other = Uuid::new_v4();
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/admin/audit-log?action={}&mailbox_id={}&limit=50",
            unique_action, other
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, total, rows) = parse_logs(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total, Some(0));
    assert!(rows.is_empty());

    // Cleanup.
    let _ = sqlx::query("DELETE FROM audit_log WHERE mailbox_id = $1")
        .bind(admin_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(admin_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(&pool)
        .await;
}

/// Non-admin token should get 403 from the new handler too.
#[tokio::test]
async fn audit_log_non_admin_forbidden() {
    let Some((router, _)) = try_build_app().await else {
        return;
    };
    let now = Utc::now();
    let exp = now + Duration::seconds(900);
    let claims = Claims {
        sub: Uuid::new_v4().to_string(),
        username: "user@example.com".to_string(),
        is_admin: false,
        is_compliance_officer: false,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .unwrap();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/admin/audit-log?limit=10")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
