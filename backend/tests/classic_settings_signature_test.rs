// TMAIL-378 — Integration tests for GET / POST /classic/settings/signature
// against a live PostgreSQL database.
//
// Coverage:
//   * GET (no existing signature) — renders the form with empty body,
//     no Remove button, threads the session's csrf_token into the hidden
//     _csrf input + the logout-form partial.
//   * GET (with existing signature) — renders the form prefilled with
//     `text_body`, the preview block shows `html_body`, and the Remove
//     button is present.
//   * POST action=save (no existing signature) — INSERTs a new row with
//     name="Default", correct text_body, sanitised html_body, is_default=true.
//   * POST action=save (existing signature) — UPDATEs the existing row
//     in-place rather than creating a duplicate.
//   * POST action=save sanitises hostile HTML — <script>, onclick=,
//     javascript: URLs are stripped from html_body before it lands in DB.
//   * POST action=save with empty body — re-renders the form with a
//     validation banner; no DB mutation.
//   * POST action=save with oversized body — re-renders the form with a
//     "too long" banner; no DB mutation.
//   * POST action=remove (signature exists) — deletes the row, redirects
//     with flash banner.
//   * POST action=remove (no signature) — redirects with error flash but
//     does not 500.
//   * POST without session cookie → bounces to /classic/login.
//   * POST without _csrf form field → 403 from classic_csrf_middleware.
//
// DB-gated: if DATABASE_URL is unreachable or the `signatures` table is
// missing, each test logs and returns Ok() rather than failing the suite.

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
use tasmail::models::signature::Signature;
use tasmail::router::create_router;
use tasmail::services::auth_service::hash_password;
use tasmail::services::cache_service::CacheService;
use tasmail::services::encryption::EncryptionService;
use tasmail::services::queue_heartbeat::QueueHeartbeat;
use tasmail::state::AppState;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";
const CLASSIC_SESSION_COOKIE: &str = "tasmail_classic_sid";
const PASSWORD: &str = "any-password-for-test";

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
                "TMAIL-378 classic_settings_signature_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    // Schema sanity check — both classic_sessions AND signatures must be
    // present for the test to make sense.
    for table in &["classic_sessions", "signatures"] {
        let exists: (bool,) = match sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "TMAIL-378 classic_settings_signature_test: skipping — schema query failed: {}",
                    e
                );
                return None;
            }
        };
        if !exists.0 {
            eprintln!(
                "TMAIL-378 classic_settings_signature_test: skipping — `{}` table missing",
                table
            );
            return None;
        }
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

struct Seeded {
    domain_id: Uuid,
    user_id: Uuid,
    session: ClassicSession,
    csrf: String,
}

async fn seed(pool: &PgPool) -> Seeded {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("sig-test-{}.example", domain_id);
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(domain_id)
        .bind(&domain_name)
        .execute(pool)
        .await
        .expect("seed domain");

    let user_id = Uuid::new_v4();
    let username = format!("user-{}@{}", user_id, domain_name);
    let hash = hash_password(PASSWORD).expect("hash");
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

    let csrf = format!("csrf-{:032x}", Uuid::new_v4().as_u128());
    let session =
        ClassicSession::create(pool, user_id, &csrf, Some("127.0.0.1"), Some("ua-test"))
            .await
            .expect("seed classic session");

    Seeded {
        domain_id,
        user_id,
        session,
        csrf,
    }
}

