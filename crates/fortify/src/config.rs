//! Configuration management for Fortify.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use cerberus_common::constants::{DEFAULT_LISTEN_ADDR, DEFAULT_REDIS_URL};

/// Application configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// Redis connection URL
    #[serde(default = "default_redis_url")]
    pub redis_url: String,

    /// HTTP listen address
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Initial threat level (0-10)
    #[serde(default = "default_threat_level")]
    pub initial_threat_level: u8,

    /// Enable cluster mode
    #[serde(default)]
    pub cluster_enabled: bool,

    /// This node's unique ID (auto-generated if not set)
    #[serde(default = "generate_node_id")]
    pub node_id: String,

    /// CAPTCHA configuration
    #[serde(default)]
    pub captcha: CaptchaConfig,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

/// CAPTCHA-specific configuration
#[derive(Debug, Clone, Deserialize)]
pub struct CaptchaConfig {
    /// Path to font file for CAPTCHA text
    #[serde(default = "default_font_path")]
    pub font_path: String,

    /// Passport token validity in seconds
    #[serde(default = "default_passport_ttl")]
    pub passport_ttl_secs: u64,

    /// Challenge validity in seconds
    #[serde(default = "default_challenge_ttl")]
    pub challenge_ttl_secs: u64,
}

impl Default for CaptchaConfig {
    fn default() -> Self {
        Self {
            font_path: default_font_path(),
            passport_ttl_secs: default_passport_ttl(),
            challenge_ttl_secs: default_challenge_ttl(),
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per minute per circuit
    #[serde(default = "default_max_requests")]
    pub max_requests_per_minute: u32,

    /// Maximum failed CAPTCHAs before soft-lock
    #[serde(default = "default_max_failures")]
    pub max_failed_attempts: u32,

    /// Soft-lock duration in seconds
    #[serde(default = "default_soft_lock")]
    pub soft_lock_duration_secs: u64,

    /// Ban duration in seconds
    #[serde(default = "default_ban_duration")]
    pub ban_duration_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests_per_minute: default_max_requests(),
            max_failed_attempts: default_max_failures(),
            soft_lock_duration_secs: default_soft_lock(),
            ban_duration_secs: default_ban_duration(),
        }
    }
}

// Default value functions
fn default_redis_url() -> String { DEFAULT_REDIS_URL.to_string() }
fn default_listen_addr() -> String { DEFAULT_LISTEN_ADDR.to_string() }
fn default_threat_level() -> u8 { 5 }
fn default_font_path() -> String { "assets/fonts/DejaVuSans.ttf".to_string() }
fn default_passport_ttl() -> u64 { 600 } // 10 minutes
fn default_challenge_ttl() -> u64 { 300 } // 5 minutes
fn default_max_requests() -> u32 { 60 }
fn default_max_failures() -> u32 { 5 }
fn default_soft_lock() -> u64 { 1800 } // 30 minutes
fn default_ban_duration() -> u64 { 3600 } // 1 hour

fn generate_node_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    format!("node-{:08x}", rng.random::<u32>())
}

impl AppConfig {
    /// Load configuration from file, with CLI overrides
    pub fn load(config_path: &str, args: &super::Args) -> Result<Self> {
        let mut config = if Path::new(config_path).exists() {
            let settings = config::Config::builder()
                .add_source(config::File::with_name(config_path))
                .build()
                .context("Failed to load config file")?;

            settings
                .try_deserialize()
                .context("Failed to parse config")?
        } else {
            // Use defaults if config file doesn't exist
            tracing::warn!("Config file not found, using defaults");
            Self::default()
        };

        // Apply CLI overrides
        if let Some(ref redis_url) = args.redis_url {
            config.redis_url = redis_url.clone();
        }
        if let Some(ref listen) = args.listen {
            config.listen_addr = listen.clone();
        }

        Ok(config)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            redis_url: default_redis_url(),
            listen_addr: default_listen_addr(),
            initial_threat_level: default_threat_level(),
            cluster_enabled: false,
            node_id: generate_node_id(),
            captcha: CaptchaConfig::default(),
            rate_limit: RateLimitConfig::default(),
        }
    }
}
