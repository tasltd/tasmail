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

    // Added: Start background queue processor for retry-enabled email sending (TMAIL-58)
    let queue_processor = services::queue_processor::QueueProcessor::new(
        std::sync::Arc::new(pool.clone()),
        config.smtp.clone(),
        5,
    );
    queue_processor.start();

    let state = AppState {
        db: pool,
        config,
    };

    let app = router::create_router(state);

    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
