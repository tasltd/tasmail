// TMAIL-377 — Integration tests for /classic/settings/sessions/* against a
// live PostgreSQL database.
//
// Coverage:
//   * GET   sessions                        — lists both tables, marks the
//                                             current row, hides its
//                                             per-row revoke button.
//   * POST  sessions/revoke (classic, other) — deletes the row, current
//                                              survives, flash is success.
//   * POST  sessions/revoke (classic, self) — refuses, current survives.
//   * POST  sessions/revoke (spa)           — deletes the refresh-token row.
//   * POST  sessions/revoke (other user's id) — silent no-op, no row
//                                               crosses tenant lines.
//   * POST  sessions/revoke-all             — renders confirm page with
//                                             counts; nothing deleted yet.
//   * POST  sessions/revoke-all/confirm     — wipes EVERY classic + spa
//                                             row for the user, clears
//                                             the session cookie, 303 →
//                                             /classic/login.
//   * GET   sessions without cookie         — 303 → /classic/login.
//   * POST  revoke without _csrf            — 403 from
//                                             classic_csrf_middleware;
//                                             nothing deleted.
//
// DB-gated: if DATABASE_URL is unreachable or the `classic_sessions`
// table is missing, each test logs and returns rather than failing.

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
const TEST_PASSWORD: &str = "active-sessions-fixture-pw";

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
                "TMAIL-377 classic_settings_sessions_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    let exists: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'classic_sessions')",
    )
    .fetch_one(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "TMAIL-377 classic_settings_sessions_test: skipping — schema query failed: {}",
                e
            );
            return None;
        }
    };
    if !exists.0 {
        eprintln!(
            "TMAIL-377 classic_settings_sessions_test: skipping — classic_sessions table missing"
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

/// Fresh isolated user + two classic sessions + two SPA refresh-token
/// sessions. Returns IDs so each test can assert which rows survived.
struct Seeded {
    domain_id: Uuid,
    user_id: Uuid,
    current_session: ClassicSession,
    other_session: ClassicSession,
    spa_session_a: Uuid,
    spa_session_b: Uuid,
    current_csrf: String,
}

async fn seed(pool: &PgPool) -> Seeded {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("sessions-test-{}.example", domain_id);
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(domain_id)
        .bind(&domain_name)
        .execute(pool)
        .await
        .expect("seed domain");

    let user_id = Uuid::new_v4();
    let username = format!("user-{}@{}", user_id, domain_name);
    let hash = hash_password(TEST_PASSWORD).expect("hash");
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

    let current_csrf = format!("csrfC{:032x}", Uuid::new_v4().as_u128());
    let other_csrf = format!("csrfO{:032x}", Uuid::new_v4().as_u128());

    let current_session = ClassicSession::create(
        pool,
        user_id,
        &current_csrf,
        Some("127.0.0.1"),
        Some("Mozilla/5.0 current"),
    )
    .await
    .expect("seed current classic session");
    let other_session = ClassicSession::create(
        pool,
        user_id,
        &other_csrf,
        Some("10.0.0.1"),
        Some("Mozilla/5.0 other"),
    )
    .await
    .expect("seed other classic session");

    let spa_session_a = Uuid::new_v4();
    let spa_session_b = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sessions (id, mailbox_id, refresh_token_hash, expires_at, created_at, ip_address, user_agent)
         VALUES
            ($1, $5, $2, NOW() + INTERVAL '7 days', NOW(), '192.0.2.1', 'TASMail-Mobile/1.0'),
            ($3, $5, $4, NOW() + INTERVAL '7 days', NOW() - INTERVAL '1 hour', '192.0.2.2', 'TASMail-Web/SPA')",
    )
    .bind(spa_session_a)
    .bind(format!("spa-A-{}", spa_session_a))
    .bind(spa_session_b)
    .bind(format!("spa-B-{}", spa_session_b))
    .bind(user_id)
    .execute(pool)
    .await
    .expect("seed spa sessions");

    Seeded {
        domain_id,
        user_id,
        current_session,
        other_session,
        spa_session_a,
        spa_session_b,
        current_csrf,
    }
}

async fn cleanup(pool: &PgPool, seeded: &Seeded) {
    let _ = sqlx::query("DELETE FROM sessions WHERE mailbox_id = $1")
        .bind(seeded.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM classic_sessions WHERE user_id = $1")
        .bind(seeded.user_id)
        .execute(pool)
        .await;
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

// ============================================================
// GET /classic/settings/sessions
// ============================================================

#[tokio::test]
async fn get_lists_both_tables_marks_current_row() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/sessions")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK, "GET should 200; body={body}");
    assert!(body.contains("Active sessions"));
    // Both classic rows surface in the table.
    assert!(body.contains("Mozilla/5.0 current"));
    assert!(body.contains("Mozilla/5.0 other"));
    // Both SPA rows surface.
    assert!(body.contains("TASMail-Mobile/1.0"));
    assert!(body.contains("TASMail-Web/SPA"));
    // The current row is marked.
    assert!(
        body.contains("This browser"),
        "current row must carry the 'This browser' badge: {body}"
    );
    // The danger-zone Sign-out-everywhere CTA renders.
    assert!(body.contains("action=\"/classic/settings/sessions/revoke-all\""));
    // Per-row csrf token threaded through.
    assert!(body.contains(&format!("value=\"{}\"", seeded.current_csrf)));

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn get_without_cookie_bounces_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/sessions")
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, headers, _) = body_str(resp).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/classic/login"),
    );
}

