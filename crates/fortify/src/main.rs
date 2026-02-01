//! # Fortify - Cerberus L7+ Logic Engine
//!
//! The brain of Cerberus. Handles CAPTCHA generation/verification,
//! threat dial management, circuit tracking, and cluster coordination.
//!
//! ## Architecture
//! ```text
//! HAProxy → Nginx → Fortify → Backend
//!                      ↓
//!                   Redis (State)
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod captcha;
mod circuits;
mod config;
mod routes;
mod state;

use config::AppConfig;
use state::AppState;

/// Cerberus Fortify - L7+ Logic Engine
#[derive(Parser, Debug)]
#[command(name = "fortify")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config/fortify.toml")]
    config: String,

    /// Redis URL (overrides config)
    #[arg(long, env = "REDIS_URL")]
    redis_url: Option<String>,

    /// Listen address (overrides config)
    #[arg(short, long, env = "LISTEN_ADDR")]
    listen: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "LOG_LEVEL")]
    log_level: String,

    /// Enable JSON logging output
    #[arg(long, default_value = "false")]
    json_logs: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize logging
    init_logging(&args.log_level, args.json_logs)?;

    info!("🔥 Starting Cerberus Fortify v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = AppConfig::load(&args.config, &args)?;
    info!("📋 Configuration loaded from {}", args.config);

    // Initialize application state
    let state = AppState::new(config.clone()).await?;
    info!("✅ Redis connected: {}", config.redis_url);

    // Build router
    let app = routes::create_router(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!("🚀 Fortify listening on {}", config.listen_addr);

    axum::serve(listener, app)
        .await
        .context("Server error")?;

    Ok(())
}

/// Initialize structured logging with tracing
fn init_logging(level: &str, json: bool) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true).with_thread_ids(true))
            .init();
    }

    Ok(())
}
