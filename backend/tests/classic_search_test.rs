// TMAIL-373 — Integration tests for GET /classic/search against a live
// PostgreSQL database.
//
// Coverage (no IMAP credentials are mocked — all assertions are on
// branches that DON'T touch IMAP):
//   * Unauthenticated GET → 303 to /classic/login (session middleware
//     short-circuits before the handler runs).
//   * Authenticated GET /classic/search (no `q`) → 200 HTML rendering
//     the empty-state landing page. Includes the search form, the
//     prompt copy, the base layout shell, AND echoes the empty query
//     into the nav-level search box.
//   * Authenticated GET /classic/search?q=  (whitespace-only) →
//     same empty-state render — server treats it as "no query".
//   * Authenticated GET /classic/search?folder=Drafts (no q) → empty
//     state preserves the folder selector value so the user can submit
//     a query against their chosen folder.
//   * Authenticated GET /classic/search?q=<crlf> → 400 BadRequest, the
//     validation layer rejects the IMAP-injection candidate BEFORE
//     reaching IMAP.
//
// The IMAP-execution branch (q present + matches / no-matches) is
// covered by the in-process Askama render tests in
// `handlers::classic::search::tests` — those don't need a live IMAP.
// An end-to-end IMAP test for the search proxy belongs in a Playwright
// spec since it needs a populated mailbox.
//
// DB-gated: if DATABASE_URL is unreachable, each test logs + returns
// rather than failing the suite. Same convention as classic_logout_test.

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
                "TMAIL-373 classic_search_test: skipping — DB unreachable at {}: {}",
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
        Err(_) => return None,
    };
    if !exists.0 {
        eprintln!(
            "TMAIL-373 classic_search_test: skipping — classic_sessions table missing"
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

struct SeededSession {
    domain_id: Uuid,
    user_id: Uuid,
    session: ClassicSession,
}

async fn seed_session(pool: &PgPool) -> SeededSession {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("search-test-{}.example", domain_id);
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(domain_id)
        .bind(&domain_name)
        .execute(pool)
        .await
        .expect("seed test domain");

    let user_id = Uuid::new_v4();
    let username = format!("user-{}@{}", user_id, domain_name);
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, active, is_admin)
         VALUES ($1, $2, $3, 'placeholder-hash-not-used', true, false)",
    )
    .bind(user_id)
    .bind(domain_id)
    .bind(&username)
    .execute(pool)
    .await
    .expect("seed test mailbox");

    let csrf_token = format!(
        "csrftok{:032x}{:04x}",
        Uuid::new_v4().as_u128(),
        rand::random::<u16>()
    );
    let session = ClassicSession::create(
        pool,
        user_id,
        &csrf_token,
        Some("127.0.0.1"),
        Some("test-ua"),
    )
    .await
    .expect("seed classic_sessions row");

    SeededSession {
        domain_id,
        user_id,
        session,
    }
}

async fn cleanup_seeded(pool: &PgPool, seeded: &SeededSession) {
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(seeded.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(seeded.domain_id)
        .execute(pool)
        .await;
}

async fn body_to_string(
    resp: axum::http::Response<Body>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

fn signed_cookie_value(session_id: Uuid) -> String {
    build_cookie_value(TEST_JWT_SECRET, session_id)
}

// ----- Unauthenticated -----

#[tokio::test]
async fn get_search_without_session_bounces_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/search")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let (status, headers, _body) = body_to_string(resp).await;

    // The session middleware redirects to login on missing/invalid cookies.
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "unauthenticated GET /classic/search must redirect, got {status}"
    );
    let location = headers
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert_eq!(location, "/classic/login");
}

// ----- Empty-state happy path -----

#[tokio::test]
async fn get_search_with_no_query_renders_empty_state() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/search")
        .header(header::COOKIE, cookie_header)
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let (status, headers, body) = body_to_string(resp).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "authenticated GET /classic/search must render 200, got {status}\nbody={body}"
    );
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.starts_with("text/html"),
        "search page must render HTML, got Content-Type={content_type}"
    );

    // Empty-state copy + form fields anchor the page.
    assert!(
        body.contains("Search your mailbox") || body.contains("Type a search term"),
        "empty-state copy missing: {body}"
    );
    assert!(
        body.contains("name=\"q\""),
        "search form input must render: {body}"
    );
    assert!(
        body.contains("action=\"/classic/search\""),
        "search form action must point at /classic/search: {body}"
    );
    // Defaulted folder field surfaces INBOX.
    assert!(
        body.contains("value=\"INBOX\""),
        "search form must default folder to INBOX: {body}"
    );
    // The base.html layout (skip-link, brand, nav) MUST still render
    // so the shell stays consistent.
    assert!(body.contains("class=\"skip-link\""), "skip-link missing");
    assert!(body.contains("<nav class=\"site-nav\""), "primary nav missing");
    // The nav-level search box is also rendered (TMAIL-373).
    assert!(
        body.contains("id=\"nav-search-q\""),
        "nav-level search box must render so search is reachable from every page"
    );

    cleanup_seeded(&pool, &seeded).await;
}

#[tokio::test]
async fn get_search_with_whitespace_query_renders_empty_state() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");

    // Whitespace-only `q` must be treated as "no query" so we don't
    // burn an IMAP search on it.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/search?q=%20%20%20")
        .header(header::COOKIE, cookie_header)
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let (status, _headers, body) = body_to_string(resp).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Search your mailbox") || body.contains("Type a search term"),
        "whitespace-only query must render empty-state, not IMAP results: {body}"
    );
    // No "No messages match" copy — that would mean we ran IMAP and got
    // zero hits, which we explicitly avoid for whitespace.
    assert!(
        !body.contains("No messages match"),
        "whitespace-only query must NOT trigger the IMAP no-match copy: {body}"
    );

    cleanup_seeded(&pool, &seeded).await;
}

#[tokio::test]
async fn get_search_preserves_folder_selection_when_no_query() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/search?folder=Drafts")
        .header(header::COOKIE, cookie_header)
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let (status, _headers, body) = body_to_string(resp).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("value=\"Drafts\""),
        "folder selector must echo the requested folder back into the form: {body}"
    );

    cleanup_seeded(&pool, &seeded).await;
}

// ----- Validation -----

#[tokio::test]
async fn get_search_rejects_crlf_injection_in_query() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");

    // CRLF in `q` triggers `validate_search_query` → BadRequest before
    // reaching IMAP. This is the same protection /api/search has.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/search?q=test%0D%0ALOGOUT")
        .header(header::COOKIE, cookie_header)
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "CRLF in q must trigger 400, got {status}"
    );

    cleanup_seeded(&pool, &seeded).await;
}

#[tokio::test]
async fn get_search_rejects_crlf_injection_in_folder() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed_session(&pool).await;
    let cookie_val = signed_cookie_value(seeded.session.id);
    let cookie_header = format!("{CLASSIC_SESSION_COOKIE}={cookie_val}");

    // The folder name is validated before the query — a CRLF folder
    // alone (no q) must 400 too.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/search?folder=INBOX%0D%0ALOGOUT")
        .header(header::COOKIE, cookie_header)
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "CRLF in folder must trigger 400, got {status}"
    );

    cleanup_seeded(&pool, &seeded).await;
}
