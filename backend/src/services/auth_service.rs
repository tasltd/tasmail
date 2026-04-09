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
    pub exp: usize,
    pub iat: usize,
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

    Session::create(
        pool,
        mailbox.id,
        &refresh_hash,
        expires_at,
        ip_address,
        user_agent,
    )
    .await?;

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

    Session::create(pool, mailbox.id, &new_hash, expires_at, None, None).await?;

    Ok(TokenPair {
        access_token,
        refresh_token: new_refresh,
        expires_in: config.access_token_expiry_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "secure_password_123!";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_refresh_token_hashing() {
        let token = generate_refresh_token();
        let hash1 = hash_refresh_token(&token);
        let hash2 = hash_refresh_token(&token);
        // Same input produces same hash
        assert_eq!(hash1, hash2);
        // Different tokens produce different hashes
        let token2 = generate_refresh_token();
        assert_ne!(hash_refresh_token(&token), hash_refresh_token(&token2));
    }
}
