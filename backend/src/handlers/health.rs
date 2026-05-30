// TMAIL-310: liveness vs readiness split.
//
// The previous `health_check` only ran `SELECT 1` and returned
// `{status: "healthy"}` even when Redis was down, the queue processor was
// stalled, or the mailbox table was unreachable — uptime monitors saw green
// while the user-visible system was red.
//
// This module now exposes:
//   * GET /api/health                  — back-compat shape (legacy clients)
//   * GET /api/health?detail=full      — structured per-component report
//   * GET /api/health/live             — liveness probe (process + DB ping)
//   * GET /api/health/ready            — readiness probe (DB + Redis + queue + mailboxes)
//
// Liveness is intentionally narrow (a Kubernetes liveness probe restarting
// the pod because Redis is down would be wrong). Readiness gates traffic.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::cache_service::CacheService;
use crate::services::queue_heartbeat::QueueHeartbeat;
use crate::state::AppState;

// PURPOSE: Threshold (in seconds) for considering the queue processor
// stalled. The processor polls every 5s by default; 60s gives 12 cycles of
// headroom before the readiness probe flips a component to "stalled".
const QUEUE_STALL_THRESHOLD_SECS: i64 = 60;

/// Added (TMAIL-310): Query params for `/api/health`. The only recognised
/// value is `detail=full`, which switches the response to the structured
/// per-component report shared with `/api/health/ready`.
#[derive(Debug, Deserialize, Default)]
pub struct HealthQuery {
    #[serde(default)]
    pub detail: Option<String>,
}

/// Per-component status entry in the structured readiness report.
#[derive(Debug, Clone, Serialize)]
pub struct ComponentStatus {
    /// Whether the component is healthy from the application's POV.
    pub ok: bool,
    /// Short tag suitable for dashboards: "connected", "disconnected",
    /// "stalled", "not_started", "error", "ok".
    pub status: &'static str,
    /// Optional human-readable note (latency, count, error message).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ComponentStatus {
    fn ok(status: &'static str, detail: Option<String>) -> Self {
        Self { ok: true, status, detail }
    }
    fn down(status: &'static str, detail: Option<String>) -> Self {
        Self { ok: false, status, detail }
    }
}

/// Full readiness report — serialised both by `/api/health/ready` and
/// `/api/health?detail=full`.
#[derive(Debug, Clone, Serialize)]
pub struct ReadinessReport {
    pub status: &'static str, // "ready" | "degraded"
    pub version: &'static str,
    pub database: ComponentStatus,
    pub mailboxes: ComponentStatus,
    pub redis: ComponentStatus,
    pub queue: ComponentStatus,
}

impl ReadinessReport {
    /// PURPOSE: True only when every probed component is healthy. Drives the
    /// HTTP status code on the readiness endpoint (200 vs 503).
    pub fn is_ready(&self) -> bool {
        self.database.ok && self.mailboxes.ok && self.redis.ok && self.queue.ok
    }
}

/// PURPOSE: GET /api/health — kept for back-compat. By default returns the
/// original `{status, version, database}` shape so existing uptime monitors
/// don't break. Passing `?detail=full` returns the structured readiness
/// report (same shape as `/api/health/ready`).
pub async fn health_check(
    State(state): State<AppState>,
    Query(q): Query<HealthQuery>,
) -> Json<Value> {
    if q.detail.as_deref() == Some("full") {
        let report = build_readiness_report(&state).await;
        return Json(serde_json::to_value(report).unwrap_or(Value::Null));
    }

    // Legacy shape — single DB probe, "healthy" / "degraded" status.
    let db_ok = check_database(&state.db).await.ok;
    Json(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "database": if db_ok { "connected" } else { "disconnected" },
    }))
}

/// PURPOSE: GET /api/health/live — liveness probe.
///
/// Only checks the process is up and can hit Postgres. Used by orchestrators
/// (systemd, Kubernetes) to decide whether to restart the binary. Redis
/// failure must NOT flip liveness — restarting the pod won't bring Redis back.
pub async fn liveness(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let db = check_database(&state.db).await;
    let code = if db.ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = json!({
        "status": if db.ok { "alive" } else { "down" },
        "version": env!("CARGO_PKG_VERSION"),
        "database": serde_json::to_value(&db).unwrap_or(Value::Null),
    });
    (code, Json(body))
}

/// PURPOSE: GET /api/health/ready — readiness probe.
///
/// Returns 503 when any of {DB, Redis, queue heartbeat, mailbox table} is
/// unhealthy, so load balancers stop sending traffic to a broken instance.
pub async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let report = build_readiness_report(&state).await;
    let code = if report.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(serde_json::to_value(report).unwrap_or(Value::Null)))
}

/// PURPOSE: Aggregate every component probe into a single report. Exposed at
/// `pub(crate)` so tests can call it directly without the HTTP layer.
pub(crate) async fn build_readiness_report(state: &AppState) -> ReadinessReport {
    // Run all four probes concurrently — total wall time = max(probes), not sum.
    let (database, mailboxes, redis, queue) = tokio::join!(
        check_database(&state.db),
        check_mailboxes(&state.db),
        check_redis(&state.cache),
        async { check_queue_heartbeat(&state.queue_heartbeat) },
    );

    let is_ready = database.ok && mailboxes.ok && redis.ok && queue.ok;
    ReadinessReport {
        status: if is_ready { "ready" } else { "degraded" },
        version: env!("CARGO_PKG_VERSION"),
        database,
        mailboxes,
        redis,
        queue,
    }
}

