// Added (TMAIL-322): Periodic LDAP sync — the background scheduler deferred in
// services/ldap_service.rs.
//
// PURPOSE: Every poll it finds active ldap_configurations whose
// sync_interval_minutes has elapsed since last_sync_at, and runs the same
// decrypt → search_users → apply_sync path as the manual
// POST /api/admin/ldap/:id/sync, writing an ldap_sync_logs row for each run.
// CONSTRAINTS: One failing directory must never stop the others, so every
// per-config error is recorded on that config's sync log and the loop moves on.

use serde_json::json;
use sqlx::PgPool;

use crate::models::ldap_config::{LdapConfiguration, LdapSyncLog};
use crate::services::encryption::EncryptionService;
use crate::services::ldap_service::LdapService;

pub struct LdapSyncScheduler {
    pool: PgPool,
    encryption: EncryptionService,
    poll_interval_secs: u64,
}

impl LdapSyncScheduler {
    pub fn new(pool: PgPool, encryption: EncryptionService, poll_interval_secs: u64) -> Self {
        Self {
            pool,
            encryption,
            // A zero interval would spin the loop; clamp to one second.
            poll_interval_secs: poll_interval_secs.max(1),
        }
    }

    /// Start the background scheduler loop.
    pub fn start(self) {
        tokio::spawn(async move {
            tracing::info!(
                "LDAP sync scheduler started (poll interval: {}s)",
                self.poll_interval_secs
            );
            loop {
                if let Err(e) = self.process_due_syncs().await {
                    tracing::error!("LDAP sync scheduler error: {}", e);
                }
                tokio::time::sleep(std::time::Duration::from_secs(self.poll_interval_secs)).await;
            }
        });
    }

    /// Run a sync for every active config that is due.
    async fn process_due_syncs(&self) -> Result<(), sqlx::Error> {
        let configs = LdapConfiguration::find_due_for_sync(&self.pool).await?;
        if configs.is_empty() {
            return Ok(());
        }

        tracing::info!("Running LDAP sync for {} due configuration(s)", configs.len());
        for config in configs {
            if let Err(e) = self.sync_one(&config).await {
                tracing::error!("LDAP sync for config '{}' failed: {}", config.name, e);
            }
        }
        Ok(())
    }

    async fn sync_one(&self, config: &LdapConfiguration) -> Result<(), sqlx::Error> {
        let sync_log = LdapSyncLog::create(&self.pool, config.id).await?;

        let bind_password = match self.encryption.decrypt(&config.bind_password_encrypted) {
            Ok(pw) => pw,
            Err(e) => {
                return self
                    .fail(config, sync_log.id, "decrypt_bind_password", &e.to_string())
                    .await;
            }
        };

        let users = match LdapService::search_users(
            &config.server_url,
            &config.bind_dn,
            &bind_password,
            &config.search_base,
            &config.search_filter,
            &config.email_attribute,
            &config.name_attribute,
        )
        .await
        {
            Ok(u) => u,
            Err(e) => return self.fail(config, sync_log.id, "ldap_search", &e.to_string()).await,
        };

        let found = users.len();
        let result = LdapService::apply_sync(&self.pool, users).await;
        let total_synced = result.created + result.updated;
        LdapSyncLog::complete(
            &self.pool,
            sync_log.id,
            "completed",
            result.created,
            result.updated,
            result.disabled,
            &serde_json::Value::Array(result.errors.clone()),
        )
        .await?;
        LdapConfiguration::update_sync_status(&self.pool, config.id, "completed", total_synced)
            .await?;

        tracing::info!(
            "LDAP sync completed for config '{}': {} found, {} created, {} updated, {} disabled, {} errors",
            config.name,
            found,
            result.created,
            result.updated,
            result.disabled,
            result.errors.len(),
        );
        Ok(())
    }

    /// Mark a run failed. Updating last_sync_at also stops an unreachable
    /// directory being retried on every poll instead of once per interval.
    async fn fail(
        &self,
        config: &LdapConfiguration,
        log_id: uuid::Uuid,
        stage: &str,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        let errors = json!([{ "stage": stage, "error": error }]);
        LdapSyncLog::complete(&self.pool, log_id, "failed", 0, 0, 0, &errors).await?;
        LdapConfiguration::update_sync_status(&self.pool, config.id, "failed", 0).await?;
        tracing::warn!("LDAP sync for config '{}' failed at {}: {}", config.name, stage, error);
        Ok(())
    }
}
