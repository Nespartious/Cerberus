//! Shared constants for Cerberus components.

/// Default Redis connection URL
pub const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";

/// Default Fortify HTTP listen address
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8888";

/// Default passport token validity (10 minutes)
pub const DEFAULT_PASSPORT_TTL_SECS: u64 = 600;

/// Maximum failed CAPTCHA attempts before soft-lock
pub const MAX_FAILED_ATTEMPTS: u32 = 5;

/// Soft-lock duration in seconds (30 minutes)
pub const SOFT_LOCK_DURATION_SECS: u64 = 1800;

/// Ban duration in seconds (1 hour)
pub const BAN_DURATION_SECS: u64 = 3600;

/// Circuit info expiry in Redis (30 minutes)
pub const CIRCUIT_TTL_SECS: u64 = 1800;

/// CAPTCHA challenge expiry in Redis (5 minutes)
pub const CAPTCHA_TTL_SECS: u64 = 300;

/// Cluster heartbeat interval (seconds)
pub const CLUSTER_HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// Cluster node timeout (seconds)
pub const CLUSTER_NODE_TIMEOUT_SECS: u64 = 15;

/// Redis key prefixes
pub mod redis_keys {
    /// Circuit info: circuit:{circuit_id}
    pub const CIRCUIT_PREFIX: &str = "circuit:";

    /// CAPTCHA challenge: captcha:{challenge_id}
    pub const CAPTCHA_PREFIX: &str = "captcha:";

    /// Passport token: passport:{token}
    pub const PASSPORT_PREFIX: &str = "passport:";

    /// Global threat level
    pub const THREAT_LEVEL: &str = "cerberus:threat_level";

    /// Cluster state: cluster:node:{node_id}
    pub const CLUSTER_NODE_PREFIX: &str = "cluster:node:";

    /// Metrics: metrics:{metric_name}
    pub const METRICS_PREFIX: &str = "metrics:";

    /// Rate limit counters: ratelimit:{circuit_id}
    pub const RATELIMIT_PREFIX: &str = "ratelimit:";
}

/// HTTP header names
pub mod headers {
    /// Circuit ID header (from HAProxy/Tor)
    pub const X_CIRCUIT_ID: &str = "X-Circuit-Id";

    /// Passport token header
    pub const X_PASSPORT_TOKEN: &str = "X-Passport-Token";

    /// Threat level header (internal)
    pub const X_THREAT_LEVEL: &str = "X-Threat-Level";

    /// Node ID header (cluster internal)
    pub const X_NODE_ID: &str = "X-Node-Id";
}
