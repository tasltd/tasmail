mod config;
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
    let scheduler = services::email_scheduler::EmailScheduler::new(
        std::sync::Arc::new(pool.clone()),
        config.smtp.clone(),
        5,
    );
    scheduler.start();

    // Changed: Queue processor is now BYOK — it loads each item's per-user SMTP config from
    // smtp_configurations and decrypts the password using the JWT-derived AES key.
    // Production-grade: FOR UPDATE SKIP LOCKED via EmailQueueItem::claim_batch lets multiple
    // worker processes run safely; Prometheus metrics are emitted on every cycle.
    let queue_processor = services::queue_processor::QueueProcessor::new(
        std::sync::Arc::new(pool.clone()),
        config.jwt.clone(),
        5,
    )
    .with_batch_size(50)
    .with_worker_concurrency(4);
    queue_processor.start();

    // Added: Install Prometheus metrics recorder and collect process metrics (TMAIL-41)
    let metrics_handle = handlers::metrics::install_prometheus_recorder();
    // Added: Register process metrics collector (CPU, memory, FDs)
    let process_collector = metrics_process::Collector::default();
    process_collector.describe();
    process_collector.collect();

    // Added: Initialize Redis cache service (degrades gracefully if Redis unavailable)
    let cache = services::cache_service::CacheService::new(&config.redis).await;
    // Added: Encryption service derived from JWT secret — used for DB-stored credentials
    let encryption = services::encryption::EncryptionService::from_jwt_secret(&config.jwt.secret);

    let state = AppState {
        db: pool,
        config,
        // Added: Pass Prometheus handle for /metrics endpoint rendering (TMAIL-41)
        metrics_handle: Some(metrics_handle),
        // Added: Redis cache for branding/quota/rate-limit/session caching
        cache,
        // Added: Encryption service for DB-stored payment credentials
        encryption,
    };

    let app = router::create_router(state);

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
