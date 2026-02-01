//! HTTP route handlers for Fortify.

use axum::{
    Router,
    routing::{get, post},
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

mod captcha;
mod health;
mod passport;

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health & Status
        .route("/health", get(health::health_check))
        .route("/ready", get(health::ready_check))
        .route("/metrics", get(health::metrics))

        // CAPTCHA endpoints
        .route("/challenge", get(captcha::get_challenge))
        .route("/verify", post(captcha::verify_challenge))

        // Passport validation (for HAProxy/Nginx)
        .route("/validate", get(passport::validate_passport))

        // Admin endpoints (protected by randomized path in production)
        .nest("/admin", admin_routes())

        // Add shared state
        .with_state(state)
}

/// Admin routes (threat dial, circuit management, etc.)
fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/threat-level", get(get_threat_level).post(set_threat_level))
        .route("/circuits/:circuit_id", get(get_circuit_info).delete(ban_circuit))
        .route("/stats", get(get_stats))
}

// === Admin Handlers ===

#[derive(Serialize)]
struct ThreatLevelResponse {
    level: u8,
    requires_captcha: bool,
    captcha_count: u8,
}

async fn get_threat_level(
    State(state): State<AppState>,
) -> Json<ThreatLevelResponse> {
    let level = state.get_threat_level().await;
    Json(ThreatLevelResponse {
        level: level.value(),
        requires_captcha: level.requires_captcha(),
        captcha_count: level.captcha_count(),
    })
}

#[derive(Deserialize)]
struct SetThreatLevel {
    level: u8,
}

async fn set_threat_level(
    State(state): State<AppState>,
    Json(payload): Json<SetThreatLevel>,
) -> Result<Json<ThreatLevelResponse>, StatusCode> {
    let level = cerberus_common::ThreatLevel::new(payload.level);

    state
        .set_threat_level(level)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ThreatLevelResponse {
        level: level.value(),
        requires_captcha: level.requires_captcha(),
        captcha_count: level.captcha_count(),
    }))
}

async fn get_circuit_info(
    State(_state): State<AppState>,
    axum::extract::Path(circuit_id): axum::extract::Path<String>,
) -> Result<Json<cerberus_common::CircuitInfo>, StatusCode> {
    // TODO: Implement Redis lookup
    tracing::debug!(circuit_id = %circuit_id, "Looking up circuit");
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn ban_circuit(
    State(_state): State<AppState>,
    axum::extract::Path(circuit_id): axum::extract::Path<String>,
) -> StatusCode {
    // TODO: Implement circuit banning
    tracing::info!(circuit_id = %circuit_id, "Banning circuit");
    StatusCode::NOT_IMPLEMENTED
}

#[derive(Serialize)]
struct StatsResponse {
    node_id: String,
    threat_level: u8,
    uptime_secs: u64,
    // TODO: Add more stats
}

async fn get_stats(
    State(state): State<AppState>,
) -> Json<StatsResponse> {
    let level = state.get_threat_level().await;
    Json(StatsResponse {
        node_id: state.node_id.clone(),
        threat_level: level.value(),
        uptime_secs: 0, // TODO: Track uptime
    })
}
