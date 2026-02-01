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
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

mod captcha;
mod circuits;
mod cluster;
mod config;
mod haproxy;
mod routes;
mod state;

use captcha::{AmmoBox, AmmoBoxConfig, ammo_box_worker};
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

    info!(
        "🔥 Starting Cerberus Fortify v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration
    let config = AppConfig::load(&args.config, &args)?;
    info!("📋 Configuration loaded from {}", args.config);

    // Create shutdown broadcast channel
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);

    // Initialize Ammo Box (pre-generated CAPTCHA pool)
    let ammo_config = AmmoBoxConfig {
        ram_capacity: 10_000,
        ..Default::default()
    };
    let ammo_box = Arc::new(AmmoBox::new(ammo_config));

    // Spawn Ammo Box background worker
    let ammo_clone = ammo_box.clone();
    let ammo_shutdown = shutdown_tx.subscribe();
    tokio::spawn(async move {
        ammo_box_worker(ammo_clone, ammo_shutdown).await;
    });

    // Initialize application state
    let state = AppState::new(config.clone(), ammo_box).await?;
    info!("✅ Redis connected: {}", config.redis_url);

    // Build router
    let app = routes::create_router(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!("🚀 Fortify listening on {}", config.listen_addr);

    // Handle graceful shutdown
    let shutdown_signal = async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        info!("🛑 Shutdown signal received");
        let _ = shutdown_tx.send(());
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .context("Server error")?;

    info!("👋 Fortify shutdown complete");
    Ok(())
}

/// Initialize structured logging with tracing
fn init_logging(level: &str, json: bool) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

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