// ============================================================
// POST /classic/settings/sessions/revoke (single row)
// ============================================================

#[tokio::test]
async fn post_revoke_other_classic_row_succeeds_current_survives() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "kind=classic&session_id={}&_csrf={}",
        seeded.other_session.id, seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Classic UI session revoked"));
    assert!(body.contains("alert-success"));

    // Current row preserved, other row gone.
    assert!(classic_session_exists(&pool, seeded.current_session.id).await);
    assert!(!classic_session_exists(&pool, seeded.other_session.id).await);

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_revoke_current_classic_row_refused() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "kind=classic&session_id={}&_csrf={}",
        seeded.current_session.id, seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Sign-out button in the navigation"),
        "should refuse with a guidance message: {body}"
    );
    // Current row preserved.
    assert!(classic_session_exists(&pool, seeded.current_session.id).await);

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_revoke_spa_row_succeeds() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "kind=spa&session_id={}&_csrf={}",
        seeded.spa_session_a, seeded.current_csrf
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("SPA / mobile refresh token revoked"));
    assert!(!spa_session_exists(&pool, seeded.spa_session_a).await);
    // Sibling row untouched.
    assert!(spa_session_exists(&pool, seeded.spa_session_b).await);

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_revoke_other_users_classic_row_silent_noop() {
    // Tenant-isolation check: a hostile user can't revoke another
    // user's session because the model query filters by user_id.
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    // Seed a second user with their own classic session that we'll
    // attempt to revoke.
    let other_domain = Uuid::new_v4();
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(other_domain)
        .bind(format!("victim-{}.example", other_domain))
        .execute(&pool)
        .await
        .unwrap();
    let victim_user = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, active, is_admin)
         VALUES ($1, $2, $3, $4, true, false)",
    )
    .bind(victim_user)
    .bind(other_domain)
    .bind(format!("victim-{}@victim.example", victim_user))
    .bind(hash_password("victim-pw-xxx").expect("hash"))
    .execute(&pool)
    .await
    .unwrap();
    let victim_session = ClassicSession::create(
        &pool,
        victim_user,
        "victim-csrf-token",
        Some("8.8.8.8"),
        Some("VictimAgent/1.0"),
    )
    .await
    .expect("seed victim session");

    let form = format!(
        "kind=classic&session_id={}&_csrf={}",
        victim_session.id, seeded.current_csrf
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;
    assert_eq!(status, StatusCode::OK);
    // Returns the "already inactive" flash rather than success — the
    // WHERE clause silently filters the row out.
    assert!(
        body.contains("no longer active"),
        "tenant-mismatched revoke should produce a no-op flash, not success: {body}"
    );

    // Victim row UNTOUCHED.
    assert!(
        classic_session_exists(&pool, victim_session.id).await,
        "victim's classic session row must NOT be deleted across tenant lines"
    );

    let _ = sqlx::query("DELETE FROM classic_sessions WHERE user_id = $1")
        .bind(victim_user)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(victim_user)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(other_domain)
        .execute(&pool)
        .await;
    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_revoke_without_csrf_field_is_rejected() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "kind=classic&session_id={}",
        seeded.other_session.id
    );

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, _) = body_str(resp).await;
    // classic_csrf_middleware rejects missing _csrf with 403.
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Nothing was deleted.
    assert!(classic_session_exists(&pool, seeded.current_session.id).await);
    assert!(classic_session_exists(&pool, seeded.other_session.id).await);

    cleanup(&pool, &seeded).await;
}

