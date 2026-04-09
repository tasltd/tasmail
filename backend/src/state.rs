use sqlx::PgPool;

use crate::config::Config;

/// Shared application state accessible in all handlers
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
}
