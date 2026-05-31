// TMAIL-376 — Integration tests for GET / POST /classic/settings/password
// against a live PostgreSQL database.
//
// Coverage:
//   * GET happy path — renders the form, threads the session's csrf_token
//     into the hidden `_csrf` input + the logout-form partial.
//   * POST happy path — verifies current password, rotates the hash,
//     deletes every OTHER classic session AND every SPA refresh-token
//     `sessions` row for the user. The current classic session row is
//     preserved.
//   * POST with WRONG current password → form re-renders (400) with the
//     "current password is incorrect" banner; no DB rows are mutated.
//   * POST with mismatched new/confirm → form re-renders (400), no DB
//     mutation.
//   * POST with same new == current → form re-renders (400), no DB
//     mutation. (Catches the "I changed it but it didn't change" support
//     ticket.)
//   * POST without session cookie → bounces to /classic/login (the
//     classic_session_middleware short-circuits).
//   * POST without _csrf form field → 403 HTML retry page from
//     classic_csrf_middleware; both classic sessions stay alive.
//
// DB-gated: if DATABASE_URL is unreachable or the `classic_sessions`
// table is missing, each test logs and returns Ok() rather than failing
// the suite. Same convention as `classic_logout_test.rs` (TMAIL-360).

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use tasmail::config::{
    Config, DatabaseConfig, ImapConfig, JwtConfig, LockoutConfig, RedisConfig, ServerConfig,
    SmtpConfig, StorageConfig,
};
use tasmail::middleware::classic_session::build_cookie_value;
use tasmail::models::classic_session::ClassicSession;
use tasmail::router::create_router;
use tasmail::services::auth_service::hash_password;
use tasmail::services::cache_service::CacheService;
use tasmail::services::encryption::EncryptionService;
use tasmail::services::queue_heartbeat::QueueHeartbeat;
use tasmail::state::AppState;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";
const CLASSIC_SESSION_COOKIE: &str = "tasmail_classic_sid";
const CURRENT_PASSWORD: &str = "old-password-correct-horse";
const NEW_PASSWORD: &str = "fresh-password-rotated-9";

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
                "TMAIL-376 classic_settings_password_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    // Confirm the classic_sessions table is present — older databases
    // without migration 080 applied should skip rather than 500.
    let exists: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'classic_sessions')",
    )
    .fetch_one(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "TMAIL-376 classic_settings_password_test: skipping — schema query failed: {}",
                e
            );
            return None;
        }
    };
    if !exists.0 {
        eprintln!(
            "TMAIL-376 classic_settings_password_test: skipping — classic_sessions table missing"
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
        metrics_allowed_ips: None,
        rspamd_url: None,
        rspamd_password: None,
        billing: None,
        push: None,
        redis: RedisConfig::default(),
        lockout: LockoutConfig::default(),
    }
}

/// Seed: fresh domain, mailbox (with CURRENT_PASSWORD hashed), two
/// classic_sessions rows (current + a second concurrent browser), and
/// one SPA refresh-token `sessions` row. Returns IDs so the tests can
/// assert which rows survived a POST.
struct Seeded {
    domain_id: Uuid,
    user_id: Uuid,
    current_session: ClassicSession,
    other_session: ClassicSession,
    spa_session_id: Uuid,
    current_csrf: String,
}

