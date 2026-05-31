use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::{JwtConfig, LockoutConfig};
use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::models::mailbox::Mailbox;
use crate::models::session::Session;

/// Added (TMAIL-273): Generic, non-enumerating message returned on every 423.
/// Same text whether the lockout is fresh or already in effect, so attackers
/// can't fingerprint how close they are to the threshold.
const LOCKOUT_MESSAGE: &str = "Account temporarily locked. Try again later.";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // mailbox_id
    pub username: String,
    pub is_admin: bool,
    // Added: TMAIL-137 — dedicated compliance officer role for eDiscovery.
    // `default` lets us decode pre-migration tokens issued before this field existed.
    #[serde(default)]
    pub is_compliance_officer: bool,
    pub exp: usize,
    pub iat: usize,
}

/// Added: TMAIL-210 — shared admin-role gate. Returns Forbidden when the
/// JWT claims don't carry is_admin = true. Every admin/* handler should
/// invoke this as the first thing it does, mirroring the
/// claims.is_admin check that admin/audit.rs and admin/users.rs already
/// have.
pub fn require_admin(claims: &Claims) -> Result<(), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    Ok(())
}

/// Added: TMAIL-137 — compliance role gate. Admins implicitly pass; a
/// dedicated compliance officer (without full admin rights) also passes.
/// Used by eDiscovery handlers so investigators can be granted access
/// without the broader admin surface.
pub fn require_compliance(claims: &Claims) -> Result<(), AppError> {
    if claims.is_admin || claims.is_compliance_officer {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "Compliance officer or admin access required".to_string(),
    ))
}

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// Hash a password using Argon2id
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hashing failed: {}", e)))?;
    Ok(hash.to_string())
}

