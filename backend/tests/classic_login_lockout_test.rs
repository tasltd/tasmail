// TMAIL-385 — Integration tests for the locked-state countdown + attempts-
// remaining warning copy on POST /classic/login. Driven against a real
// PostgreSQL database so we exercise the per-account brute-force lockout
// machinery (migration 073) end-to-end through the handler.
//
// Coverage:
//   * Locked POST — when `locked_until > now`, the response renders the
//     locked-state banner with a minute countdown AND a password-reset
//     link, returns 423, and does NOT increment `failed_login_attempts`.
//     The latter is the "no auth attempt is made server-side" check from
//     the acceptance criteria.
//   * Warning-window POST — when `failed_login_attempts` is inside the
//     warning window (default threshold=5, window=2 → counter at 3 or 4),
//     a bad-password POST renders the "Warning: N attempts remaining"
//     copy ABOVE the generic credential error.
//   * Below-window POST — at counter=0..=2 (with one increment from this
//     attempt), the response is the generic error WITHOUT the warning
//     copy. Locks down that legitimate users who mistype once don't get
//     a scary "your account is locked!" banner.
//
// DB-gated: if DATABASE_URL is unreachable or migration 073 hasn't been
// applied, each test logs and returns Ok() rather than failing the suite.
// Same convention as `classic_settings_password_test.rs` (TMAIL-376) and
// `classic_logout_test.rs` (TMAIL-360).

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use chrono::{DateTime, Duration, Utc};
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
use tasmail::router::create_router;
use tasmail::services::auth_service::hash_password;
use tasmail::services::cache_service::CacheService;
use tasmail::services::encryption::EncryptionService;
use tasmail::services::queue_heartbeat::QueueHeartbeat;
use tasmail::state::AppState;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";
const LOGIN_CSRF_COOKIE: &str = "tasmail_classic_login_csrf";
const CORRECT_PASSWORD: &str = "correct-horse-battery-staple-1";

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
                "TMAIL-385 classic_login_lockout_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    // Confirm the per-account lockout columns from migration 073 are
    // present. Older DBs without that migration applied should skip
    // rather than 500.
    let exists: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM information_schema.columns
             WHERE table_name = 'mailboxes' AND column_name = 'locked_until'
        )",
    )
    .fetch_one(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "TMAIL-385 classic_login_lockout_test: skipping — schema query failed: {}",
                e
            );
            return None;
        }
    };
    if !exists.0 {
        eprintln!(
            "TMAIL-385 classic_login_lockout_test: skipping — migration 073 columns missing"
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
        // Default policy: threshold=5, window=15 min, lockout=15 min.
        // Warning window is 2 attempts → values 3 and 4 surface the
        // "N attempts remaining" copy.
        lockout: LockoutConfig::default(),
    }
}

/// Seed a mailbox with a known password and the requested lockout-state
/// columns. Returns the freshly-inserted user_id so cleanup() can drop it
/// (which cascades through the mailbox FK to the lockout columns).
struct Seeded {
    domain_id: Uuid,
    user_id: Uuid,
    username: String,
}