async fn seed(pool: &PgPool) -> Seeded {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("pw-test-{}.example", domain_id);
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(domain_id)
        .bind(&domain_name)
        .execute(pool)
        .await
        .expect("seed domain");

    let user_id = Uuid::new_v4();
    let username = format!("user-{}@{}", user_id, domain_name);
    let hash = hash_password(CURRENT_PASSWORD).expect("hash");
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, active, is_admin)
         VALUES ($1, $2, $3, $4, true, false)",
    )
    .bind(user_id)
    .bind(domain_id)
    .bind(&username)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("seed mailbox");

    let current_csrf = format!("csrfA{:032x}", Uuid::new_v4().as_u128());
    let other_csrf = format!("csrfB{:032x}", Uuid::new_v4().as_u128());

    let current_session =
        ClassicSession::create(pool, user_id, &current_csrf, Some("127.0.0.1"), Some("ua-A"))
            .await
            .expect("seed current classic session");
    let other_session =
        ClassicSession::create(pool, user_id, &other_csrf, Some("10.0.0.1"), Some("ua-B"))
            .await
            .expect("seed other classic session");

    // One SPA refresh-token `sessions` row so we can assert it's gone
    // after a successful POST. We bypass auth_service::insert_session
    // (which sets RLS context) by using a raw INSERT — the test
    // doesn't need RLS, just the row existence.
    let spa_session_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sessions (id, mailbox_id, refresh_token_hash, expires_at, created_at)
         VALUES ($1, $2, $3, NOW() + INTERVAL '7 days', NOW())",
    )
    .bind(spa_session_id)
    .bind(user_id)
    .bind(format!("spa-test-hash-{}", spa_session_id))
    .execute(pool)
    .await
    .expect("seed spa session");

    Seeded {
        domain_id,
        user_id,
        current_session,
        other_session,
        spa_session_id,
        current_csrf,
    }
}

async fn cleanup(pool: &PgPool, seeded: &Seeded) {
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(seeded.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(seeded.domain_id)
        .execute(pool)
        .await;
}

async fn body_str(
    resp: axum::http::Response<Body>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

fn cookie_for(session_id: Uuid) -> String {
    format!(
        "{CLASSIC_SESSION_COOKIE}={}",
        build_cookie_value(TEST_JWT_SECRET, session_id)
    )
}

async fn classic_session_exists(pool: &PgPool, id: Uuid) -> bool {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM classic_sessions WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count classic_sessions");
    row.0 > 0
}

async fn spa_session_exists(pool: &PgPool, id: Uuid) -> bool {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count sessions");
    row.0 > 0
}

async fn stored_password_hash(pool: &PgPool, user_id: Uuid) -> String {
    let row: (String,) = sqlx::query_as("SELECT password_hash FROM mailboxes WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("read password_hash");
    row.0
}

// ============================================================
// GET /classic/settings/password
// ============================================================

#[tokio::test]
async fn get_renders_form_with_session_csrf() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/password")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK, "GET form should 200; body={body}");
    assert!(
        body.contains("action=\"/classic/settings/password\""),
        "form must POST back to /classic/settings/password: {body}"
    );
    assert!(
        body.contains(&format!("value=\"{}\"", seeded.current_csrf)),
        "session csrf_token must be threaded into the hidden _csrf field"
    );
    assert!(body.contains("name=\"current_password\""));
    assert!(body.contains("name=\"new_password\""));
    assert!(body.contains("name=\"confirm_password\""));
    // Logout form partial must render — the page is authenticated.
    assert!(body.contains("action=\"/classic/logout\""));
    // No alert banner on a fresh render.
    assert!(!body.contains("role=\"alert\""));

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn get_without_cookie_bounces_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/password")
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, headers, _) = body_str(resp).await;

    assert_eq!(status, StatusCode::SEE_OTHER, "no cookie → 303");
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/classic/login"),
    );
}

// ============================================================
// POST /classic/settings/password — happy path
// ============================================================

#[tokio::test]
async fn post_rotates_password_keeps_current_session_revokes_peers() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;
    let original_hash = stored_password_hash(&pool, seeded.user_id).await;

    let form = format!(
        "current_password={}&new_password={}&confirm_password={}&_csrf={}",
        CURRENT_PASSWORD, NEW_PASSWORD, NEW_PASSWORD, seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK, "happy POST should 200; body={body}");
    assert!(
        body.contains("Password updated"),
        "success page should be rendered: {body}"
    );

    // 1) The password hash was rotated.
    let new_hash = stored_password_hash(&pool, seeded.user_id).await;
    assert_ne!(
        original_hash, new_hash,
        "password_hash should change after a successful POST"
    );

    // 2) The CURRENT classic session row is preserved — the user stays
    //    signed in on this browser (matches the acceptance criterion).
    assert!(
        classic_session_exists(&pool, seeded.current_session.id).await,
        "current classic_sessions row MUST survive a password change"
    );

    // 3) The OTHER classic session row is gone — the second browser
    //    will be logged out on its next request.
    assert!(
        !classic_session_exists(&pool, seeded.other_session.id).await,
        "other concurrent classic_sessions row MUST be deleted"
    );

    // 4) Every SPA refresh token for this user is gone.
    assert!(
        !spa_session_exists(&pool, seeded.spa_session_id).await,
        "SPA refresh-token sessions row MUST be revoked"
    );

    cleanup(&pool, &seeded).await;
}