/// Verify a password against an Argon2id hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Invalid password hash: {}", e)))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Generate a JWT access token
pub fn create_access_token(
    config: &JwtConfig,
    mailbox: &Mailbox,
) -> Result<String, AppError> {
    let now = Utc::now();
    let exp = now + Duration::seconds(config.access_token_expiry_secs as i64);

    let claims = Claims {
        sub: mailbox.id.to_string(),
        username: mailbox.username.clone(),
        is_admin: mailbox.is_admin,
        is_compliance_officer: mailbox.is_compliance_officer,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encoding failed: {}", e)))
}

/// Validate and decode a JWT access token
pub fn validate_access_token(config: &JwtConfig, token: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

/// Generate a random refresh token
pub fn generate_refresh_token() -> String {
    Uuid::new_v4().to_string()
}

/// Hash a refresh token for storage (SHA-256)
pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Added (TMAIL-273): Per-account lockout state going into the next
/// authentication decision. Encapsulates the three persistent columns added
/// by migration 073 so the pure decision helper can be unit-tested without a
/// database.
#[derive(Debug, Clone, Copy)]
pub struct LockoutState {
    pub failed_attempts: i32,
    pub last_failed_at: Option<chrono::DateTime<Utc>>,
    pub locked_until: Option<chrono::DateTime<Utc>>,
}

impl From<&Mailbox> for LockoutState {
    fn from(m: &Mailbox) -> Self {
        Self {
            failed_attempts: m.failed_login_attempts,
            last_failed_at: m.last_failed_login_at,
            locked_until: m.locked_until,
        }
    }
}

/// Added (TMAIL-273): Outcome of one authentication attempt against the
/// lockout state machine. Drives both the SQL update and the audit_log
/// transition record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockoutOutcome {
    /// Caller arrived while still inside an active lockout window —
    /// reject without checking the password.
    AlreadyLocked,
    /// This failed attempt pushed the rolling counter to the threshold —
    /// lockout starts now.
    JustLocked,
    /// Failed attempt, counter incremented, threshold not yet reached.
    FailedNotLocked,
    /// Password verified successfully; counter and lockout cleared.
    Success,
}

/// Added (TMAIL-273): Pure helper computing the next failure-counter +
/// locked_until pair given the current state. Pure → unit-testable.
///
/// Behaviour:
/// * If the last failure is older than `window_secs`, the counter restarts at 1
///   (the rolling window has expired).
/// * Otherwise increment.
/// * If the resulting counter is >= `threshold`, set `locked_until = now + duration`.
pub fn next_failed_state(
    prev: &LockoutState,
    cfg: &LockoutConfig,
    now: chrono::DateTime<Utc>,
) -> (i32, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>, LockoutOutcome) {
    let within_window = prev
        .last_failed_at
        .map(|t| (now - t).num_seconds() < cfg.window_secs)
        .unwrap_or(false);
    let counter = if within_window { prev.failed_attempts + 1 } else { 1 };
    if counter >= cfg.threshold {
        let until = now + Duration::seconds(cfg.duration_secs);
        (counter, now, Some(until), LockoutOutcome::JustLocked)
    } else {
        (counter, now, None, LockoutOutcome::FailedNotLocked)
    }
}

/// Added (TMAIL-273): Pure helper that decides whether the caller is still
/// locked out before we even check the password. Returns true when
/// `locked_until` is set and still in the future.
pub fn is_currently_locked(state: &LockoutState, now: chrono::DateTime<Utc>) -> bool {
    state.locked_until.map(|t| t > now).unwrap_or(false)
}

/// Added (TMAIL-359): Reusable password-evaluation step shared by the JWT
/// login flow (`authenticate`) AND the Classic UI cookie-session login
/// (`handlers::classic::auth`). Performs the same three operations:
///
///   1. Resolve `username` to a `Mailbox`. Missing / inactive → `Unauthorized`.
///   2. Honour the existing per-account lockout window (migration 073). Still
///      locked → `AccountLocked` with the generic LOCKOUT_MESSAGE.
///   3. Verify the password. On failure, increment the rolling counter and
///      audit-log; on the *just-locked* transition, return `AccountLocked`
///      instead of `Unauthorized` so the caller surfaces the right status.
///   4. On success, clear the lockout columns and return the verified
///      `Mailbox`. Token / session creation is the caller's concern — that's
///      what makes this reusable between the two login surfaces.
///
/// Auditing matches `authenticate`'s prior behaviour 1:1 so existing audit
/// dashboards keep producing the same row shapes.
pub async fn evaluate_password_login(
    pool: &sqlx::PgPool,
    lockout_cfg: &LockoutConfig,
    username: &str,
    password: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<Mailbox, AppError> {
    let mailbox = Mailbox::find_by_username(pool, username)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    let now = Utc::now();
    let state = LockoutState::from(&mailbox);

    // 1) If the account is still in an active lockout window, reject
    //    immediately — without touching the password hash. This is the
    //    "successful login during lockout still returns 423" branch of the
    //    acceptance criteria.
    if is_currently_locked(&state, now) {
        // NOTE: Audit a separate "already locked" hit so investigators can
        // see continued attempts against a locked account.
        let _ = AuditLog::record(
            pool,
            Some(mailbox.id),
            "auth.locked_attempt",
            Some("mailbox"),
            Some(&mailbox.id.to_string()),
            Some(serde_json::json!({
                "locked_until": state.locked_until,
            })),
            ip_address,
            user_agent,
        )
        .await;
        return Err(AppError::AccountLocked(LOCKOUT_MESSAGE.to_string()));
    }

    // 2) If the previous lockout window has passed but the columns are still
    //    populated, log the expiry transition once before resetting state.
    let lockout_just_expired =
        state.locked_until.map(|t| t <= now).unwrap_or(false);

    // 3) Verify the password.
    if !verify_password(password, &mailbox.password_hash)? {
        let (new_count, last_failed, new_lock, outcome) =
            next_failed_state(&state, lockout_cfg, now);
        // Best-effort persist — don't fail the whole login flow if the
        // UPDATE blows up (the user still gets a 401/423).
        let _ = persist_failed_state(pool, mailbox.id, new_count, last_failed, new_lock).await;

        if outcome == LockoutOutcome::JustLocked {
            let _ = AuditLog::record(
                pool,
                Some(mailbox.id),
                "auth.locked",
                Some("mailbox"),
                Some(&mailbox.id.to_string()),
                Some(serde_json::json!({
                    "failed_login_attempts": new_count,
                    "locked_until": new_lock,
                    "threshold": lockout_cfg.threshold,
                    "duration_secs": lockout_cfg.duration_secs,
                })),
                ip_address,
                user_agent,
            )
            .await;
            return Err(AppError::AccountLocked(LOCKOUT_MESSAGE.to_string()));
        }
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    // 4) Password matched — clear lockout columns. Log the expiry-on-success
    //    transition if the previous lockout had simply timed out before this
    //    successful login.
    let had_lockout = state.locked_until.is_some() || state.failed_attempts > 0;
    let _ = persist_success_reset(pool, mailbox.id).await;
    if had_lockout {
        let action = if lockout_just_expired {
            "auth.lockout_expired"
        } else {
            "auth.lockout_cleared"
        };
        let _ = AuditLog::record(
            pool,
            Some(mailbox.id),
            action,
            Some("mailbox"),
            Some(&mailbox.id.to_string()),
            Some(serde_json::json!({
                "prior_failed_attempts": state.failed_attempts,
                "prior_locked_until": state.locked_until,
            })),
            ip_address,
            user_agent,
        )
        .await;
    }

    Ok(mailbox)
}

/// Added (TMAIL-376): Hash a new password and persist it to the mailbox row.
///
/// Thin convenience wrapper used by the Classic UI's `/classic/settings/password`
/// handler (and any future surface that needs to rotate a user's password
/// without going through the full forgot-password email flow). The caller is
/// responsible for having already verified the current password — typically
/// via `evaluate_password_login` — and for revoking outstanding sessions
/// afterwards.
///
/// Returns `true` when the UPDATE touched a row, `false` when the mailbox
/// id no longer resolves (the user was deleted between the verify step
/// and this call). Callers should treat `false` as a hard error since the
/// session under which the change was requested is by definition stale.
pub async fn change_password(
    pool: &sqlx::PgPool,
    mailbox_id: Uuid,
    new_password: &str,
) -> Result<bool, AppError> {
    let new_hash = hash_password(new_password)?;
    Mailbox::update_password(pool, mailbox_id, &new_hash)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("update_password failed: {}", e)))
}

/// Authenticate user: verify credentials, create tokens, store session
///
/// Added (TMAIL-273): Enforces per-account brute-force lockout in addition
/// to the existing per-IP rate limit. Threshold/window/duration come from
/// `LockoutConfig` so operators can tune the policy.
///
/// Changed (TMAIL-359): The password-evaluation + lockout-bookkeeping step
/// was extracted into `evaluate_password_login` so the Classic UI's
/// cookie-session login path can share it. This function now wraps that
/// shared helper with JWT issuance + refresh-token persistence.
pub async fn authenticate(
    pool: &sqlx::PgPool,
    config: &JwtConfig,
    lockout_cfg: &LockoutConfig,
    username: &str,
    password: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<TokenPair, AppError> {
    let mailbox = evaluate_password_login(
        pool,
        lockout_cfg,
        username,
        password,
        ip_address,
        user_agent,
    )
    .await?;

    let access_token = create_access_token(config, &mailbox)?;
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&refresh_token);

    let expires_at = Utc::now() + Duration::seconds(config.refresh_token_expiry_secs as i64);

    // Fix: TMAIL-157 / TMAIL-197 — Session::create acquires its own pool
    // connection, so the RLS session vars set elsewhere don't apply. Pin the
    // app.mailbox_id + app.is_admin config to the same connection that runs
    // the INSERT, otherwise the sessions-table policy 500s with
    // "new row violates row-level security policy for table sessions".
    insert_session_with_rls_context(pool, &mailbox, &refresh_hash, expires_at, ip_address, user_agent).await?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: config.access_token_expiry_secs,
    })
}

