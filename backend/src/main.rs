mod config;
// Added (TMAIL-308): Multi-origin CORS parser with wildcard support.
mod cors;
mod error;
mod handlers;
mod middleware;
mod models;
mod router;
mod services;
mod state;
// Added: Centralized input validation module for security hardening (TMAIL-37)
mod validation;

use std::net::SocketAddr;
use std::path::Path;

use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with JSON format in production
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "tasmail=debug,tower_http=debug".into());

    let use_json = std::env::var("LOG_FORMAT").map(|v| v == "json").unwrap_or(false);

    if use_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json().with_target(true))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    dotenvy::dotenv().ok();

    // Load configuration
    let config = if Path::new("config.toml").exists() {
        Config::load(Path::new("config.toml"))?
    } else {
        tracing::info!("No config.toml found, loading from environment variables");
        Config::from_env()?
    };

    tracing::info!(
        "Starting TASMail backend on {}:{}",
        config.server.host,
        config.server.port
    );

    // Set up database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&config.database.url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    tracing::info!("Database connected and migrations applied");

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;

    // Start background email scheduler (polls every 5 seconds)
    // Changed (TMAIL-301): scheduler now takes JwtConfig instead of SmtpConfig.
    // It loads per-user SMTP credentials from `smtp_configurations` and decrypts
    // them with the JWT-derived AES-256-GCM key (BYOK). The previous wiring
    // passed the server's notification SmtpConfig plus a literal "placeholder"
    // password, so every scheduled send was rejected at SMTP AUTH.
    let scheduler = services::email_scheduler::EmailScheduler::new(
        std::sync::Arc::new(pool.clone()),
        config.jwt.clone(),
        5,
    );
    scheduler.start();

    // TMAIL-158: Initialize Redis cache up-front so the queue processor can share it
    // with the HTTP handlers — both read per-user SMTP config and we want a single
    // cache namespace so invalidations from the API immediately invalidate the queue's view.
    // Degrades gracefully (passthrough mode) when Redis is unreachable.
    let cache = services::cache_service::CacheService::new(&config.redis).await;

    // Changed: Queue processor is now BYOK — it loads each item's per-user SMTP config from
    // smtp_configurations and decrypts the password using the JWT-derived AES key.
    // Production-grade: FOR UPDATE SKIP LOCKED via EmailQueueItem::claim_batch lets multiple
    // worker processes run safely; Prometheus metrics are emitted on every cycle.
    // TMAIL-158: pass the shared CacheService so per-user SMTP rows are cached for 5 min.
    let queue_processor = services::queue_processor::QueueProcessor::new(
        std::sync::Arc::new(pool.clone()),
        config.jwt.clone(),
        cache.clone(),
        5,
    )
    .with_batch_size(50)
    .with_worker_concurrency(4);
    queue_processor.start();

    // TMAIL-177: usage-based billing rollup. Wakes daily by default; the operator can
    // bump TASMAIL_BILLING_ROLLUP_SECS for testing (e.g., 60s in dev).
    let rollup_secs: u64 = std::env::var("TASMAIL_BILLING_ROLLUP_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(86_400);
    services::billing_rollup::BillingRollup::new(std::sync::Arc::new(pool.clone()), rollup_secs).start();

    // Added: Install Prometheus metrics recorder and collect process metrics (TMAIL-41)
    let metrics_handle = handlers::metrics::install_prometheus_recorder();
    // Added: Register process metrics collector (CPU, memory, FDs)
    let process_collector = metrics_process::Collector::default();
    process_collector.describe();
    process_collector.collect();

    // Changed (TMAIL-158): cache is initialized earlier so the queue processor can share it.
    // Added: Encryption service derived from JWT secret — used for DB-stored credentials
    let encryption = services::encryption::EncryptionService::from_jwt_secret(&config.jwt.secret);

    // TMAIL-306: shared Arc<OnceLock<Router>> handle. The `state` field below holds
    // one clone of this Arc; the router we build below captures another clone of the
    // same Arc inside its state. After `create_router` returns, we `set()` the wired
    // router on the OnceLock, which both observers then see. This lets the
    // `/api/mobile/batch` handler dispatch sub-requests through the same router that
    // serves public traffic without restructuring the startup order.
    let inner_router_holder: std::sync::Arc<std::sync::OnceLock<axum::Router>> =
        std::sync::Arc::new(std::sync::OnceLock::new());

    let state = AppState {
        db: pool,
        config,
        // Added: Pass Prometheus handle for /metrics endpoint rendering (TMAIL-41)
        metrics_handle: Some(metrics_handle),
        // Added: Redis cache for branding/quota/rate-limit/session caching
        cache,
        // Added: Encryption service for DB-stored payment credentials
        encryption,
        // Added (TMAIL-306): empty-on-bootstrap, populated below for batch dispatch.
        inner_router: inner_router_holder.clone(),
    };

    let app = router::create_router(state);
    // TMAIL-306: publish the wired router so the batch handler can dispatch through
    // the same middleware/handler chain. `set` is infallible here — we're the only
    // writer and we hold the only Arc clone outside the router itself.
    let _ = inner_router_holder.set(app.clone());

    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // into_make_service_with_connect_info exposes the client SocketAddr to handlers
    // (used by ConnectInfo<SocketAddr> in handlers::enterprise_quote, TMAIL-182).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