// -------------------- per-component probes --------------------

async fn check_database(db: &sqlx::PgPool) -> ComponentStatus {
    match sqlx::query("SELECT 1").execute(db).await {
        Ok(_) => ComponentStatus::ok("connected", None),
        Err(e) => ComponentStatus::down("disconnected", Some(e.to_string())),
    }
}

async fn check_mailboxes(db: &sqlx::PgPool) -> ComponentStatus {
    // PURPOSE: Confirms the `mailboxes` table is queryable and reports the
    // active-mailbox count. A 0 count is still healthy — fresh deployments
    // legitimately have no mailboxes yet.
    match sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM mailboxes WHERE active = true",
    )
    .fetch_one(db)
    .await
    {
        Ok(n) => ComponentStatus::ok("ok", Some(format!("{} active mailboxes", n))),
        Err(e) => ComponentStatus::down("error", Some(e.to_string())),
    }
}

async fn check_redis(cache: &CacheService) -> ComponentStatus {
    if !cache.is_connected().await {
        // Passthrough mode — the cache was never wired (e.g. Redis URL bad
        // at startup). Treated as disconnected for readiness.
        return ComponentStatus::down(
            "disconnected",
            Some("Redis client not initialised".to_string()),
        );
    }
    match cache.ping().await {
        Ok(()) => ComponentStatus::ok("connected", None),
        Err(reason) => ComponentStatus::down("disconnected", Some(reason)),
    }
}

fn check_queue_heartbeat(hb: &QueueHeartbeat) -> ComponentStatus {
    match hb.seconds_since_tick() {
        Some(secs) if secs <= QUEUE_STALL_THRESHOLD_SECS => ComponentStatus::ok(
            "ok",
            Some(format!("{}s since last tick", secs)),
        ),
        Some(secs) => ComponentStatus::down(
            "stalled",
            Some(format!(
                "{}s since last tick (threshold {}s)",
                secs, QUEUE_STALL_THRESHOLD_SECS
            )),
        ),
        None => ComponentStatus::down(
            "not_started",
            Some("processor has not ticked yet".to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_heartbeat_check_reports_not_started() {
        let hb = QueueHeartbeat::new();
        let cs = check_queue_heartbeat(&hb);
        assert!(!cs.ok);
        assert_eq!(cs.status, "not_started");
    }

    #[test]
    fn queue_heartbeat_check_reports_ok_after_tick() {
        let hb = QueueHeartbeat::new();
        hb.record_tick();
        let cs = check_queue_heartbeat(&hb);
        assert!(cs.ok, "expected ok component, got {:?}", cs);
        assert_eq!(cs.status, "ok");
        assert!(cs.detail.as_deref().unwrap_or("").contains("since last tick"));
    }

    #[test]
    fn component_status_serialises_with_kebab_fields() {
        let cs = ComponentStatus::ok("connected", Some("hello".into()));
        let v = serde_json::to_value(&cs).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["status"], "connected");
        assert_eq!(v["detail"], "hello");
    }

    #[test]
    fn component_status_omits_detail_when_none() {
        let cs = ComponentStatus::ok("connected", None);
        let v = serde_json::to_value(&cs).unwrap();
        assert!(v.get("detail").is_none(), "detail must be omitted when None");
    }

    #[test]
    fn readiness_report_is_ready_only_when_all_components_ok() {
        let ok = ComponentStatus::ok("connected", None);
        let down = ComponentStatus::down("disconnected", Some("err".into()));
        let all_ok = ReadinessReport {
            status: "ready",
            version: env!("CARGO_PKG_VERSION"),
            database: ok.clone(),
            mailboxes: ok.clone(),
            redis: ok.clone(),
            queue: ok.clone(),
        };
        assert!(all_ok.is_ready());

        let one_down = ReadinessReport {
            status: "degraded",
            version: env!("CARGO_PKG_VERSION"),
            database: ok.clone(),
            mailboxes: ok.clone(),
            redis: down,
            queue: ok,
        };
        assert!(!one_down.is_ready());
    }

    // PURPOSE: Lock in the stall threshold so we don't accidentally relax it
    // and miss a stalled processor for 5+ minutes in production.
    #[test]
    fn queue_stall_threshold_is_within_expected_range() {
        assert!(QUEUE_STALL_THRESHOLD_SECS >= 15);
        assert!(QUEUE_STALL_THRESHOLD_SECS <= 120);
    }

    #[tokio::test]
    async fn check_redis_reports_disconnected_when_cache_disabled() {
        // Mirrors the production "Redis down at boot" path: passthrough mode.
        let cache = CacheService::disabled();
        let cs = check_redis(&cache).await;
        assert!(!cs.ok);
        assert_eq!(cs.status, "disconnected");
    }
}
