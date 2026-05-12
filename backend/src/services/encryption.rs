// Added: Thin wrapper around the existing AES-256-GCM helpers in models::ai_config.
// PURPOSE: Single point that holds the derived 32-byte key so call sites pass a service ref
// instead of recomputing the key per call. Mirrors PayPro's EncryptionService.

use crate::models::ai_config;

#[derive(Clone)]
pub struct EncryptionService {
    key: [u8; 32],
}

impl EncryptionService {
    /// PURPOSE: Build a service whose key is derived from the JWT secret (matches AI-config behaviour).
    /// Same secret on backend restart yields the same key, so persisted ciphertext stays decryptable.
    pub fn from_jwt_secret(jwt_secret: &str) -> Self {
        Self { key: ai_config::derive_encryption_key(jwt_secret) }
    }

    pub fn encrypt(&self, plaintext: &str) -> anyhow::Result<String> {
        ai_config::encrypt_api_key(plaintext, &self.key).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn decrypt(&self, ciphertext: &str) -> anyhow::Result<String> {
        ai_config::decrypt_api_key(ciphertext, &self.key).map_err(|e| anyhow::anyhow!(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let svc = EncryptionService::from_jwt_secret("test-jwt-secret-do-not-reuse-9k3lQz");
        let plaintext = "sk_live_paystack_xxxxxxxxxxxxxxxxxxxxxxxx";
        let ct = svc.encrypt(plaintext).unwrap();
        assert_ne!(ct, plaintext);
        let dec = svc.decrypt(&ct).unwrap();
        assert_eq!(dec, plaintext);
    }

    #[test]
    fn different_secrets_produce_undecryptable_ciphertext() {
        let a = EncryptionService::from_jwt_secret("secret-A");
        let b = EncryptionService::from_jwt_secret("secret-B");
        let ct = a.encrypt("hello").unwrap();
        assert!(b.decrypt(&ct).is_err());
    }
}