/// Added (TMAIL-273): Persist failed-attempt counter + lockout timestamp.
/// Best-effort — the login response shape doesn't change if this UPDATE fails.
async fn persist_failed_state(
    pool: &sqlx::PgPool,
    mailbox_id: uuid::Uuid,
    failed_attempts: i32,
    last_failed_at: chrono::DateTime<Utc>,
    locked_until: Option<chrono::DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mailboxes
            SET failed_login_attempts = $2,
                last_failed_login_at = $3,
                locked_until = $4,
                updated_at = NOW()
          WHERE id = $1",
    )
    .bind(mailbox_id)
    .bind(failed_attempts)
    .bind(last_failed_at)
    .bind(locked_until)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Added (TMAIL-273): Reset lockout state after a successful login.
async fn persist_success_reset(
    pool: &sqlx::PgPool,
    mailbox_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mailboxes
            SET failed_login_attempts = 0,
                last_failed_login_at = NULL,
                locked_until = NULL,
                updated_at = NOW()
          WHERE id = $1",
    )
    .bind(mailbox_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Fix: TMAIL-157 — connection-pinned session insert. RLS policy on the
/// `sessions` table requires `app.mailbox_id` to match the row being
/// inserted; SET configs only stick to a single connection so we have to
/// hold one for the whole set+insert sequence.
async fn insert_session_with_rls_context(
    pool: &sqlx::PgPool,
    mailbox: &Mailbox,
    refresh_hash: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT set_config('app.mailbox_id', $1, false)")
        .bind(mailbox.id.to_string())
        .execute(&mut *conn)
        .await?;
    sqlx::query("SELECT set_config('app.is_admin', $1, false)")
        .bind(mailbox.is_admin.to_string())
        .execute(&mut *conn)
        .await?;

    sqlx::query(
        "INSERT INTO sessions (id, mailbox_id, refresh_token_hash, expires_at, created_at, ip_address, user_agent) \
         VALUES (gen_random_uuid(), $1, $2, $3, NOW(), $4, $5)",
    )
    .bind(mailbox.id)
    .bind(refresh_hash)
    .bind(expires_at)
    .bind(ip_address)
    .bind(user_agent)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Added: Issue a fresh token pair for a known mailbox without verifying password.
/// PURPOSE: Used by the signup flow — the user has just created the account and we
/// want to log them in immediately without a separate /login round-trip.
/// CONSTRAINTS: Caller MUST have already authenticated the user some other way
/// (e.g., they just signed up, or finished an OIDC/SAML flow). Never expose this
/// directly via an HTTP endpoint.
pub async fn issue_token_pair_for_mailbox(
    pool: &sqlx::PgPool,
    config: &JwtConfig,
    mailbox: &Mailbox,
) -> Result<TokenPair, AppError> {
    let access_token = create_access_token(config, mailbox)?;
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + Duration::seconds(config.refresh_token_expiry_secs as i64);

    // Changed: TMAIL-197 — share the connection-pinned insert helper with
    // authenticate(); see insert_session_with_rls_context above.
    insert_session_with_rls_context(pool, mailbox, &refresh_hash, expires_at, None, None).await?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: config.access_token_expiry_secs,
    })
}

