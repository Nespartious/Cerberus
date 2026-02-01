//! CAPTCHA generation and verification endpoints.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use cerberus_common::CaptchaResult;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ChallengeQuery {
    /// Circuit ID (from X-Circuit-Id header or query param)
    pub circuit_id: Option<String>,
}

#[derive(Serialize)]
pub struct ChallengeResponse {
    pub challenge_id: String,
    pub image_data: String,
    pub grid_size: (u8, u8),
    pub instructions: String,
    pub expires_in_secs: u32,
}

/// Generate a new CAPTCHA challenge
pub async fn get_challenge(
    State(state): State<AppState>,
    Query(params): Query<ChallengeQuery>,
) -> Result<Json<ChallengeResponse>, (StatusCode, String)> {
    let mut redis = state.redis.clone();

    // Check if circuit is allowed
    if let Some(ref circuit_id) = params.circuit_id {
        let (allowed, reason) = state
            .circuit_tracker
            .is_allowed(&mut redis, circuit_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if !allowed {
            return Err((
                StatusCode::FORBIDDEN,
                reason.unwrap_or_else(|| "Access denied".to_string()),
            ));
        }
    }

    let threat_level = state.get_threat_level().await;
    let difficulty = threat_level.captcha_difficulty();

    let challenge = state
        .captcha_generator
        .generate(&mut redis, params.circuit_id, difficulty)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ChallengeResponse {
        challenge_id: challenge.challenge_id,
        image_data: challenge.image_data,
        grid_size: challenge.grid_size,
        instructions: challenge.instructions,
        expires_in_secs: difficulty.timeout_secs(),
    }))
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub challenge_id: String,
    /// User's answer (text input for MVP)
    pub answer: String,
    /// Circuit ID for tracking
    pub circuit_id: Option<String>,
}

/// Verify a CAPTCHA response
pub async fn verify_challenge(
    State(state): State<AppState>,
    Json(payload): Json<VerifyRequest>,
) -> Result<Json<CaptchaResult>, (StatusCode, String)> {
    let mut redis = state.redis.clone();

    // Check if circuit is allowed
    if let Some(ref circuit_id) = payload.circuit_id {
        let (allowed, reason) = state
            .circuit_tracker
            .is_allowed(&mut redis, circuit_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if !allowed {
            return Err((
                StatusCode::FORBIDDEN,
                reason.unwrap_or_else(|| "Access denied".to_string()),
            ));
        }
    }

    let result = state
        .captcha_verifier
        .verify(
            &mut redis,
            &payload.challenge_id,
            &payload.answer,
            payload.circuit_id.as_deref(),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Update circuit state
    if let Some(ref circuit_id) = payload.circuit_id {
        if result.success {
            if let Some(ref token) = result.passport_token {
                let expires = chrono::Utc::now().timestamp()
                    + state.config.captcha.passport_ttl_secs as i64;
                let _ = state
                    .circuit_tracker
                    .record_success(&mut redis, circuit_id, token, expires)
                    .await;
            }
        } else {
            let _ = state
                .circuit_tracker
                .record_failure(&mut redis, circuit_id)
                .await;
        }
    }

    Ok(Json(result))
}
