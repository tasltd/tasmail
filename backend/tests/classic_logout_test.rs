// TMAIL-360 — Integration tests for POST /classic/logout against a live
// PostgreSQL database.
//
// Coverage:
//   * Happy path — POST with valid session cookie + matching _csrf form
//     field → 303 to /classic/login, Set-Cookie clears the session cookie,
//     and the `classic_sessions` row is gone from the DB.
//   * GET /classic/logout → 405 Method Not Allowed (logout is POST-only by
//     design — see the module-level comment on handlers::classic::logout
//     for the CSRF/drive-by reasoning).
//   * POST without cookie → bounces to /classic/login (the session
//     middleware short-circuits before CSRF + handler even run).
//   * POST with valid cookie but missing _csrf field → 403 HTML retry page,
//     session row STAYS in the DB.
//   * POST with valid cookie but mismatched _csrf token → 403 HTML retry
//     page, session row STAYS in the DB.
//
// The tests are DB-gated: if DATABASE_URL is unreachable (e.g. CI without
// a PG service), each test logs and returns Ok rather than failing the
// suite — same convention as `admin_audit_test.rs` (TMAIL-307).
//
// Test isolation: every test seeds its own domain + mailbox + session
// keyed by a fresh UUID so concurrent runs never collide on the unique
// mailboxes.username index. Cleanup at end-of-test removes the seeded
// rows so the test DB doesn't accumulate cruft across runs.

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
use tasmail::services::cache_service::CacheService;
use tasmail::services::encryption::EncryptionService;
use tasmail::services::queue_heartbeat::QueueHeartbeat;
use tasmail::state::AppState;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";
const CLASSIC_SESSION_COOKIE: &str = "tasmail_classic_sid";

/// Resolve the DB URL to use — env wins, project default fallback.
fn resolve_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tasmail:tasmail@localhost/tasmail".to_string())
}

/// Build a real-DB AppState + router. Returns None when the DB isn't
/// reachable so each test can skip cleanly without failing the suite.
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
                "TMAIL-360 classic_logout_test: skipping — DB unreachable at {}: {}",
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
                "TMAIL-360 classic_logout_test: skipping — schema query failed: {}",
                e
            );
            return None;
        }
    };
    if !exists.0 {
        eprintln!(
            "TMAIL-360 classic_logout_test: skipping — classic_sessions table missing \
             (migration 080 not applied)"
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
            // NOTE: the cookie signing key is derived from this secret —
            // the middleware computes HMAC-SHA256(jwt_secret, session_id)
            // and embeds the base64url-encoded result in the cookie body.
            // Using the same constant here as `build_cookie_value` calls
            // below keeps the signature verifiable end-to-end.
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

/// Seed a fresh domain + mailbox + classic_sessions row, returning everything
/// the test needs to build a cookie and assert post-conditions. The
/// `cleanup` closure deletes the mailbox (CASCADE removes the domain via
/// `mailboxes.domain_id` and any sessions via `classic_sessions.user_id`).
struct SeededSession {
    domain_id: Uuid,
    user_id: Uuid,
    session: ClassicSession,
    csrf_token: String,
}

async fn seed_session(pool: &PgPool) -> SeededSession {
    // Unique domain + mailbox per test so concurrent test runs don't fight
    // over the unique mailboxes.username index.
    let domain_id = Uuid::new_v4();
    let domain_name = format!("logout-test-{}.example", domain_id);
    sqlx::query(
        "INSERT INTO domains (id, name, active) VALUES ($1, $2, true)",
    )
    .bind(domain_id)
    .bind(&domain_name)
    .execute(pool)
    .await
    .expect("seed test domain");

    let user_id = Uuid::new_v4();
    let username = format!("user-{}@{}", user_id, domain_name);
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, active, is_admin)
         VALUES ($1, $2, $3, 'placeholder-hash-not-used-in-logout-flow', true, false)",
    )
    .bind(user_id)
    .bind(domain_id)
    .bind(&username)
    .execute(pool)
    .await
    .expect("seed test mailbox");

    // CSRF token is 43-char URL-safe base64 (32 bytes), matching the shape
    // `auth::generate_csrf_token` produces. Using a fixed-but-unique value
    // per test keeps assertions stable.
    let csrf_token = format!(
        "csrftok{:032x}{:04x}",
        Uuid::new_v4().as_u128(),
        rand::random::<u16>()
    );
    let session = ClassicSession::create(pool, user_id, &csrf_token, Some("127.0.0.1"), Some("test-ua"))
        .await
        .expect("seed classic_sessions row");

    SeededSession {
        domain_id,
        user_id,
        session,
        csrf_token,
    }
}