// ============================================================
// POST validation failures — DB must NOT mutate
// ============================================================

#[tokio::test]
async fn post_with_wrong_current_password_rerenders_form_no_mutation() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;
    let original_hash = stored_password_hash(&pool, seeded.user_id).await;

    let form = format!(
        "current_password=WRONG-pw&new_password={}&confirm_password={}&_csrf={}",
        NEW_PASSWORD, NEW_PASSWORD, seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("current password you entered is incorrect"),
        "form should re-render with the wrong-password error: {body}"
    );

    // No DB mutation.
    assert_eq!(stored_password_hash(&pool, seeded.user_id).await, original_hash);
    assert!(classic_session_exists(&pool, seeded.current_session.id).await);
    assert!(classic_session_exists(&pool, seeded.other_session.id).await);
    assert!(spa_session_exists(&pool, seeded.spa_session_id).await);

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_with_mismatched_new_confirm_rerenders_form_no_mutation() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;
    let original_hash = stored_password_hash(&pool, seeded.user_id).await;

    let form = format!(
        "current_password={}&new_password={}&confirm_password={}&_csrf={}",
        CURRENT_PASSWORD,
        NEW_PASSWORD,
        "different-confirm-9",
        seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("New passwords do not match"), "body={body}");

    // No DB mutation — the mismatch is caught BEFORE the password is
    // even verified (cheap validation first).
    assert_eq!(stored_password_hash(&pool, seeded.user_id).await, original_hash);
    assert!(classic_session_exists(&pool, seeded.other_session.id).await);
    assert!(spa_session_exists(&pool, seeded.spa_session_id).await);

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_with_short_new_password_rerenders_form() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "current_password={}&new_password=short&confirm_password=short&_csrf={}",
        CURRENT_PASSWORD, seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("at least 8 characters"),
        "should mention the 8-char minimum: {body}"
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_with_same_new_as_current_rerenders_form() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;
    let original_hash = stored_password_hash(&pool, seeded.user_id).await;

    let form = format!(
        "current_password={}&new_password={}&confirm_password={}&_csrf={}",
        CURRENT_PASSWORD, CURRENT_PASSWORD, CURRENT_PASSWORD, seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("different from your current password"),
        "no-op rotation should be rejected: {body}"
    );
    // Password unchanged.
    assert_eq!(stored_password_hash(&pool, seeded.user_id).await, original_hash);

    cleanup(&pool, &seeded).await;
}

// ============================================================
// POST middleware short-circuits (auth + CSRF)
// ============================================================

#[tokio::test]
async fn post_without_session_cookie_bounces_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let form = format!(
        "current_password={}&new_password={}&confirm_password={}&_csrf=anything",
        CURRENT_PASSWORD, NEW_PASSWORD, NEW_PASSWORD
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, headers, _) = body_str(resp).await;

    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/classic/login")
    );
}

#[tokio::test]
async fn post_with_missing_csrf_field_returns_403_no_mutation() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;
    let original_hash = stored_password_hash(&pool, seeded.user_id).await;

    // Form body intentionally has no _csrf field. classic_csrf_middleware
    // is expected to short-circuit before our handler runs.
    let form = format!(
        "current_password={}&new_password={}&confirm_password={}",
        CURRENT_PASSWORD, NEW_PASSWORD, NEW_PASSWORD
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, _body) = body_str(resp).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "missing _csrf must be rejected by middleware as 403"
    );

    // No DB mutation: every row that was present before is still there.
    assert_eq!(stored_password_hash(&pool, seeded.user_id).await, original_hash);
    assert!(classic_session_exists(&pool, seeded.current_session.id).await);
    assert!(classic_session_exists(&pool, seeded.other_session.id).await);
    assert!(spa_session_exists(&pool, seeded.spa_session_id).await);

    cleanup(&pool, &seeded).await;
}
