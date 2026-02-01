//! Circuit state tracking with Redis backend.

use anyhow::Result;
use cerberus_common::{CircuitInfo, CircuitStatus};
use redis::AsyncCommands;

/// Circuit tracking service
pub struct CircuitTracker {
    /// Circuit state TTL in seconds
    circuit_ttl: u64,
    /// Max failed attempts before soft-lock
    max_failed_attempts: u32,
    /// Soft-lock duration in seconds
    soft_lock_duration: u64,
    /// Ban duration in seconds
    ban_duration: u64,
}

impl CircuitTracker {
    pub fn new(
        circuit_ttl: u64,
        max_failed_attempts: u32,
        soft_lock_duration: u64,
        ban_duration: u64,
    ) -> Self {
        Self {
            circuit_ttl,
            max_failed_attempts,
            soft_lock_duration,
            ban_duration,
        }
    }

    /// Get or create circuit info
    pub async fn get_or_create(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: &str,
    ) -> Result<CircuitInfo> {
        let key = format!("circuit:{}", circuit_id);

        // Try to get existing
        let existing: Option<String> = redis.get(&key).await?;

        if let Some(data) = existing {
            let mut info: CircuitInfo = serde_json::from_str(&data)?;
            info.last_seen = chrono::Utc::now().timestamp();

            // Update last_seen
            self.save(redis, &info).await?;

            return Ok(info);
        }

        // Create new circuit
        let info = CircuitInfo::new(circuit_id.to_string());
        self.save(redis, &info).await?;

        tracing::debug!(circuit_id = %circuit_id, "New circuit tracked");

        Ok(info)
    }

    /// Get circuit info (if exists)
    pub async fn get(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: &str,
    ) -> Result<Option<CircuitInfo>> {
        let key = format!("circuit:{}", circuit_id);
        let data: Option<String> = redis.get(&key).await?;

        match data {
            Some(d) => Ok(Some(serde_json::from_str(&d)?)),
            None => Ok(None),
        }
    }

    /// Save circuit info to Redis
    pub async fn save(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        info: &CircuitInfo,
    ) -> Result<()> {
        let key = format!("circuit:{}", info.circuit_id);
        let data = serde_json::to_string(info)?;

        // Determine TTL based on status
        let ttl = match info.status {
            CircuitStatus::Banned => self.ban_duration,
            CircuitStatus::SoftLocked => self.soft_lock_duration,
            _ => self.circuit_ttl,
        };

        redis.set_ex::<_, _, ()>(&key, &data, ttl).await?;

        Ok(())
    }

    /// Record a failed CAPTCHA attempt
    pub async fn record_failure(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: &str,
    ) -> Result<CircuitInfo> {
        let mut info = self.get_or_create(redis, circuit_id).await?;

        info.failed_attempts += 1;
        info.last_seen = chrono::Utc::now().timestamp();

        // Check if should be soft-locked
        if info.failed_attempts >= self.max_failed_attempts {
            info.status = CircuitStatus::SoftLocked;
            tracing::warn!(
                circuit_id = %circuit_id,
                failed_attempts = info.failed_attempts,
                "Circuit soft-locked due to failed attempts"
            );
        }

        self.save(redis, &info).await?;

        Ok(info)
    }

    /// Record a successful CAPTCHA solve
    pub async fn record_success(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: &str,
        passport_token: &str,
        passport_expires: i64,
    ) -> Result<CircuitInfo> {
        let mut info = self.get_or_create(redis, circuit_id).await?;

        info.successful_solves += 1;
        info.status = CircuitStatus::Verified;
        info.passport_token = Some(passport_token.to_string());
        info.passport_expires = Some(passport_expires);
        info.last_seen = chrono::Utc::now().timestamp();

        // Reset failed attempts on success
        info.failed_attempts = 0;

        // Check for VIP upgrade (e.g., 5+ successful solves)
        if info.successful_solves >= 5 && info.status == CircuitStatus::Verified {
            info.status = CircuitStatus::Vip;
            tracing::info!(circuit_id = %circuit_id, "Circuit upgraded to VIP");
        }

        self.save(redis, &info).await?;

        Ok(info)
    }

    /// Ban a circuit
    pub async fn ban(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: &str,
        reason: &str,
    ) -> Result<()> {
        let mut info = self.get_or_create(redis, circuit_id).await?;

        info.status = CircuitStatus::Banned;
        info.last_seen = chrono::Utc::now().timestamp();

        self.save(redis, &info).await?;

        tracing::warn!(
            circuit_id = %circuit_id,
            reason = %reason,
            "Circuit banned"
        );

        Ok(())
    }

    /// Check if circuit is allowed to make requests
    pub async fn is_allowed(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: &str,
    ) -> Result<(bool, Option<String>)> {
        let info = self.get(redis, circuit_id).await?;

        match info {
            Some(info) => match info.status {
                CircuitStatus::Banned => Ok((false, Some("Circuit is banned".to_string()))),
                CircuitStatus::SoftLocked => {
                    Ok((false, Some("Too many failed attempts. Try again later.".to_string())))
                }
                _ => Ok((true, None)),
            },
            None => Ok((true, None)), // New circuits are allowed
        }
    }

    /// Get rate limit status for a circuit
    pub async fn check_rate_limit(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: &str,
        max_requests_per_minute: u32,
    ) -> Result<(bool, u32)> {
        let key = format!("ratelimit:{}", circuit_id);

        // Increment counter
        let count: u32 = redis.incr(&key, 1).await?;

        // Set expiry on first request
        if count == 1 {
            redis.expire::<_, ()>(&key, 60).await?;
        }

        let allowed = count <= max_requests_per_minute;
        let remaining = if allowed {
            max_requests_per_minute - count
        } else {
            0
        };

        Ok((allowed, remaining))
    }
}
