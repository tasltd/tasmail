use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::state::AppState;

/// GET /api/health — basic health check
pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    // Check database connectivity
    let db_ok = sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .is_ok();

    Json(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "database": if db_ok { "connected" } else { "disconnected" },
    }))
}
