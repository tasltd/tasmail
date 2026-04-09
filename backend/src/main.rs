mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod router;
mod services;
mod state;

use std::net::SocketAddr;
use std::path::Path;

use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tasmail=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

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
