// TMAIL-309: Integration test that proves the per-request RLS context is the
// real defense-in-depth — *not* the per-handler `WHERE mailbox_id = $N`
// discipline.
//
// What this test does:
//   1. Seeds two mailboxes (A and B), each with a signature row.
//   2. Issues a "forged handler" query that DELIBERATELY OMITS the WHERE clause:
//        SELECT id, mailbox_id FROM signatures
//      Without RLS this returns both rows — a cross-tenant leak.
//   3. Acquires a connection via `db_session::acquire_with_rls` for user A's
//      claims, then runs the forged query on that connection.
//   4. Asserts the result contains ONLY A's signature row — RLS denied B's row.
//   5. Negative control: runs the SAME query on a raw pool connection (no RLS
//      context). FORCE ROW LEVEL SECURITY blocks the table owner from seeing
//      any rows when no session vars are set, so we assert zero rows there —
//      proving the policy is what's enforcing isolation, not just our seed
//      WHERE clause.
//   6. Cleanup so repeated runs stay idempotent.
//
// The test is DB-gated: if DATABASE_URL is unreachable (CI without Postgres),
// the test logs and returns early instead of failing — matching the convention
// in `admin_audit_test.rs`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use tasmail::config::{
    Config, DatabaseConfig, ImapConfig, JwtConfig, LockoutConfig, RedisConfig, ServerConfig,
    SmtpConfig, StorageConfig,
};
use tasmail::services::auth_service::Claims;
use tasmail::services::cache_service::CacheService;
use tasmail::services::db_session;
use tasmail::services::encryption::EncryptionService;
use tasmail::state::AppState;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";

fn resolve_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tasmail:tasmail@localhost/tasmail".to_string())
}

async fn try_build_state() -> Option<(AppState, PgPool)> {
    let db_url = resolve_db_url();
    let pool = match PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect(&db_url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "TMAIL-309 rls_context_test: skipping — DB unreachable at {}: {}",
                db_url, e
            );
            return None;
        }
    };

    let exists: (bool,) = match sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'signatures')",
    )
    .fetch_one(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "TMAIL-309 rls_context_test: skipping — could not query schema: {}",
                e
            );
            return None;
        }
    };
    if !exists.0 {
        eprintln!(
            "TMAIL-309 rls_context_test: skipping — signatures table missing (migrations not run)"
        );
        return None;
    }

    let inner_router_holder: std::sync::Arc<std::sync::OnceLock<axum::Router>> =
        std::sync::Arc::new(std::sync::OnceLock::new());

    let state = AppState {
        db: pool.clone(),
        config: test_config(db_url),
        metrics_handle: None,
        cache: CacheService::disabled(),
        encryption: EncryptionService::from_jwt_secret(TEST_JWT_SECRET),
        inner_router: inner_router_holder,
        // Added (TMAIL-356): AppState gained `queue_heartbeat` in TMAIL-310
        // but this older test file didn't get updated — fill it in so the
        // build passes for everything else.
        queue_heartbeat: tasmail::services::queue_heartbeat::QueueHeartbeat::new(),
    };
    Some((state, pool))
}

fn test_config(db_url: String) -> Config {
    Config {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
        },
        database: DatabaseConfig {
            url: db_url,
            max_connections: 4,
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
        // Added (TMAIL-356): Config gained `metrics_allowed_ips` in TMAIL-314
        // but this older test file didn't get updated — `None` falls back to
        // loopback-only, which is what the integration tests expect.
        metrics_allowed_ips: None,
        rspamd_url: None,
        rspamd_password: None,
        billing: None,
        push: None,
        redis: RedisConfig::default(),
        lockout: LockoutConfig::default(),
    }
}

fn claims_for(user_id: Uuid) -> Claims {
    let now = chrono::Utc::now();
    let exp = now + chrono::Duration::seconds(900);
    Claims {
        sub: user_id.to_string(),
        username: format!("user-{}@example.com", user_id),
        is_admin: false,
        is_compliance_officer: false,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    }
}

