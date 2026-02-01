//! CAPTCHA generation and verification endpoints.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use cerberus_common::{CaptchaChallenge, CaptchaResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ChallengeQuery {
    /// Circuit ID (from X-Circuit-Id header or query param)
    circuit_id: Option<String>,
}

#[derive(Serialize)]
pub struct ChallengeResponse {
    challenge_id: String,
    image_url: String,
    grid_size: (u8, u8),
    instructions: String,
    expires_in_secs: u32,
}

/// Generate a new CAPTCHA challenge
pub async fn get_challenge(
    State(state): State<AppState>,
    Query(params): Query<ChallengeQuery>,
) -> Result<Json<ChallengeResponse>, StatusCode> {
    let threat_level = state.get_threat_level().await;
    let difficulty = threat_level.captcha_difficulty();
    let grid_size = difficulty.grid_size();

    // Generate challenge ID
    let challenge_id = generate_challenge_id();

    // TODO: Generate actual CAPTCHA image
    // TODO: Store challenge in Redis with expected answer

    tracing::debug!(
        challenge_id = %challenge_id,
        circuit_id = ?params.circuit_id,
        difficulty = ?difficulty,
        "Generated CAPTCHA challenge"
    );

    Ok(Json(ChallengeResponse {
        challenge_id: challenge_id.clone(),
        image_url: format!("/captcha/image/{}", challenge_id),
        grid_size,
        instructions: get_instructions_for_difficulty(difficulty),
        expires_in_secs: difficulty.timeout_secs(),
    }))
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    challenge_id: String,
    /// Grid positions clicked by user (0-indexed)
    selected_positions: Vec<(u8, u8)>,
    /// Circuit ID for tracking
    circuit_id: Option<String>,
}

/// Verify a CAPTCHA response
pub async fn verify_challenge(
    State(state): State<AppState>,
    Json(payload): Json<VerifyRequest>,
) -> Result<Json<CaptchaResult>, StatusCode> {
    // TODO: Lookup challenge from Redis
    // TODO: Compare selected positions with expected
    // TODO: Update circuit state based on result
    // TODO: Generate passport if successful

    tracing::debug!(
        challenge_id = %payload.challenge_id,
        circuit_id = ?payload.circuit_id,
        selections = ?payload.selected_positions,
        "Verifying CAPTCHA"
    );

    // Placeholder response
    Ok(Json(CaptchaResult {
        success: false,
        remaining_challenges: 0,
        passport_token: None,
        error_message: Some("Not implemented yet".to_string()),
    }))
}

/// Generate a cryptographically random challenge ID
fn generate_challenge_id() -> String {
    use rand::Rng;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    let mut bytes = [0u8; 16];
    rand::rng().fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn get_instructions_for_difficulty(difficulty: cerberus_common::CaptchaDifficulty) -> String {
    match difficulty {
        cerberus_common::CaptchaDifficulty::Easy => {
            "Click all squares containing a cat".to_string()
        }
        cerberus_common::CaptchaDifficulty::Medium => {
            "Click all squares containing animals, in order from left to right".to_string()
        }
        cerberus_common::CaptchaDifficulty::Hard => {
            "Click the squares matching the pattern shown above".to_string()
        }
        cerberus_common::CaptchaDifficulty::Extreme => {
            "Solve the puzzle within 20 seconds".to_string()
        }
    }
}
