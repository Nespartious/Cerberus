//! Core types shared across Cerberus components.

use serde::{Deserialize, Serialize};

/// Threat Dial Level (0-10)
/// Controls the aggressiveness of CAPTCHA challenges.
///
/// - 0: No CAPTCHAs (development only)
/// - 1-3: Light protection (low traffic)
/// - 4-6: Standard protection (normal operation)
/// - 7-9: High protection (under attack)
/// - 10: Maximum lockdown (emergency)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThreatLevel(u8);

impl ThreatLevel {
    pub const MIN: ThreatLevel = ThreatLevel(0);
    pub const MAX: ThreatLevel = ThreatLevel(10);
    pub const DEFAULT: ThreatLevel = ThreatLevel(5);

    /// Create a new ThreatLevel, clamping to valid range [0, 10]
    pub fn new(level: u8) -> Self {
        Self(level.min(10))
    }

    pub fn value(&self) -> u8 {
        self.0
    }

    /// Returns true if this level requires a CAPTCHA challenge
    pub fn requires_captcha(&self) -> bool {
        self.0 > 0
    }

    /// Returns the number of CAPTCHAs required at this threat level
    pub fn captcha_count(&self) -> u8 {
        match self.0 {
            0 => 0,
            1..=3 => 1,
            4..=6 => 2,
            7..=9 => 3,
            10 => 5, // Maximum lockdown
            _ => unreachable!(),
        }
    }

    /// Returns the CAPTCHA difficulty (grid size) at this level
    pub fn captcha_difficulty(&self) -> CaptchaDifficulty {
        match self.0 {
            0..=3 => CaptchaDifficulty::Easy,
            4..=6 => CaptchaDifficulty::Medium,
            7..=9 => CaptchaDifficulty::Hard,
            10 => CaptchaDifficulty::Extreme,
            _ => unreachable!(),
        }
    }
}

impl Default for ThreatLevel {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl From<u8> for ThreatLevel {
    fn from(value: u8) -> Self {
        Self::new(value)
    }
}

/// CAPTCHA difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptchaDifficulty {
    /// 2x2 grid, simple distortion
    Easy,
    /// 3x3 grid, moderate distortion
    Medium,
    /// 4x4 grid, heavy distortion
    Hard,
    /// 5x5 grid, extreme distortion + time pressure
    Extreme,
}

impl CaptchaDifficulty {
    pub fn grid_size(&self) -> (u8, u8) {
        match self {
            Self::Easy => (2, 2),
            Self::Medium => (3, 3),
            Self::Hard => (4, 4),
            Self::Extreme => (5, 5),
        }
    }

    /// Timeout in seconds for this difficulty
    pub fn timeout_secs(&self) -> u32 {
        match self {
            Self::Easy => 60,
            Self::Medium => 45,
            Self::Hard => 30,
            Self::Extreme => 20,
        }
    }
}

/// Circuit state in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CircuitStatus {
    /// New circuit, never seen before
    New,
    /// Passed CAPTCHA, has valid passport
    Verified,
    /// Failed too many CAPTCHAs, soft-locked
    SoftLocked,
    /// Confirmed malicious, banned
    Banned,
    /// VIP status (verified + good behavior)
    Vip,
}

impl Default for CircuitStatus {
    fn default() -> Self {
        Self::New
    }
}

/// Represents a Tor circuit's identity and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitInfo {
    /// Unique circuit identifier (from Tor/HAProxy)
    pub circuit_id: String,

    /// Current status
    pub status: CircuitStatus,

    /// Number of failed CAPTCHA attempts
    pub failed_attempts: u32,

    /// Number of successful CAPTCHA solves
    pub successful_solves: u32,

    /// Timestamp of first seen (Unix epoch seconds)
    pub first_seen: i64,

    /// Timestamp of last activity
    pub last_seen: i64,

    /// Passport token (if verified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passport_token: Option<String>,

    /// Passport expiry timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passport_expires: Option<i64>,
}

impl CircuitInfo {
    pub fn new(circuit_id: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            circuit_id,
            status: CircuitStatus::New,
            failed_attempts: 0,
            successful_solves: 0,
            first_seen: now,
            last_seen: now,
            passport_token: None,
            passport_expires: None,
        }
    }

    /// Check if the passport is currently valid
    pub fn has_valid_passport(&self) -> bool {
        match (self.passport_token.as_ref(), self.passport_expires) {
            (Some(_), Some(expires)) => {
                let now = chrono::Utc::now().timestamp();
                now < expires
            }
            _ => false,
        }
    }

    /// Check if this circuit should be rate-limited
    pub fn should_rate_limit(&self) -> bool {
        matches!(self.status, CircuitStatus::SoftLocked | CircuitStatus::Banned)
    }
}

/// CAPTCHA challenge data sent to the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaChallenge {
    /// Unique challenge ID
    pub challenge_id: String,

    /// Base64-encoded PNG image
    pub image_data: String,

    /// Grid dimensions (cols, rows)
    pub grid_size: (u8, u8),

    /// Instructions for the user
    pub instructions: String,

    /// Expected click positions (server-side only, not sent to client)
    #[serde(skip_serializing)]
    pub expected_positions: Vec<(u8, u8)>,

    /// Challenge expiry timestamp
    pub expires_at: i64,
}

/// CAPTCHA verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaResult {
    pub success: bool,
    pub remaining_challenges: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passport_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Cluster node state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Node unique identifier
    pub node_id: String,

    /// Node hostname or IP
    pub address: String,

    /// WireGuard endpoint
    pub wireguard_endpoint: String,

    /// Is this node healthy?
    pub healthy: bool,

    /// Last heartbeat timestamp
    pub last_heartbeat: i64,

    /// Current threat level on this node
    pub threat_level: ThreatLevel,
}

/// Metrics snapshot for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Requests per second (last minute avg)
    pub requests_per_second: f64,

    /// Active circuits being tracked
    pub active_circuits: u64,

    /// CAPTCHAs served in last minute
    pub captchas_served: u64,

    /// CAPTCHAs passed in last minute
    pub captchas_passed: u64,

    /// CAPTCHAs failed in last minute
    pub captchas_failed: u64,

    /// Currently banned circuits
    pub banned_circuits: u64,

    /// Current threat dial level
    pub threat_level: u8,
}