/// Seed a domain + mailbox + signature for one tenant. Returns `(mailbox_id,
/// signature_id, domain_id)` so the caller can clean up later.
async fn seed_tenant(
    pool: &PgPool,
    suffix: &str,
) -> Result<(Uuid, Uuid, Uuid), sqlx::Error> {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("rls-test-{}-{}.example", suffix, Uuid::new_v4());
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(domain_id)
        .bind(&domain_name)
        .execute(pool)
        .await?;

    let mailbox_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, is_admin, active)
         VALUES ($1, $2, $3, 'placeholder-hash', false, true)",
    )
    .bind(mailbox_id)
    .bind(domain_id)
    .bind(format!("rls-{}-{}@{}", suffix, mailbox_id, domain_name))
    .execute(pool)
    .await?;

    // INSERT into signatures — but RLS is forced, so we need to set
    // app.mailbox_id on this connection to pass the WITH CHECK policy.
    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT set_config('app.mailbox_id', $1, false)")
        .bind(mailbox_id.to_string())
        .execute(&mut *conn)
        .await?;
    sqlx::query("SELECT set_config('app.current_user_id', $1, false)")
        .bind(mailbox_id.to_string())
        .execute(&mut *conn)
        .await?;
    let signature_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO signatures (id, mailbox_id, name, html_body, text_body, is_default)
         VALUES ($1, $2, $3, '<p>sig</p>', 'sig', false)",
    )
    .bind(signature_id)
    .bind(mailbox_id)
    .bind(format!("sig-{}", suffix))
    .execute(&mut *conn)
    .await?;
    drop(conn);

    Ok((mailbox_id, signature_id, domain_id))
}

async fn cleanup_tenant(
    pool: &PgPool,
    mailbox_id: Uuid,
    domain_id: Uuid,
) {
    // Need RLS context to delete the signature; cascade handles the mailbox+domain.
    if let Ok(mut conn) = pool.acquire().await {
        let _ = sqlx::query("SELECT set_config('app.mailbox_id', $1, false)")
            .bind(mailbox_id.to_string())
            .execute(&mut *conn)
            .await;
        let _ = sqlx::query("DELETE FROM signatures WHERE mailbox_id = $1")
            .bind(mailbox_id)
            .execute(&mut *conn)
            .await;
    }
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(mailbox_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(pool)
        .await;
}

/// The headline test: a "forged handler" without a WHERE clause must NOT leak
/// across tenants when running on an RLS-primed connection.
#[tokio::test]
async fn rls_context_blocks_cross_tenant_leak_on_forged_handler() {
    let Some((state, pool)) = try_build_state().await else {
        return; // skipped — DB unreachable
    };

    let (mailbox_a, signature_a, domain_a) = match seed_tenant(&pool, "a").await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("TMAIL-309: failed to seed tenant A: {}", e);
            return;
        }
    };
    let (mailbox_b, signature_b, domain_b) = match seed_tenant(&pool, "b").await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("TMAIL-309: failed to seed tenant B: {}", e);
            cleanup_tenant(&pool, mailbox_a, domain_a).await;
            return;
        }
    };

    // -------- Forged handler: NO WHERE clause --------
    // A real handler would filter by mailbox_id, but the whole point of RLS is
    // that even if the WHERE is dropped, the policy still blocks the leak.
    let claims_a = claims_for(mailbox_a);
    let mut conn = db_session::acquire_with_rls(&state, &claims_a)
        .await
        .expect("acquire_with_rls for tenant A");

    let leaked_rows: Vec<(Uuid, Uuid)> =
        sqlx::query("SELECT id, mailbox_id FROM signatures")
            .fetch_all(&mut *conn)
            .await
            .expect("forged SELECT on RLS-primed conn")
            .into_iter()
            .map(|row| (row.get::<Uuid, _>("id"), row.get::<Uuid, _>("mailbox_id")))
            .collect();
    drop(conn);

    assert_eq!(
        leaked_rows.len(),
        1,
        "RLS should have hidden tenant B's signature; got {} rows: {:?}",
        leaked_rows.len(),
        leaked_rows
    );
    assert_eq!(
        leaked_rows[0].0, signature_a,
        "expected only tenant A's signature visible under RLS"
    );
    assert_eq!(
        leaked_rows[0].1, mailbox_a,
        "expected only tenant A's mailbox_id visible under RLS"
    );
    // And the forbidden row is NOT in the result set.
    assert!(
        !leaked_rows.iter().any(|(id, _)| *id == signature_b),
        "tenant B's signature leaked into tenant A's view (RLS broken)"
    );

    // -------- Negative control: same query, no RLS context --------
    // signatures has FORCE ROW LEVEL SECURITY, so a connection with no session
    // vars set sees zero rows. This proves the isolation we observed above is
    // RLS doing its job, not just our seed data being lucky.
    let mut raw_conn = pool.acquire().await.expect("acquire raw conn");
    let unprimed_rows: Vec<(Uuid, Uuid)> = sqlx::query(
        "SELECT id, mailbox_id FROM signatures WHERE id = $1 OR id = $2",
    )
    .bind(signature_a)
    .bind(signature_b)
    .fetch_all(&mut *raw_conn)
    .await
    .expect("forged SELECT on unprimed conn")
    .into_iter()
    .map(|row| (row.get::<Uuid, _>("id"), row.get::<Uuid, _>("mailbox_id")))
    .collect();
    drop(raw_conn);

    assert_eq!(
        unprimed_rows.len(),
        0,
        "unprimed connection should see zero rows under FORCE RLS; got {:?}",
        unprimed_rows
    );

    // -------- Positive control: RLS context for tenant B sees B's row --------
    let claims_b = claims_for(mailbox_b);
    let mut conn_b = db_session::acquire_with_rls(&state, &claims_b)
        .await
        .expect("acquire_with_rls for tenant B");
    let b_rows: Vec<(Uuid, Uuid)> = sqlx::query("SELECT id, mailbox_id FROM signatures")
        .fetch_all(&mut *conn_b)
        .await
        .expect("SELECT on tenant B conn")
        .into_iter()
        .map(|row| (row.get::<Uuid, _>("id"), row.get::<Uuid, _>("mailbox_id")))
        .collect();
    drop(conn_b);

    assert_eq!(b_rows.len(), 1, "tenant B should see exactly one signature");
    assert_eq!(b_rows[0].0, signature_b, "tenant B should see its own row");

    // Cleanup
    cleanup_tenant(&pool, mailbox_a, domain_a).await;
    cleanup_tenant(&pool, mailbox_b, domain_b).await;
}

