//! Application state and shared resources.

use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use std::sync::Arc;
use tokio::sync::RwLock;

use cerberus_common::ThreatLevel;
use crate::config::AppConfig;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Application configuration
    pub config: AppConfig,

    /// Redis connection manager (auto-reconnecting)
    pub redis: ConnectionManager,

    /// Current threat level (cached locally, synced with Redis)
    pub threat_level: Arc<RwLock<ThreatLevel>>,

    /// Node identifier for clustering
    pub node_id: String,
}

impl AppState {
    /// Create new application state, connecting to Redis
    pub async fn new(config: AppConfig) -> Result<Self> {
        // Connect to Redis with connection manager (handles reconnection)
        let client = redis::Client::open(config.redis_url.as_str())
            .context("Failed to create Redis client")?;

        let redis = ConnectionManager::new(client)
            .await
            .context("Failed to connect to Redis")?;

        let threat_level = Arc::new(RwLock::new(ThreatLevel::new(config.initial_threat_level)));
        let node_id = config.node_id.clone();

        Ok(Self {
            config,
            redis,
            threat_level,
            node_id,
        })
    }

    /// Get current threat level
    pub async fn get_threat_level(&self) -> ThreatLevel {
        *self.threat_level.read().await
    }

    /// Update threat level (local + Redis)
    pub async fn set_threat_level(&self, level: ThreatLevel) -> Result<()> {
        use redis::AsyncCommands;

        // Update local cache
        *self.threat_level.write().await = level;

        // Sync to Redis for cluster visibility
        let mut conn = self.redis.clone();
        conn.set(
            cerberus_common::constants::redis_keys::THREAT_LEVEL,
            level.value(),
        )
        .await
        .context("Failed to sync threat level to Redis")?;

        tracing::info!(level = level.value(), "Threat level updated");

        Ok(())
    }
}
