//! CAPTCHA generation and verification.
//!
//! MVP Implementation: Simple text-based placeholder CAPTCHA.
//! Production: Will use image-based grid challenges.

mod generator;
mod verifier;

pub use generator::CaptchaGenerator;
pub use verifier::CaptchaVerifier;

use cerberus_common::CaptchaDifficulty;
use serde::{Deserialize, Serialize};

/// Stored challenge data in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredChallenge {
    /// The expected answer (positions or text)
    pub answer: String,
    /// Circuit ID that requested this challenge
    pub circuit_id: Option<String>,
    /// Difficulty level
    pub difficulty: CaptchaDifficulty,
    /// Creation timestamp
    pub created_at: i64,
    /// Expiry timestamp
    pub expires_at: i64,
}
