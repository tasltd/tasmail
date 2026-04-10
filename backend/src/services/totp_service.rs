use sha2::{Digest, Sha256};
use totp_rs::{Algorithm, Secret, TOTP};

use crate::error::AppError;

/// Generate a new TOTP secret and provisioning URI
pub fn generate_totp(
    username: &str,
    issuer: &str,
) -> Result<(String, String), AppError> {
    let secret = Secret::generate_secret();
    let secret_base32 = secret.to_encoded().to_string();

    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to convert TOTP secret: {}", e))
        })?,
        Some(issuer.to_string()),
        username.to_string(),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create TOTP: {}", e)))?;

    let uri = totp.get_url();

    Ok((secret_base32, uri))
}

/// Verify a TOTP code against a stored secret
pub fn verify_totp(secret_base32: &str, code: &str) -> Result<bool, AppError> {
    let secret = Secret::Encoded(secret_base32.to_string());
    let secret_bytes = secret.to_bytes().map_err(|e| {
        AppError::Internal(anyhow::anyhow!("Invalid TOTP secret: {}", e))
    })?;

    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret_bytes, None, String::new())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create TOTP: {}", e)))?;

    Ok(totp.check_current(code).unwrap_or(false))
}

/// Generate backup recovery codes (10 codes, 8 characters each)
pub fn generate_backup_codes() -> Vec<String> {
    use rand::Rng;
    let mut rng = rand::rng();
    (0..10)
        .map(|_| {
            let code: u32 = rng.random_range(10_000_000..99_999_999);
            format!("{}", code)
        })
        .collect()
}

/// Hash a backup code for storage
pub fn hash_backup_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify a backup code against a hash
pub fn verify_backup_code(code: &str, hash: &str) -> bool {
    hash_backup_code(code) == hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_totp_creates_valid_secret() {
        let (secret, uri) = generate_totp("user@example.com", "TASMail").unwrap();
        assert!(!secret.is_empty());
        assert!(uri.contains("otpauth://totp/"));
        assert!(uri.contains("TASMail"));
        assert!(uri.contains("user%40example.com") || uri.contains("user@example.com"));
    }

    #[test]
    fn test_verify_totp_with_valid_code() {
        let (secret, _) = generate_totp("test@example.com", "TASMail").unwrap();

        // Generate current valid code
        let secret_obj = Secret::Encoded(secret.clone());
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_obj.to_bytes().unwrap(),
            None,
            String::new(),
        )
        .unwrap();
        let current_code = totp.generate_current().unwrap();

        assert!(verify_totp(&secret, &current_code).unwrap());
    }

    #[test]
    fn test_verify_totp_with_invalid_code() {
        let (secret, _) = generate_totp("test@example.com", "TASMail").unwrap();
        assert!(!verify_totp(&secret, "000000").unwrap());
        assert!(!verify_totp(&secret, "123456").unwrap());
    }

    #[test]
    fn test_generate_backup_codes() {
        let codes = generate_backup_codes();
        assert_eq!(codes.len(), 10);
        for code in &codes {
            assert_eq!(code.len(), 8);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
        // All codes should be unique
        let unique: std::collections::HashSet<&String> = codes.iter().collect();
        assert_eq!(unique.len(), 10);
    }

    #[test]
    fn test_hash_and_verify_backup_code() {
        let code = "12345678";
        let hash = hash_backup_code(code);
        assert!(verify_backup_code(code, &hash));
        assert!(!verify_backup_code("87654321", &hash));
    }

    #[test]
    fn test_backup_code_hash_is_deterministic() {
        let code = "99887766";
        assert_eq!(hash_backup_code(code), hash_backup_code(code));
    }

    #[test]
    fn test_different_secrets_produce_different_codes() {
        let (secret1, _) = generate_totp("user1@example.com", "TASMail").unwrap();
        let (secret2, _) = generate_totp("user2@example.com", "TASMail").unwrap();
        assert_ne!(secret1, secret2);
    }
}