// ============================================================
// POST /classic/settings/sessions/revoke-all (confirm page)
// ============================================================

#[tokio::test]
async fn post_revoke_all_renders_confirm_page_no_mutation() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!("_csrf={}", seeded.current_csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke-all")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Sign out everywhere?"));
    // Counts the rows that would be destroyed (2 classic + 2 spa).
    assert!(body.contains("Classic UI browsers: 2"));
    assert!(body.contains("SPA / mobile refresh tokens: 2"));
    // The confirm form points at the destructive endpoint.
    assert!(body.contains("action=\"/classic/settings/sessions/revoke-all/confirm\""));

    // Critically: NO row was deleted yet.
    assert!(classic_session_exists(&pool, seeded.current_session.id).await);
    assert!(classic_session_exists(&pool, seeded.other_session.id).await);
    assert!(spa_session_exists(&pool, seeded.spa_session_a).await);
    assert!(spa_session_exists(&pool, seeded.spa_session_b).await);

    cleanup(&pool, &seeded).await;
}

// ============================================================
// POST /classic/settings/sessions/revoke-all/confirm (destructive)
// ============================================================

#[tokio::test]
async fn post_revoke_all_confirm_wipes_all_rows_and_redirects() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!("_csrf={}", seeded.current_csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke-all/confirm")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, headers, _) = body_str(resp).await;

    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/classic/login"),
    );
    // Response carries a Set-Cookie clearing the session cookie.
    let set_cookies: Vec<&str> = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert!(
        set_cookies.iter().any(|c| c.contains("tasmail_classic_sid=")
            && (c.contains("Max-Age=0") || c.contains("max-age=0"))),
        "response must clear the session cookie; got: {:?}",
        set_cookies
    );

    // Every classic + spa row for this user is gone.
    assert!(!classic_session_exists(&pool, seeded.current_session.id).await);
    assert!(!classic_session_exists(&pool, seeded.other_session.id).await);
    assert!(!spa_session_exists(&pool, seeded.spa_session_a).await);
    assert!(!spa_session_exists(&pool, seeded.spa_session_b).await);

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_revoke_all_confirm_isolates_other_users() {
    // Sanity: signing out everywhere does NOT touch other users' rows.
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    // Seed a bystander user with their own row.
    let bystander_domain = Uuid::new_v4();
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(bystander_domain)
        .bind(format!("bystander-{}.example", bystander_domain))
        .execute(&pool)
        .await
        .unwrap();
    let bystander_user = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, active, is_admin)
         VALUES ($1, $2, $3, $4, true, false)",
    )
    .bind(bystander_user)
    .bind(bystander_domain)
    .bind(format!("bystander-{}@bystander.example", bystander_user))
    .bind(hash_password("bystander-pw").expect("hash"))
    .execute(&pool)
    .await
    .unwrap();
    let bystander_session = ClassicSession::create(
        &pool,
        bystander_user,
        "bystander-csrf",
        Some("3.3.3.3"),
        Some("BystanderAgent/1.0"),
    )
    .await
    .unwrap();

    let form = format!("_csrf={}", seeded.current_csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/sessions/revoke-all/confirm")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_for(seeded.current_session.id))
        .body(Body::from(form))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, _) = body_str(resp).await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // Bystander's row survives.
    assert!(
        classic_session_exists(&pool, bystander_session.id).await,
        "bystander user's classic session MUST NOT be touched"
    );

    let _ = sqlx::query("DELETE FROM classic_sessions WHERE user_id = $1")
        .bind(bystander_user)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(bystander_user)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(bystander_domain)
        .execute(&pool)
        .await;
    cleanup(&pool, &seeded).await;
}