async fn cleanup_seeded(pool: &PgPool, seeded: &SeededSession) {
    // Mailbox CASCADE removes the session row AND the domain link.
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(seeded.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(seeded.domain_id)
        .execute(pool)
        .await;
}

async fn body_to_string(resp: axum::http::Response<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

/// Find a single Set-Cookie line by name.
fn find_set_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    for v in headers.get_all(header::SET_COOKIE) {
        if let Ok(s) = v.to_str()
            && s.starts_with(&prefix)
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Build a signed cookie value the middleware will accept — uses the same
/// `build_cookie_value` helper the live login path uses, so this test
/// catches any divergence between cookie minting and cookie verification.
fn signed_cookie_value(session_id: Uuid) -> String {
    build_cookie_value(TEST_JWT_SECRET, session_id)
}

// ----- Happy path -----

#[tokio::test]
async fn happy_path_post_logout_destroys_session_and_clears_cookie() {
    let Some((router, pool)) = try_build_app().await else {
        return; // skipped — DB unreachable
    };
    let seeded = seed_session(&pool).await;

    // Build the request: valid signed cookie + matching _csrf form field.
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");
    let form_body = format!("_csrf={}", seeded.csrf_token);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/logout")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_header)
        .body(Body::from(form_body))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, headers, body) = body_to_string(resp).await;

    // 303 See Other → /classic/login.
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "expected 303, got {} body={}",
        status,
        body
    );
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/classic/login"),
        "Location header must point at /classic/login"
    );

    // Cookie cleared (Max-Age=0).
    let clear = find_set_cookie(&headers, CLASSIC_SESSION_COOKIE)
        .expect("logout MUST set a Set-Cookie clearing the session cookie");
    assert!(
        clear.contains("Max-Age=0"),
        "session-cookie Set-Cookie must use Max-Age=0 to clear it; got: {clear}"
    );
    assert!(clear.contains("HttpOnly"), "clear cookie must remain HttpOnly: {clear}");

    // Cache-Control on the redirect itself so a back-button hit doesn't
    // surface a stale snapshot of the inbox.
    assert!(
        headers
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("no-store"),
        "logout redirect MUST carry Cache-Control: no-store"
    );

    // The DB row is gone.
    let lookup = ClassicSession::find_active(&pool, seeded.session.id)
        .await
        .expect("classic_sessions lookup query runs");
    assert!(
        lookup.is_none(),
        "classic_sessions row MUST be deleted by POST /classic/logout; \
         found leftover row id={}",
        seeded.session.id
    );

    cleanup_seeded(&pool, &seeded).await;
}

// ----- POST-only invariant -----

#[tokio::test]
async fn get_logout_with_valid_session_does_not_destroy_session() {
    // The whole point of POST-only logout is to defeat drive-by sign-outs
    // via `<img src="/classic/logout">` in a hostile email, browser
    // pre-fetch, or search-engine crawl. The invariant to lock down is
    // therefore not "axum returns 405" — that's an implementation detail
    // and the session middleware happily 303-bounces unauthenticated
    // GETs to login (which is fine: no logout, no harm).
    //
    // What MUST hold is: when an authenticated user (valid session
    // cookie) hits GET /classic/logout, the session row stays put.
    // axum returns 405 because the route is POST-only — but even if a
    // future refactor accidentally turned the route into `.route("...",
    // get(post_logout).post(post_logout))`, this test would catch it
    // because the row would disappear.
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/logout")
        .header(header::COOKIE, cookie_header)
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();

    // Belt-and-braces: GET MUST NOT be 200 or 303 (anything that suggests
    // the logout succeeded). Allowed: 4xx (preferred) — anything else
    // would mean a GET handler exists somewhere it shouldn't.
    assert!(
        status.is_client_error(),
        "GET /classic/logout with a valid session MUST be a 4xx error \
         (axum returns 405 from the POST-only route), got {}",
        status
    );

    // The security invariant: the session row MUST still exist.
    let lookup = ClassicSession::find_active(&pool, seeded.session.id)
        .await
        .expect("classic_sessions lookup runs");
    assert!(
        lookup.is_some(),
        "session row MUST survive a GET to /classic/logout — drive-by \
         logout defence has regressed"
    );

    cleanup_seeded(&pool, &seeded).await;
}