/// Confirms `acquire_with_rls` actually sets all three session vars (not just
/// `app.mailbox_id` as the pre-TMAIL-309 version did). 38+ RLS policies depend
/// on `app.current_user_id` being set — if this regresses, those policies
/// silently deny every row.
#[tokio::test]
async fn rls_context_sets_all_three_session_vars() {
    let Some((state, _pool)) = try_build_state().await else {
        return;
    };

    let user_id = Uuid::new_v4();
    let claims = claims_for(user_id);
    let mut conn = db_session::acquire_with_rls(&state, &claims)
        .await
        .expect("acquire_with_rls");

    let current_user_id: String =
        sqlx::query_scalar("SELECT current_setting('app.current_user_id', true)")
            .fetch_one(&mut *conn)
            .await
            .expect("read app.current_user_id");
    let mailbox_id: String =
        sqlx::query_scalar("SELECT current_setting('app.mailbox_id', true)")
            .fetch_one(&mut *conn)
            .await
            .expect("read app.mailbox_id");
    let is_admin: String = sqlx::query_scalar("SELECT current_setting('app.is_admin', true)")
        .fetch_one(&mut *conn)
        .await
        .expect("read app.is_admin");

    assert_eq!(current_user_id, user_id.to_string());
    assert_eq!(mailbox_id, user_id.to_string());
    assert_eq!(is_admin, "false");
}

/// `acquire_with_rls` should refuse to set RLS context if the JWT's `sub` is
/// not a real UUID — better to fail loud than to bind an unparseable value
/// and have RLS silently deny everything.
#[tokio::test]
async fn rls_context_rejects_invalid_claim_sub() {
    let Some((state, _pool)) = try_build_state().await else {
        return;
    };

    let mut bad_claims = claims_for(Uuid::new_v4());
    bad_claims.sub = "not-a-uuid".to_string();

    let result = db_session::acquire_with_rls(&state, &bad_claims).await;
    assert!(
        result.is_err(),
        "acquire_with_rls should reject non-UUID claim.sub, got Ok"
    );
}
