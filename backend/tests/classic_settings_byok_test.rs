// TMAIL-380 — Integration tests for GET / POST /classic/settings/byok
// against a live PostgreSQL database.
//
// Coverage:
//   * GET (no existing IMAP/SMTP rows) — renders the form with empty
//     host/username, sensible default ports (993 / 587), and the
//     password helper says "Required".
//   * GET (with existing IMAP + SMTP) — prefills host/port/username,
//     defaults encryption from the saved row, and the password fields
//     remain BLANK (never echoed back). Helper text changes to "Leave
//     blank to keep your saved password."
//   * POST action=save WITHOUT tested_ok=1 — re-renders the form with
//     the "Test the connection before saving…" banner; no DB rows are
//     touched. (Spec: "Save button is gated to only enable after a
//     successful test or 'save without testing' link".)
//   * POST action=save_no_test (no rows) — INSERTs default rows for
//     IMAP and SMTP, encrypts password at rest.
//   * POST action=save_no_test (existing rows, password blank) —
//     UPDATEs host/port/username/encryption, KEEPS encrypted_password
//     as-is. (Spec: "on edit, an unchanged field keeps the encrypted
//     password row as-is".)
//   * POST action=save_no_test (existing rows, new password) — UPDATEs
//     every column INCLUDING re-encrypting the new password.
//   * GET without session cookie → bounces to /classic/login.
//   * POST without _csrf form field → 403 from classic_csrf_middleware.
//
// DB-gated: if DATABASE_URL is unreachable or the
// imap_configurations / smtp_configurations / classic_sessions tables
// are missing, each test logs and returns Ok() rather than failing the
// suite. Same convention as `classic_settings_signature_test.rs`.

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
use tasmail::models::ai_config::{decrypt_api_key, derive_encryption_key, encrypt_api_key};
use tasmail::models::classic_session::ClassicSession;
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
                "TMAIL-380 classic_settings_byok_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    // Schema sanity — every table the BYOK page reads or writes must be
    // present. Without these, the test would 500 in the handler.
    for table in &["classic_sessions", "imap_configurations", "smtp_configurations"] {
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
                    "TMAIL-380 classic_settings_byok_test: skipping — schema query failed: {}",
                    e
                );
                return None;
            }
        };
        if !exists.0 {
            eprintln!(
                "TMAIL-380 classic_settings_byok_test: skipping — `{}` table missing",
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
    let domain_name = format!("byok-test-{}.example", domain_id);
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
    let _ = sqlx::query("DELETE FROM imap_configurations WHERE user_id = $1")
        .bind(seeded.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM smtp_configurations WHERE user_id = $1")
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

async fn seed_imap(pool: &PgPool, user_id: Uuid, plaintext_password: &str) -> Uuid {
    let key = derive_encryption_key(TEST_JWT_SECRET);
    let encrypted = encrypt_api_key(plaintext_password, &key).expect("encrypt IMAP password");
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO imap_configurations
            (id, user_id, name, host, port, username, encrypted_password, encryption, is_default)
         VALUES ($1, $2, 'Default', 'imap.gmail.com', 993, 'alice@gmail.com', $3, 'ssl', true)",
    )
    .bind(id)
    .bind(user_id)
    .bind(&encrypted)
    .execute(pool)
    .await
    .expect("seed imap_configurations row");
    id
}

async fn seed_smtp(pool: &PgPool, user_id: Uuid, plaintext_password: &str) -> Uuid {
    let key = derive_encryption_key(TEST_JWT_SECRET);
    let encrypted = encrypt_api_key(plaintext_password, &key).expect("encrypt SMTP password");
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO smtp_configurations
            (id, user_id, name, host, port, username, encrypted_password, encryption, is_default)
         VALUES ($1, $2, 'Default', 'smtp.gmail.com', 587, 'alice@gmail.com', $3, 'starttls', true)",
    )
    .bind(id)
    .bind(user_id)
    .bind(&encrypted)
    .execute(pool)
    .await
    .expect("seed smtp_configurations row");
    id
}

async fn fetch_imap_row(
    pool: &PgPool,
    user_id: Uuid,
) -> Option<(String, i32, String, String, String)> {
    sqlx::query_as::<_, (String, i32, String, String, String)>(
        "SELECT host, port, username, encrypted_password, encryption \
         FROM imap_configurations WHERE user_id = $1 AND is_default = true LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .expect("fetch imap row")
}

async fn fetch_smtp_row(
    pool: &PgPool,
    user_id: Uuid,
) -> Option<(String, i32, String, String, String)> {
    sqlx::query_as::<_, (String, i32, String, String, String)>(
        "SELECT host, port, username, encrypted_password, encryption \
         FROM smtp_configurations WHERE user_id = $1 AND is_default = true LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .expect("fetch smtp row")
}

// ============================================================
// GET /classic/settings/byok
// ============================================================

#[tokio::test]
async fn get_renders_blank_form_when_no_rows_exist() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK, "GET should 200; body={body}");
    assert!(
        body.contains("action=\"/classic/settings/byok\""),
        "save form should POST back to /classic/settings/byok"
    );
    assert!(
        body.contains("action=\"/classic/settings/byok/test\""),
        "test form should POST to /classic/settings/byok/test"
    );
    assert!(
        body.contains(&format!("value=\"{}\"", seeded.csrf)),
        "session csrf_token must be threaded into the hidden _csrf field"
    );
    // Sensible default ports show up on a brand-new form.
    assert!(body.contains("value=\"993\""), "IMAP port default 993 missing");
    assert!(body.contains("value=\"587\""), "SMTP port default 587 missing");
    // The "required" helper copy reflects the create path (no saved
    // row → password is required).
    assert!(
        body.contains("Required"),
        "create-path helper copy should mention 'Required'"
    );
    // Logout partial.
    assert!(body.contains("action=\"/classic/logout\""));

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn get_prefills_form_from_existing_rows_but_blanks_passwords() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    seed_imap(&pool, seeded.user_id, "imap-secret-do-not-leak").await;
    seed_smtp(&pool, seeded.user_id, "smtp-secret-do-not-leak").await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::OK);
    // Hosts + usernames prefilled.
    assert!(body.contains("imap.gmail.com"));
    assert!(body.contains("smtp.gmail.com"));
    assert!(body.contains("alice@gmail.com"));
    // Edit-path helper copy.
    assert!(
        body.contains("Leave blank to keep your saved password"),
        "edit-path helper copy missing"
    );
    // Spec: passwords NEVER round-trip back to the rendered form.
    assert!(
        !body.contains("imap-secret-do-not-leak"),
        "plaintext IMAP password leaked into rendered HTML: {body}"
    );
    assert!(
        !body.contains("smtp-secret-do-not-leak"),
        "plaintext SMTP password leaked into rendered HTML: {body}"
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn get_without_cookie_bounces_to_login() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let req = Request::builder()
        .method(Method::GET)
        .uri("/classic/settings/byok")
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
// POST /classic/settings/byok — gating + save semantics
// ============================================================

#[tokio::test]
async fn post_save_without_tested_ok_refuses_and_does_not_mutate_db() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "_csrf={}&action=save\
         &imap_host=imap.gmail.com&imap_port=993&imap_username=alice%40gmail.com\
         &imap_password=imap-pw&imap_encryption=ssl\
         &smtp_host=smtp.gmail.com&smtp_port=587&smtp_username=alice%40gmail.com\
         &smtp_password=smtp-pw&smtp_encryption=starttls",
        seeded.csrf
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    // Refused — re-renders the form with a 400.
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("Test the connection before saving"),
        "expected the test-first banner: {body}"
    );

    // No DB mutation.
    assert!(fetch_imap_row(&pool, seeded.user_id).await.is_none());
    assert!(fetch_smtp_row(&pool, seeded.user_id).await.is_none());

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_save_no_test_creates_default_rows_when_none_exist() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    let form = format!(
        "_csrf={}&action=save_no_test\
         &imap_host=imap.gmail.com&imap_port=993&imap_username=alice%40gmail.com\
         &imap_password=new-imap-pw&imap_encryption=ssl\
         &smtp_host=smtp.gmail.com&smtp_port=587&smtp_username=alice%40gmail.com\
         &smtp_password=new-smtp-pw&smtp_encryption=starttls",
        seeded.csrf
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    assert_eq!(status, StatusCode::SEE_OTHER);

    let imap = fetch_imap_row(&pool, seeded.user_id)
        .await
        .expect("imap row created");
    assert_eq!(imap.0, "imap.gmail.com");
    assert_eq!(imap.1, 993);
    assert_eq!(imap.2, "alice@gmail.com");
    assert_eq!(imap.4, "ssl");
    // Password is encrypted at rest (round-trips through decrypt).
    let key = derive_encryption_key(TEST_JWT_SECRET);
    assert_eq!(decrypt_api_key(&imap.3, &key).unwrap(), "new-imap-pw");

    let smtp = fetch_smtp_row(&pool, seeded.user_id)
        .await
        .expect("smtp row created");
    assert_eq!(smtp.0, "smtp.gmail.com");
    assert_eq!(smtp.1, 587);
    assert_eq!(smtp.2, "alice@gmail.com");
    assert_eq!(smtp.4, "starttls");
    assert_eq!(decrypt_api_key(&smtp.3, &key).unwrap(), "new-smtp-pw");

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_save_no_test_updates_existing_rows_and_keeps_password_when_blank() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    seed_imap(&pool, seeded.user_id, "imap-original").await;
    seed_smtp(&pool, seeded.user_id, "smtp-original").await;

    // Submit with EVERY field changed EXCEPT the passwords.
    let form = format!(
        "_csrf={}&action=save_no_test\
         &imap_host=imap.outlook.com&imap_port=993&imap_username=bob%40outlook.com\
         &imap_password=&imap_encryption=ssl\
         &smtp_host=smtp.outlook.com&smtp_port=587&smtp_username=bob%40outlook.com\
         &smtp_password=&smtp_encryption=starttls",
        seeded.csrf
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    assert_eq!(status, StatusCode::SEE_OTHER);

    let imap = fetch_imap_row(&pool, seeded.user_id)
        .await
        .expect("imap row exists");
    assert_eq!(imap.0, "imap.outlook.com");
    assert_eq!(imap.2, "bob@outlook.com");
    // Spec: "on edit, an unchanged field keeps the encrypted password
    // row as-is".
    let key = derive_encryption_key(TEST_JWT_SECRET);
    assert_eq!(decrypt_api_key(&imap.3, &key).unwrap(), "imap-original");

    let smtp = fetch_smtp_row(&pool, seeded.user_id)
        .await
        .expect("smtp row exists");
    assert_eq!(smtp.0, "smtp.outlook.com");
    assert_eq!(smtp.2, "bob@outlook.com");
    assert_eq!(decrypt_api_key(&smtp.3, &key).unwrap(), "smtp-original");

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_save_no_test_updates_password_when_typed() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    seed_imap(&pool, seeded.user_id, "imap-original").await;
    seed_smtp(&pool, seeded.user_id, "smtp-original").await;

    let form = format!(
        "_csrf={}&action=save_no_test\
         &imap_host=imap.gmail.com&imap_port=993&imap_username=alice%40gmail.com\
         &imap_password=imap-rotated&imap_encryption=ssl\
         &smtp_host=smtp.gmail.com&smtp_port=587&smtp_username=alice%40gmail.com\
         &smtp_password=smtp-rotated&smtp_encryption=starttls",
        seeded.csrf
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    let key = derive_encryption_key(TEST_JWT_SECRET);
    let imap = fetch_imap_row(&pool, seeded.user_id).await.unwrap();
    assert_eq!(decrypt_api_key(&imap.3, &key).unwrap(), "imap-rotated");
    let smtp = fetch_smtp_row(&pool, seeded.user_id).await.unwrap();
    assert_eq!(decrypt_api_key(&smtp.3, &key).unwrap(), "smtp-rotated");

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_save_no_test_requires_imap_password_on_create() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    // No rows exist yet — leaving the password blank should fail
    // validation rather than silently inserting a row with an empty
    // encrypted password.
    let form = format!(
        "_csrf={}&action=save_no_test\
         &imap_host=imap.gmail.com&imap_port=993&imap_username=alice%40gmail.com\
         &imap_password=&imap_encryption=ssl\
         &smtp_host=smtp.gmail.com&smtp_port=587&smtp_username=alice%40gmail.com\
         &smtp_password=smtp-pw&smtp_encryption=starttls",
        seeded.csrf
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let (status, _, body) = body_str(resp).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("IMAP password is required"),
        "expected the IMAP create-password validation banner: {body}"
    );
    assert!(fetch_imap_row(&pool, seeded.user_id).await.is_none());
    assert!(fetch_smtp_row(&pool, seeded.user_id).await.is_none());

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn post_without_csrf_returns_403() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };
    let seeded = seed(&pool).await;

    // No `_csrf` field at all — classic_csrf_middleware should reject
    // the request with a 403 before the handler ever runs.
    let form = "action=save_no_test\
                &imap_host=imap.gmail.com&imap_port=993&imap_username=alice%40gmail.com\
                &imap_password=imap-pw&imap_encryption=ssl\
                &smtp_host=smtp.gmail.com&smtp_port=587&smtp_username=alice%40gmail.com\
                &smtp_password=smtp-pw&smtp_encryption=starttls";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/settings/byok")
        .header(header::COOKIE, cookie_for(seeded.session.id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Defence: no rows leaked through despite the missing CSRF.
    assert!(fetch_imap_row(&pool, seeded.user_id).await.is_none());
    assert!(fetch_smtp_row(&pool, seeded.user_id).await.is_none());

    cleanup(&pool, &seeded).await;
}