// ----- No-session path -----

#[tokio::test]
async fn post_logout_without_cookie_redirects_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/logout")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("_csrf=anything"))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, headers, _body) = body_to_string(resp).await;

    // The session middleware bounces no-cookie requests to login before
    // CSRF or the handler even runs. Status: 303.
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "no-cookie POST MUST bounce to /classic/login (status 303)"
    );
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/classic/login"),
        "Location header must point at /classic/login"
    );
}

// ----- CSRF defence -----

#[tokio::test]
async fn post_logout_with_valid_cookie_but_missing_csrf_is_403() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/logout")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_header)
        // No _csrf field in the body — the CSRF middleware rejects.
        .body(Body::from("foo=bar"))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _headers, body) = body_to_string(resp).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "missing _csrf field MUST 403, got {} body={}",
        status,
        body
    );
    // CSRF rejection serves HTML, not JSON.
    assert!(
        body.contains("<html") || body.contains("<!DOCTYPE"),
        "CSRF rejection MUST be HTML, got: {body}"
    );

    // Session row MUST still exist — the handler never ran.
    let lookup = ClassicSession::find_active(&pool, seeded.session.id)
        .await
        .expect("classic_sessions lookup runs");
    assert!(
        lookup.is_some(),
        "session row MUST survive a CSRF rejection — the handler never ran"
    );

    cleanup_seeded(&pool, &seeded).await;
}

#[tokio::test]
async fn post_logout_with_mismatched_csrf_is_403_and_preserves_session() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");
    // Submit a DIFFERENT token from what's on the session row.
    let form_body = "_csrf=this-token-does-not-match-the-session-row";

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/logout")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_header)
        .body(Body::from(form_body))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _headers, body) = body_to_string(resp).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "mismatched _csrf MUST 403, got {} body={}",
        status,
        body
    );

    // Session row MUST still exist — handler never ran.
    let lookup = ClassicSession::find_active(&pool, seeded.session.id)
        .await
        .expect("classic_sessions lookup runs");
    assert!(
        lookup.is_some(),
        "session row MUST survive a CSRF rejection — the handler never ran"
    );

    cleanup_seeded(&pool, &seeded).await;
}

#[tokio::test]
async fn post_logout_with_forged_cookie_signature_is_rejected() {
    // Even if the attacker knows a real session id (it's a UUID — guessable
    // via brute force only at ~10^38 keyspace, but assume worst case), the
    // cookie's HMAC signature is derived from the JWT_SECRET. Without the
    // secret the signature can't be forged and the session middleware
    // bounces to login WITHOUT touching the row.
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;

    // Real session id, garbage signature.
    let cookie_val = format!("{}.this-is-not-a-valid-hmac", seeded.session.id.as_simple());
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");
    let form_body = format!("_csrf={}", seeded.csrf_token);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/logout")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookie_header)
        .body(Body::from(form_body))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _headers, _body) = body_to_string(resp).await;

    // Bounced to login by the session middleware.
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "forged-signature POST MUST bounce to login, got {}",
        status
    );

    // Session row survives — middleware short-circuited before delete.
    let lookup = ClassicSession::find_active(&pool, seeded.session.id)
        .await
        .expect("classic_sessions lookup runs");
    assert!(
        lookup.is_some(),
        "session row MUST survive a forged-cookie rejection"
    );

    cleanup_seeded(&pool, &seeded).await;
}