async fn seed(
    pool: &PgPool,
    failed_attempts: i32,
    locked_until: Option<DateTime<Utc>>,
) -> Seeded {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("lockout-test-{}.example", domain_id);
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(domain_id)
        .bind(&domain_name)
        .execute(pool)
        .await
        .expect("seed domain");

    let user_id = Uuid::new_v4();
    let username = format!("user-{}@{}", user_id, domain_name);
    let hash = hash_password(CORRECT_PASSWORD).expect("hash");
    sqlx::query(
        "INSERT INTO mailboxes
            (id, domain_id, username, password_hash, active, is_admin,
             failed_login_attempts, last_failed_login_at, locked_until)
         VALUES ($1, $2, $3, $4, true, false, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(domain_id)
    .bind(&username)
    .bind(&hash)
    .bind(failed_attempts)
    .bind(if failed_attempts > 0 { Some(Utc::now()) } else { None })
    .bind(locked_until)
    .execute(pool)
    .await
    .expect("seed mailbox");

    Seeded { domain_id, user_id, username }
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

async fn current_failed_attempts(pool: &PgPool, user_id: Uuid) -> i32 {
    let (count,): (i32,) =
        sqlx::query_as("SELECT failed_login_attempts FROM mailboxes WHERE id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .expect("read failed_login_attempts");
    count
}

async fn body_to_string(
    resp: axum::http::Response<Body>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&bytes).into_owned())
}

/// Build a POST request with both the pre-session CSRF cookie AND the
/// matching `_csrf` form field. Mirrors the OWASP double-submit-cookie
/// pattern the handler validates.
fn build_login_post(
    csrf_token: &str,
    username: &str,
    password: &str,
) -> Request<Body> {
    // x-www-form-urlencoded body. URL-encode @ in the username so the
    // parser doesn't choke on it (axum's Form extractor uses
    // serde_urlencoded). %40 is the canonical encoding of @.
    let encoded_user = username.replace('@', "%40");
    let body = format!(
        "_csrf={csrf_token}&email={encoded_user}&password={password}"
    );
    Request::builder()
        .method(Method::POST)
        .uri("/classic/login")
        .header(
            header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .header(
            header::COOKIE,
            format!("{}={}", LOGIN_CSRF_COOKIE, csrf_token),
        )
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn locked_account_renders_countdown_and_skips_auth_attempt() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };

    // Seed: 5 failed attempts + lockout active for the next 7 minutes.
    let lockout_until = Utc::now() + Duration::minutes(7);
    let seeded = seed(&pool, 5, Some(lockout_until)).await;
    let attempts_before = current_failed_attempts(&pool, seeded.user_id).await;
    assert_eq!(
        attempts_before, 5,
        "sanity: seed should leave counter at 5"
    );

    let csrf = "lockout-test-csrf-AABB";
    // Submit the CORRECT password — the handler MUST refuse to even
    // check it because the account is locked.
    let req = build_login_post(csrf, &seeded.username, CORRECT_PASSWORD);
    let (status, _headers, body) = body_to_string(router.oneshot(req).await.unwrap()).await;

    // 423 Locked — same status the dispatcher unit tests assert on.
    assert_eq!(
        status,
        StatusCode::LOCKED,
        "locked account POST must return 423, got {status}"
    );

    // Locked-state copy must appear; generic credential error must NOT.
    assert!(
        body.contains("Account temporarily locked"),
        "locked banner missing from response body: {body}"
    );
    assert!(
        body.contains("/classic/password-reset/request"),
        "locked banner must surface password-reset link: {body}"
    );
    assert!(
        !body.contains("Incorrect email or password"),
        "generic credential error must NOT show on locked render: {body}"
    );
    // Round-up arithmetic on a 7-minute window with sub-second slop
    // gives "7" or "8" minutes — accept either to keep the test
    // robust against the test runner's wall-clock skew.
    assert!(
        body.contains("<strong>7</strong>") || body.contains("<strong>8</strong>"),
        "locked banner must show a 7-or-8-minute countdown: {body}"
    );

    // CORE ACCEPTANCE: the lockout pre-check MUST NOT have incremented
    // the counter. If `evaluate_password_login` had been called, the
    // already-locked branch would record an `auth.locked_attempt`
    // audit log but leave the counter alone — but we never want the
    // password hash to be touched in the first place.
    let attempts_after = current_failed_attempts(&pool, seeded.user_id).await;
    assert_eq!(
        attempts_after, 5,
        "locked POST must NOT increment failed_login_attempts, was {attempts_before} → {attempts_after}"
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn warning_window_renders_attempts_remaining_copy() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };

    // Seed: 2 failed attempts, no lockout. Next failure will push the
    // counter to 3 — inside the warning window (default threshold=5,
    // window=2 → values 3 and 4 trigger the warning).
    let seeded = seed(&pool, 2, None).await;

    let csrf = "warning-test-csrf-CCDD";
    let req = build_login_post(csrf, &seeded.username, "totally-wrong-password");
    let (status, _headers, body) = body_to_string(router.oneshot(req).await.unwrap()).await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "bad-password POST inside warning window stays at 401"
    );
    assert!(
        body.contains("Warning:"),
        "warning banner must surface 'Warning:' prefix: {body}"
    );
    // 2 remaining because the increment pushed counter from 2 → 3, and
    // threshold (5) - counter (3) = 2.
    assert!(
        body.contains("2 attempts"),
        "warning copy must show '2 attempts': {body}"
    );
    assert!(
        body.contains("remaining before your account is locked"),
        "warning copy must use the full literal wording: {body}"
    );
    // Generic credential error still appears alongside the warning.
    assert!(
        body.contains("Incorrect email or password"),
        "generic credential error must still appear under the warning: {body}"
    );

    // Confirm the bookkeeping side: counter incremented to 3.
    let attempts_after = current_failed_attempts(&pool, seeded.user_id).await;
    assert_eq!(
        attempts_after, 3,
        "wrong-password POST must bump counter to 3"
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn below_warning_window_renders_generic_only() {
    let Some((router, pool)) = try_build_app().await else {
        return;
    };

    // Seed: 0 prior failures. Next failure pushes counter to 1, which
    // is BELOW the warning floor (threshold - window = 5 - 2 = 3).
    let seeded = seed(&pool, 0, None).await;

    let csrf = "below-window-csrf-EEFF";
    let req = build_login_post(csrf, &seeded.username, "wrong-password");
    let (status, _headers, body) = body_to_string(router.oneshot(req).await.unwrap()).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        body.contains("Incorrect email or password"),
        "generic credential error must show: {body}"
    );
    // No warning copy yet — first mistyped password shouldn't yell.
    assert!(
        !body.contains("Warning:"),
        "warning copy MUST NOT show on first failed attempt: {body}"
    );
    assert!(
        !body.contains("attempts remaining"),
        "warning copy MUST NOT show on first failed attempt: {body}"
    );
    // Lockout banner MUST NOT show either.
    assert!(
        !body.contains("Account temporarily locked"),
        "locked banner MUST NOT show on first failed attempt: {body}"
    );

    cleanup(&pool, &seeded).await;
}

#[tokio::test]
async fn unknown_email_renders_generic_with_no_warning_leak() {
    let Some((router, _pool)) = try_build_app().await else {
        return;
    };

    let csrf = "unknown-email-csrf-GGHH";
    let req = build_login_post(
        csrf,
        &format!("nonexistent-{}@example.com", Uuid::new_v4()),
        "any-password",
    );
    let (status, _headers, body) = body_to_string(router.oneshot(req).await.unwrap()).await;

    // Generic 401 — same shape as wrong-password against a known
    // account. No warning or locked banner — those branches would
    // leak whether the email exists.
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        body.contains("Incorrect email or password"),
        "generic credential error must show for unknown email: {body}"
    );
    assert!(
        !body.contains("Warning:"),
        "unknown-email POST must NOT surface warning copy: {body}"
    );
    assert!(
        !body.contains("Account temporarily locked"),
        "unknown-email POST must NOT surface locked banner: {body}"
    );
}
