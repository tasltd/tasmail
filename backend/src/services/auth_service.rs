use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::JwtConfig;
use crate::error::AppError;
use crate::models::mailbox::Mailbox;
use crate::models::session::Session;

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

/// Authenticate user: verify credentials, create tokens, store session
pub async fn authenticate(
    pool: &sqlx::PgPool,
    config: &JwtConfig,
    username: &str,
    password: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<TokenPair, AppError> {
    let mailbox = Mailbox::find_by_username(pool, username)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    if !verify_password(password, &mailbox.password_hash)? {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

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
}
