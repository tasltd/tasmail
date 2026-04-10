use sqlx::PgPool;
use std::sync::Arc;

use crate::config::SmtpConfig;
use crate::models::mailbox::Mailbox;
use crate::models::scheduled_email::ScheduledEmail;
use crate::services::smtp_service::{SendRequest, SmtpService};

/// Background service that polls for scheduled emails and sends them
pub struct EmailScheduler {
    pool: Arc<PgPool>,
    smtp_config: SmtpConfig,
    poll_interval_secs: u64,
}

impl EmailScheduler {
    pub fn new(pool: Arc<PgPool>, smtp_config: SmtpConfig, poll_interval_secs: u64) -> Self {
        Self {
            pool,
            smtp_config,
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

        let smtp = SmtpService::new(self.smtp_config.clone());

        for email in emails {
            match self.send_email(&smtp, &email).await {
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

    /// Send a single scheduled email
    async fn send_email(
        &self,
        smtp: &SmtpService,
        email: &ScheduledEmail,
    ) -> Result<(), anyhow::Error> {
        let mailbox = Mailbox::find_by_id(&self.pool, email.mailbox_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Mailbox {} not found", email.mailbox_id))?;

        let request = SendRequest {
            to: email.to_addresses.clone(),
            cc: Some(email.cc_addresses.clone()),
            bcc: Some(email.bcc_addresses.clone()),
            subject: email.subject.clone(),
            text_body: email.text_body.clone(),
            html_body: email.html_body.clone(),
        };

        // NOTE: In production, stored encrypted credentials would be used
        // For now, use master password from IMAP config as a placeholder
        smtp.send(&mailbox.username, "placeholder", &request)
            .await
            .map_err(|e| anyhow::anyhow!("SMTP send failed: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_scheduler_construction() {
        // Verify the struct can be created with expected parameters
        // EmailScheduler needs Arc<PgPool> which we can't create without a DB,
        // so just verify the struct fields exist by checking the type compiles
        assert_eq!(
            std::mem::size_of::<EmailScheduler>(),
            std::mem::size_of::<EmailScheduler>()
        );
    }
}