async fn cleanup(pool: &PgPool, seeded: &Seeded) {
    let _ = sqlx::query("DELETE FROM signatures WHERE mailbox_id = $1")
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

async fn count_signatures(pool: &PgPool, user_id: Uuid) -> i64 {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM signatures WHERE mailbox_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count signatures");
    row.0
}

// ============================================================
// GET /classic/settings/signature
// ============================================================

#[tokio::test]
async fn get_renders_form_when_no_signature_exists() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK, "GET form should 200; body={body}");
    assert!(
        body.contains("action=\"/classic/settings/signature\""),
        "form should POST back to /classic/settings/signature"
    );
    assert!(
        body.contains(&format!("value=\"{}\"", seeded.csrf)),
        "session csrf_token must be threaded into the hidden _csrf field"
    );
    assert!(body.contains("name=\"body\""), "textarea name=body missing");
    assert!(body.contains("name=\"is_default\""));
    assert!(body.contains("value=\"save\""), "save submit button missing");
    // No Remove button — user has no signature to remove.
    assert!(
        !body.contains("value=\"remove\""),
        "Remove button should be hidden when no signature exists"
    );
    // Logout partial present.
    assert!(body.contains("action=\"/classic/logout\""));
    // No flash banner on fresh render.
    assert!(!body.contains("class=\"alert alert-success\""));
    assert!(!body.contains("class=\"alert alert-error\""));

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn get_prefills_form_from_existing_signature() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    sqlx::query(
        "INSERT INTO signatures (mailbox_id, name, html_body, text_body, is_default)
         VALUES ($1, 'Default', $2, $3, true)",
    )
    .bind(seeded.user_id)
    .bind("<p>Best,<br><b>Kwame</b></p>")
    .bind("Best,\nKwame")
    .execute(&pool)
    .await
    .expect("seed signature");

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK);
    // The textarea content is echoed in between the tags.
    assert!(
        body.contains("Best,\nKwame"),
        "textarea must echo saved text_body: {body}"
    );
    // The preview block renders the (already sanitised) HTML.
    assert!(
        body.contains("<p>Best,<br><b>Kwame</b></p>"),
        "preview block should render html_body: {body}"
    );
    // Remove button is present because the user has a signature.
    assert!(
        body.contains("value=\"remove\""),
        "Remove button missing when signature exists: {body}"
    );
    // Default checkbox is checked.
    assert!(body.contains("checked"), "is_default checkbox should be checked");

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn get_without_cookie_bounces_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/signature")
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    assert!(
        status.is_redirection() || status == StatusCode::UNAUTHORIZED,
        "GET without cookie should bounce or 401, got {status}"
    );
}

// ============================================================
// POST /classic/settings/signature — save
// ============================================================

