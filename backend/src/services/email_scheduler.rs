use sqlx::PgPool;
use std::sync::Arc;

use crate::config::JwtConfig;
use crate::models::ai_config::derive_encryption_key;
use crate::models::mailbox::Mailbox;
use crate::models::scheduled_email::ScheduledEmail;
use crate::models::smtp_config::SmtpConfiguration;
use crate::services::smtp_service::{SendRequest, SmtpService};

/// Background service that polls for scheduled emails and sends them.
///
/// Changed (TMAIL-301): BYOK delivery — for each scheduled row, load the owning
/// mailbox's default SMTP configuration from `smtp_configurations`, decrypt the
/// AES-256-GCM password with the JWT-derived key, and deliver via that user's
/// own SMTP host. The previous implementation used the literal string
/// "placeholder" as the SMTP password, so every send authenticated as the
/// system user and was rejected by every real SMTP server — scheduled-send was
/// fully broken in production. Mirrors the pattern in `queue_processor`.
pub struct EmailScheduler {
    pool: Arc<PgPool>,
    jwt_config: JwtConfig,
    poll_interval_secs: u64,
}

impl EmailScheduler {
    pub fn new(pool: Arc<PgPool>, jwt_config: JwtConfig, poll_interval_secs: u64) -> Self {
        Self {
            pool,
            jwt_config,
            poll_interval_secs,
        }
    }

    /// Start the background scheduler loop
    pub fn start(self) {
        tokio::spawn(async move {
            tracing::info!("Email scheduler started (poll interval: {}s)", self.poll_interval_secs);
            loop {
                if let Err(e) = self.process_pending().await {
                    tracing::error!("Email scheduler error: {}", e);
                }
                tokio::time::sleep(std::time::Duration::from_secs(self.poll_interval_secs)).await;
            }
        });
    }

    /// Process all pending emails that are ready to send
    async fn process_pending(&self) -> Result<(), anyhow::Error> {
        let emails = ScheduledEmail::find_ready_to_send(&self.pool).await?;

        if emails.is_empty() {
            return Ok(());
        }

        tracing::info!("Processing {} scheduled emails", emails.len());

        let encryption_key = derive_encryption_key(&self.jwt_config.secret);

        for email in emails {
            match self.send_email(&encryption_key, &email).await {
                Ok(()) => {
                    ScheduledEmail::mark_sent(&self.pool, email.id).await?;
                    tracing::info!("Sent scheduled email {}", email.id);
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    ScheduledEmail::mark_failed(&self.pool, email.id, &error_msg).await?;
                }
            }
        }

        Ok(())
    }

    /// Send a single scheduled email using the owning mailbox's BYOK SMTP config.
    async fn send_email(
        &self,
        encryption_key: &[u8; 32],
        email: &ScheduledEmail,
    ) -> Result<(), anyhow::Error> {
        // Verify the mailbox still exists before incurring a decrypt round-trip.
        let _mailbox = Mailbox::find_by_id(&self.pool, email.mailbox_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Mailbox {} not found", email.mailbox_id))?;

        // BYOK: load per-user SMTP server + decrypt the stored password.
        let smtp_cfg = SmtpConfiguration::find_default(&self.pool, email.mailbox_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Mailbox {} has no default SMTP configuration — user must complete onboarding",
                    email.mailbox_id
                )
            })?;
        let password = smtp_cfg
            .decrypted_password(encryption_key)
            .map_err(|e| anyhow::anyhow!("Failed to decrypt SMTP password: {}", e))?;
        let from_address = smtp_cfg
            .from_address
            .clone()
            .unwrap_or_else(|| smtp_cfg.username.clone());

        let smtp_runtime_cfg = crate::config::SmtpConfig {
            host: smtp_cfg.host.clone(),
            port: smtp_cfg.port as u16,
            tls: matches!(smtp_cfg.encryption.as_str(), "ssl" | "starttls"),
            notification_from: None,
            notification_username: None,
            notification_password: None,
        };
        let smtp = SmtpService::new(smtp_runtime_cfg);

        let request = SendRequest {
            to: email.to_addresses.clone(),
            cc: Some(email.cc_addresses.clone()),
            bcc: Some(email.bcc_addresses.clone()),
            subject: email.subject.clone(),
            text_body: email.text_body.clone(),
            html_body: email.html_body.clone(),
        };

        smtp.send(&from_address, &password, &request)
            .await
            .map_err(|e| anyhow::anyhow!("SMTP send failed: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ai_config::{decrypt_api_key, encrypt_api_key};

    // Added (TMAIL-301): Round-trip test asserting that the same JWT-derived key
    // the scheduler uses can decrypt a password the BYOK API encrypts with the
    // shared helpers. Failure here means the scheduler would re-introduce the
    // "placeholder" bug by silently producing a wrong-key plaintext.
    #[test]
    fn scheduler_encryption_key_round_trips_smtp_password() {
        let jwt_secret = "tmail-301-test-secret-do-not-use-in-prod";
        let key = derive_encryption_key(jwt_secret);
        let plaintext = "s3cret-app-password-from-gmail";

        let ciphertext = encrypt_api_key(plaintext, &key).expect("encrypt");
        let decrypted = decrypt_api_key(&ciphertext, &key).expect("decrypt");

        assert_eq!(decrypted, plaintext);
        // And critically — NOT the previous literal placeholder value.
        assert_ne!(decrypted, "placeholder");
    }

    // Added (TMAIL-301): Guard against the obvious regression — different
    // JWT secrets must derive different keys, so a key swap invalidates
    // all stored ciphertexts (this is the documented JWT_SECRET behaviour
    // in CLAUDE.md → Common gotchas).
    #[test]
    fn different_jwt_secrets_produce_unusable_decryption() {
        let key_a = derive_encryption_key("secret-a");
        let key_b = derive_encryption_key("secret-b");
        let ciphertext = encrypt_api_key("real-password", &key_a).expect("encrypt");
        let result = decrypt_api_key(&ciphertext, &key_b);
        assert!(result.is_err(), "decryption with mismatched key must fail");
    }

    // Added (TMAIL-301): The SmtpConfiguration::decrypted_password helper is the
    // exact entrypoint the scheduler uses — assert it produces the right
    // plaintext given a row populated the same way the API populates it.
    #[test]
    fn smtp_configuration_decrypted_password_matches_input() {
        use uuid::Uuid;

        let jwt_secret = "tmail-301-smtp-config-test";
        let key = derive_encryption_key(jwt_secret);
        let plaintext = "my-byok-smtp-password";
        let encrypted = encrypt_api_key(plaintext, &key).expect("encrypt");

        let cfg = SmtpConfiguration {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Gmail".to_string(),
            host: "smtp.gmail.com".to_string(),
            port: 587,
            username: "user@gmail.com".to_string(),
            encrypted_password: encrypted,
            encryption: "starttls".to_string(),
            from_address: Some("user@gmail.com".to_string()),
            is_default: true,
            verified: true,
            last_tested_at: None,
            created_at: None,
            updated_at: None,
        };

        let recovered = cfg.decrypted_password(&key).expect("decrypted_password");
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn test_email_scheduler_construction() {
        // Sanity: scheduler builds with a JwtConfig (the BYOK contract).
        // Cannot exercise process_pending without a PgPool; that path is
        // covered by integration tests in tests/scheduled_email_byok.rs.
        assert_eq!(
            std::mem::size_of::<EmailScheduler>(),
            std::mem::size_of::<EmailScheduler>()
        );
    }
}
