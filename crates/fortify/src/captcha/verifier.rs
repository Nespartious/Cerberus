//! CAPTCHA verification logic.

use anyhow::Result;
use cerberus_common::{CaptchaDifficulty, CaptchaResult};
use redis::AsyncCommands;

use super::StoredChallenge;

/// CAPTCHA verifier service
pub struct CaptchaVerifier {
    /// Passport TTL in seconds
    pub passport_ttl: u64,
}

impl CaptchaVerifier {
    pub fn new(passport_ttl: u64) -> Self {
        Self { passport_ttl }
    }

    /// Verify a CAPTCHA response
    ///
    /// Returns (success, remaining_challenges, passport_token)
    pub async fn verify(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        challenge_id: &str,
        user_answer: &str,
        circuit_id: Option<&str>,
    ) -> Result<CaptchaResult> {
        let key = format!("captcha:{}", challenge_id);

        // Fetch and delete challenge (single-use)
        // Use GET + DEL for Redis 3.x compatibility (GETDEL requires Redis 6.2+)
        let stored: Option<String> = redis.get(&key).await?;
        let _: () = redis.del(&key).await?;

        let stored = match stored {
            Some(s) => s,
            None => {
                return Ok(CaptchaResult {
                    success: false,
                    remaining_challenges: 0,
                    passport_token: None,
                    error_message: Some("Challenge expired or invalid".to_string()),
                });
            }
        };

        let challenge: StoredChallenge = serde_json::from_str(&stored)?;

        // Check expiry
        let now = chrono::Utc::now().timestamp();
        if now > challenge.expires_at {
            return Ok(CaptchaResult {
                success: false,
                remaining_challenges: 0,
                passport_token: None,
                error_message: Some("Challenge expired".to_string()),
            });
        }

        // Verify circuit ID matches (if provided)
        if let (Some(stored_cid), Some(request_cid)) = (&challenge.circuit_id, circuit_id) {
            if stored_cid != request_cid {
                tracing::warn!(
                    challenge_id = %challenge_id,
                    stored_circuit = %stored_cid,
                    request_circuit = %request_cid,
                    "Circuit ID mismatch"
                );
                // Don't fail - circuits can change, just log it
            }
        }

        // Compare answers (case-insensitive for Easy/Medium)
        let success = match challenge.difficulty {
            CaptchaDifficulty::Easy | CaptchaDifficulty::Medium => {
                user_answer.to_uppercase() == challenge.answer.to_uppercase()
            }
            CaptchaDifficulty::Hard | CaptchaDifficulty::Extreme => {
                user_answer == challenge.answer
            }
        };

        if success {
            // Generate passport token
            let passport_token = self.generate_passport_token();

            // Store passport in Redis
            let passport_key = format!("passport:{}", passport_token);
            let passport_data = serde_json::json!({
                "circuit_id": circuit_id,
                "issued_at": now,
                "expires_at": now + self.passport_ttl as i64,
            });

            redis
                .set_ex::<_, _, ()>(&passport_key, passport_data.to_string(), self.passport_ttl)
                .await?;

            tracing::info!(
                challenge_id = %challenge_id,
                circuit_id = ?circuit_id,
                "CAPTCHA verified successfully"
            );

            Ok(CaptchaResult {
                success: true,
                remaining_challenges: 0,
                passport_token: Some(passport_token),
                error_message: None,
            })
        } else {
            tracing::debug!(
                challenge_id = %challenge_id,
                circuit_id = ?circuit_id,
                "CAPTCHA verification failed"
            );

            Ok(CaptchaResult {
                success: false,
                remaining_challenges: 1, // They need to try again
                passport_token: None,
                error_message: Some("Incorrect answer".to_string()),
            })
        }
    }

    /// Generate a cryptographically secure passport token
    fn generate_passport_token(&self) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;

        let mut bytes = [0u8; 32];
        rand::Rng::fill(&mut rand::rng(), &mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Validate an existing passport token
    pub async fn validate_passport(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        token: &str,
    ) -> Result<bool> {
        let key = format!("passport:{}", token);
        let exists: bool = redis.exists(&key).await?;

        if exists {
            // Update last-seen (touch the key)
            let ttl: i64 = redis.ttl(&key).await?;
            if ttl > 0 {
                // Refresh TTL on valid access
                redis.expire::<_, ()>(&key, ttl).await?;
            }
        }

        Ok(exists)
    }
}