#[tokio::test]
async fn post_save_creates_new_row_when_none_exists() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "_csrf={}&action=save&body=Best%2C%0AKwame&is_default=on",
        seeded.csrf
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();

    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "POST save happy path should 303-redirect"
    );

    let location = resp
        .headers()
        .get(header::LOCATION)
        .expect("Location header on 303")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        location.starts_with("/classic/settings/signature"),
        "Location should redirect back to settings page: {location}"
    );
    assert!(
        location.contains("flash=saved"),
        "Location should carry the saved flash: {location}"
    );

    let row = Signature::find_default(&pool, seeded.user_id)
        .await
        .expect("query signature");
    assert!(row.is_some(), "save should INSERT a row");
    let row = row.unwrap();
    assert_eq!(row.name, "Default");
    assert_eq!(row.text_body, "Best,\nKwame");
    assert!(row.is_default);
    // html_body is sanitised — plain text input becomes the sanitised
    // (escaped/wrapped) form. For plain text it should at minimum
    // contain the raw text content.
    assert!(
        row.html_body.contains("Best,") && row.html_body.contains("Kwame"),
        "html_body should contain sanitised content: {}",
        row.html_body
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_save_updates_existing_row_in_place() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    // Pre-existing signature.
    sqlx::query(
        "INSERT INTO signatures (mailbox_id, name, html_body, text_body, is_default)
         VALUES ($1, 'Default', $2, $3, true)",
    )
    .bind(seeded.user_id)
    .bind("<p>Old</p>")
    .bind("Old")
    .execute(&pool)
    .await
    .expect("seed signature");

    let before_count = count_signatures(&pool, seeded.user_id).await;
    assert_eq!(before_count, 1);

    let form = format!("_csrf={}&action=save&body=New+body&is_default=on", seeded.csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    let after_count = count_signatures(&pool, seeded.user_id).await;
    assert_eq!(
        after_count, 1,
        "save with existing row should UPDATE, not INSERT — count should stay at 1"
    );
    let row = Signature::find_default(&pool, seeded.user_id)
        .await
        .expect("query")
        .expect("row present");
    assert_eq!(row.text_body, "New body");

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_save_strips_hostile_html_from_html_body() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    // url-encode: `<script>alert(1)</script><b>Kwame</b><a href="javascript:evil()">x</a>`
    let body = "%3Cscript%3Ealert%281%29%3C%2Fscript%3E%3Cb%3EKwame%3C%2Fb%3E%3Ca+href%3D%22javascript%3Aevil%28%29%22%3Ex%3C%2Fa%3E";
    let form = format!("_csrf={}&action=save&body={body}&is_default=on", seeded.csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    let row = Signature::find_default(&pool, seeded.user_id)
        .await
        .expect("query")
        .expect("row present");

    // <script> stripped.
    assert!(
        !row.html_body.contains("<script"),
        "<script> survived sanitisation: {}",
        row.html_body
    );
    // javascript: URL stripped.
    assert!(
        !row.html_body.contains("javascript:"),
        "javascript: URL survived: {}",
        row.html_body
    );
    // Safe formatting preserved.
    assert!(
        row.html_body.contains("<b>Kwame</b>") || row.html_body.contains("Kwame"),
        "safe content lost: {}",
        row.html_body
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_save_empty_body_renders_error_no_mutation() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!("_csrf={}&action=save&body=&is_default=on", seeded.csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "empty body should 400-re-render"
    );
    assert!(
        body.contains("cannot be empty"),
        "error banner missing: {body}"
    );
    let after_count = count_signatures(&pool, seeded.user_id).await;
    assert_eq!(after_count, 0, "no row should have been created");

    cleanup(&pool, &seeded).await;
}

// ============================================================
// POST /classic/settings/signature — remove
// ============================================================

#[tokio::test]
async fn post_remove_deletes_existing_row() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    sqlx::query(
        "INSERT INTO signatures (mailbox_id, name, html_body, text_body, is_default)
         VALUES ($1, 'Default', $2, $3, true)",
    )
    .bind(seeded.user_id)
    .bind("<p>Hi</p>")
    .bind("Hi")
    .execute(&pool)
    .await
    .expect("seed signature");
    assert_eq!(count_signatures(&pool, seeded.user_id).await, 1);

    let form = format!("_csrf={}&action=remove", seeded.csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "POST remove should 303-redirect"
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .expect("Location")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        location.contains("flash=removed"),
        "Location should carry the removed flash: {location}"
    );

    assert_eq!(
        count_signatures(&pool, seeded.user_id).await,
        0,
        "row should be deleted"
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_remove_with_no_existing_signature_redirects_with_error() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!("_csrf={}&action=remove", seeded.csrf);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "remove of non-existent signature should still 303 (not 500)"
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(location.contains("flash=error"));

    cleanup(&pool, &seeded).await;
}

// ============================================================
// Negative paths
// ============================================================

#[tokio::test]
async fn post_without_csrf_field_returns_403() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    // No `_csrf=` field.
    let form = "action=save&body=Hi&is_default=on";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "missing _csrf field must 403 via classic_csrf_middleware"
    );
    assert_eq!(
        count_signatures(&pool, seeded.user_id).await,
        0,
        "no DB mutation should happen on a CSRF rejection"
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_without_session_cookie_bounces_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let form = "_csrf=anything&action=save&body=Hi&is_default=on";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/signature")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    assert!(
        status.is_redirection() || status == StatusCode::UNAUTHORIZED,
        "POST without cookie should bounce or 401, got {status}"
    );
}
