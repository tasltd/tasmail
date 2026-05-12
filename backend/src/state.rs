use sqlx::PgPool;
// Added: Prometheus handle for rendering metrics output (TMAIL-41)
use metrics_exporter_prometheus::PrometheusHandle;

use crate::config::Config;
// Added: Redis cache service for performance optimization
use crate::services::cache_service::CacheService;
// Added: AES-256-GCM encryption service used to decrypt DB-stored payment credentials.
use crate::services::encryption::EncryptionService;

/// Shared application state accessible in all handlers
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    // Added: Optional Prometheus handle for /metrics endpoint (TMAIL-41)
    pub metrics_handle: Option<PrometheusHandle>,
    // Added: Redis cache service for branding/quota/rate-limit/session caching
    pub cache: CacheService,
    // Added: Encryption service for DB-stored credentials (PaymentProviderConfig, etc.)
    pub encryption: EncryptionService,
}
