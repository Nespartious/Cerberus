//! Common error types for Cerberus components.

use thiserror::Error;

/// Common errors across Cerberus components
#[derive(Debug, Error)]
pub enum CerberusError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Redis connection/operation error
    #[error("Redis error: {0}")]
    Redis(String),

    /// CAPTCHA generation/verification error
    #[error("CAPTCHA error: {0}")]
    Captcha(String),

    /// Circuit tracking error
    #[error("Circuit tracking error: {0}")]
    CircuitTracking(String),

    /// Authentication/authorization error
    #[error("Auth error: {0}")]
    Auth(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimited(String),

    /// Circuit is banned
    #[error("Circuit banned: {0}")]
    Banned(String),

    /// Invalid input/request
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Cluster coordination error
    #[error("Cluster error: {0}")]
    Cluster(String),

    /// Timeout
    #[error("Operation timed out: {0}")]
    Timeout(String),
}

impl CerberusError {
    /// Returns the HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Config(_) => 500,
            Self::Redis(_) => 503,
            Self::Captcha(_) => 500,
            Self::CircuitTracking(_) => 500,
            Self::Auth(_) => 401,
            Self::RateLimited(_) => 429,
            Self::Banned(_) => 403,
            Self::InvalidInput(_) => 400,
            Self::Internal(_) => 500,
            Self::Cluster(_) => 503,
            Self::Timeout(_) => 504,
        }
    }

    /// Returns true if this error should be retried
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Redis(_) | Self::Cluster(_) | Self::Timeout(_))
    }
}
