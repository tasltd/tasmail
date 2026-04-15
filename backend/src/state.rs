use sqlx::PgPool;
// Added: Prometheus handle for rendering metrics output (TMAIL-41)
use metrics_exporter_prometheus::PrometheusHandle;

use crate::config::Config;

/// Shared application state accessible in all handlers
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    // Added: Optional Prometheus handle for /metrics endpoint (TMAIL-41)
    pub metrics_handle: Option<PrometheusHandle>,
}