/// Refresh an access token using a valid refresh token
pub async fn refresh_tokens(
    pool: &sqlx::PgPool,
    config: &JwtConfig,
    refresh_token: &str,
) -> Result<TokenPair, AppError> {
    let token_hash = hash_refresh_token(refresh_token);

    let session = Session::find_by_token_hash(pool, &token_hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired refresh token".to_string()))?;

    // Delete old session
    Session::delete(pool, session.id).await?;

    let mailbox = Mailbox::find_by_id(pool, session.mailbox_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User no longer exists".to_string()))?;

    // Create new token pair (rotation)
    let access_token = create_access_token(config, &mailbox)?;
    let new_refresh = generate_refresh_token();
    let new_hash = hash_refresh_token(&new_refresh);

    let expires_at = Utc::now() + Duration::seconds(config.refresh_token_expiry_secs as i64);

    // Fix: TMAIL-157 — same connection-pinned insert as authenticate().
    insert_session_with_rls_context(pool, &mailbox, &new_hash, expires_at, None, None).await?;

    Ok(TokenPair {
        access_token,
        refresh_token: new_refresh,
        expires_in: config.access_token_expiry_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_jwt_config() -> JwtConfig {
        JwtConfig {
            secret: "test-secret-key-for-unit-tests".to_string(),
            access_token_expiry_secs: 900,
            refresh_token_expiry_secs: 604800,
        }
    }

    fn test_mailbox() -> Mailbox {
        Mailbox {
            id: Uuid::new_v4(),
            domain_id: Uuid::new_v4(),
            username: "test@example.com".to_string(),
            password_hash: hash_password("testpass123").unwrap(),
            display_name: Some("Test User".to_string()),
            quota_bytes: 1_073_741_824,
            quota_warn_percent: 80,
            active: true,
            is_admin: false,
            is_compliance_officer: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            totp_secret: None,
            totp_enabled: false,
            totp_verified_at: None,
            failed_login_attempts: 0,
            last_failed_login_at: None,
            locked_until: None,
        }
    }

    #[test]
    fn test_hash_and_verify_password() {
        let password = "secure_password_123!";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_hash_password_produces_unique_hashes() {
        let password = "same_password";
        let hash1 = hash_password(password).unwrap();
        let hash2 = hash_password(password).unwrap();
        // Different salts mean different hashes
        assert_ne!(hash1, hash2);
        // Both verify correctly
        assert!(verify_password(password, &hash1).unwrap());
        assert!(verify_password(password, &hash2).unwrap());
    }

    #[test]
    fn test_empty_password() {
        let hash = hash_password("").unwrap();
        assert!(verify_password("", &hash).unwrap());
        assert!(!verify_password("anything", &hash).unwrap());
    }

    #[test]
    fn test_unicode_password() {
        let password = "p@$$w0rd_!#";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
    }

    #[test]
    fn test_verify_invalid_hash_format() {
        let result = verify_password("password", "not-a-valid-hash");
        assert!(result.is_err());
    }

    #[test]
    fn test_refresh_token_hashing() {
        let token = generate_refresh_token();
        let hash1 = hash_refresh_token(&token);
        let hash2 = hash_refresh_token(&token);
        assert_eq!(hash1, hash2);
        let token2 = generate_refresh_token();
        assert_ne!(hash_refresh_token(&token), hash_refresh_token(&token2));
    }

    #[test]
    fn test_refresh_token_uniqueness() {
        let tokens: Vec<String> = (0..100).map(|_| generate_refresh_token()).collect();
        let unique: std::collections::HashSet<&String> = tokens.iter().collect();
        assert_eq!(tokens.len(), unique.len());
    }

    #[test]
    fn test_refresh_token_hash_is_sha256() {
        let token = generate_refresh_token();
        let hash = hash_refresh_token(&token);
        // SHA-256 produces 64 hex characters
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_create_access_token() {
        let config = test_jwt_config();
        let mailbox = test_mailbox();
        let token = create_access_token(&config, &mailbox).unwrap();
        assert!(!token.is_empty());
        // Token should have 3 parts (header.payload.signature)
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn test_validate_access_token() {
        let config = test_jwt_config();
        let mailbox = test_mailbox();
        let token = create_access_token(&config, &mailbox).unwrap();
        let claims = validate_access_token(&config, &token).unwrap();
        assert_eq!(claims.sub, mailbox.id.to_string());
        assert_eq!(claims.username, "test@example.com");
        assert!(!claims.is_admin);
    }

    #[test]
    fn test_validate_token_wrong_secret() {
        let config = test_jwt_config();
        let mailbox = test_mailbox();
        let token = create_access_token(&config, &mailbox).unwrap();

        let wrong_config = JwtConfig {
            secret: "wrong-secret".to_string(),
            ..config
        };
        let result = validate_access_token(&wrong_config, &token);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_expired_token() {
        let config = test_jwt_config();
        let mailbox = test_mailbox();
        // Manually create a token that expired an hour ago
        let past = Utc::now() - Duration::seconds(7200);
        let claims = Claims {
            sub: mailbox.id.to_string(),
            username: mailbox.username,
            is_admin: false,
            is_compliance_officer: false,
            exp: (past + Duration::seconds(900)).timestamp() as usize,
            iat: past.timestamp() as usize,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(config.secret.as_bytes()),
        )
        .unwrap();
        let result = validate_access_token(&config, &token);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_malformed_token() {
        let config = test_jwt_config();
        assert!(validate_access_token(&config, "not.a.token").is_err());
        assert!(validate_access_token(&config, "").is_err());
        assert!(validate_access_token(&config, "abc").is_err());
    }

    #[test]
    fn test_admin_claim_in_token() {
        let config = test_jwt_config();
        let mut mailbox = test_mailbox();
        mailbox.is_admin = true;
        let token = create_access_token(&config, &mailbox).unwrap();
        let claims = validate_access_token(&config, &token).unwrap();
        assert!(claims.is_admin);
    }

    #[test]
    fn test_token_expiry_is_set() {
        let config = test_jwt_config();
        let mailbox = test_mailbox();
        let token = create_access_token(&config, &mailbox).unwrap();
        let claims = validate_access_token(&config, &token).unwrap();
        assert!(claims.exp > claims.iat);
        // Expiry should be ~900 seconds after issued-at
        let diff = claims.exp - claims.iat;
        assert!(diff >= 899 && diff <= 901);
    }

    // ---------------------------------------------------------------------
    // Added (TMAIL-273): Per-account brute-force lockout state machine.
    //
    // The helpers are intentionally pure (no DB) so we can exercise every
    // transition required by the acceptance criteria without standing up a
    // test database. Each test maps to one bullet on the AC:
    //
    //   - threshold_increment_on_fail   → "increment failed_login_attempts"
    //   - window_expiry_resets_counter  → "older than window resets counter"
    //   - lockout_returns_locked        → "5 in 15min ⇒ locked, return 423"
    //   - currently_locked_blocks       → "locked_until > now ⇒ block"
    //   - lockout_expiry_allows_retry   → "lockout expiry allows retry"
    // ---------------------------------------------------------------------

    fn test_lockout_cfg() -> LockoutConfig {
        LockoutConfig {
            threshold: 5,
            window_secs: 900,
            duration_secs: 900,
        }
    }

    fn state(attempts: i32, last: Option<chrono::DateTime<Utc>>, lock: Option<chrono::DateTime<Utc>>) -> LockoutState {
        LockoutState {
            failed_attempts: attempts,
            last_failed_at: last,
            locked_until: lock,
        }
    }

    #[test]
    fn lockout_threshold_increment_on_fail() {
        // Counter increments by 1 each failure within the window and stays
        // below threshold until attempt #5.
        let cfg = test_lockout_cfg();
        let now = Utc::now();
        let mut s = state(0, None, None);

        for expected in 1..5 {
            let (count, _last, lock, outcome) = next_failed_state(&s, &cfg, now);
            assert_eq!(count, expected, "attempt {expected}");
            assert!(lock.is_none(), "no lockout before threshold (attempt {expected})");
            assert_eq!(outcome, LockoutOutcome::FailedNotLocked);
            s = state(count, Some(now), None);
        }
    }

    #[test]
    fn lockout_window_expiry_resets_counter() {
        // 4 prior failures, but the last one was outside the 15-min window —
        // the next failure resets the counter to 1, not 5.
        let cfg = test_lockout_cfg();
        let now = Utc::now();
        let last_failed = now - Duration::seconds(cfg.window_secs + 1);
        let prev = state(4, Some(last_failed), None);

        let (count, last, lock, outcome) = next_failed_state(&prev, &cfg, now);
        assert_eq!(count, 1, "rolling window expired → counter restarts at 1");
        assert_eq!(last, now);
        assert!(lock.is_none());
        assert_eq!(outcome, LockoutOutcome::FailedNotLocked);
    }

    #[test]
    fn lockout_triggers_at_threshold() {
        // 5th failure within the window pushes counter to threshold → locked.
        let cfg = test_lockout_cfg();
        let now = Utc::now();
        let prev = state(4, Some(now - Duration::seconds(60)), None);

        let (count, _last, lock, outcome) = next_failed_state(&prev, &cfg, now);
        assert_eq!(count, 5);
        let lock = lock.expect("threshold should set locked_until");
        assert_eq!(
            (lock - now).num_seconds(),
            cfg.duration_secs,
            "locked_until = now + duration_secs"
        );
        assert_eq!(outcome, LockoutOutcome::JustLocked);
    }

    #[test]
    fn lockout_blocks_while_active() {
        // is_currently_locked() returns true while locked_until is in the future.
        let now = Utc::now();
        let s = state(5, Some(now), Some(now + Duration::seconds(60)));
        assert!(is_currently_locked(&s, now));
    }

    #[test]
    fn lockout_expiry_allows_retry() {
        // Once locked_until is in the past, is_currently_locked() returns
        // false → the auth flow falls through to the password check on the
        // next attempt.
        let now = Utc::now();
        let s = state(5, Some(now - Duration::seconds(1000)), Some(now - Duration::seconds(1)));
        assert!(!is_currently_locked(&s, now));
    }

    #[test]
    fn lockout_no_state_is_not_locked() {
        // Fresh account with no lockout history.
        let now = Utc::now();
        let s = state(0, None, None);
        assert!(!is_currently_locked(&s, now));
    }

    #[test]
    fn lockout_threshold_one_locks_first_attempt() {
        // Defensive: with threshold=1 the very first failure locks the
        // account. Verifies the inequality is `>=` not `>`.
        let cfg = LockoutConfig { threshold: 1, window_secs: 900, duration_secs: 900 };
        let now = Utc::now();
        let prev = state(0, None, None);

        let (count, _last, lock, outcome) = next_failed_state(&prev, &cfg, now);
        assert_eq!(count, 1);
        assert!(lock.is_some());
        assert_eq!(outcome, LockoutOutcome::JustLocked);
    }

    #[test]
    fn lockout_state_from_mailbox_copies_columns() {
        // From<&Mailbox> mirrors the three migration-073 columns into the
        // pure helper struct.
        let mut mb = test_mailbox();
        let now = Utc::now();
        mb.failed_login_attempts = 3;
        mb.last_failed_login_at = Some(now);
        mb.locked_until = Some(now + Duration::seconds(60));

        let s = LockoutState::from(&mb);
        assert_eq!(s.failed_attempts, 3);
        assert_eq!(s.last_failed_at, Some(now));
        assert_eq!(s.locked_until, Some(now + Duration::seconds(60)));
    }

    #[test]
    fn lockout_config_defaults_are_safe() {
        let cfg = LockoutConfig::default();
        assert_eq!(cfg.threshold, 5);
        assert_eq!(cfg.window_secs, 900);
        assert_eq!(cfg.duration_secs, 900);
    }
}
