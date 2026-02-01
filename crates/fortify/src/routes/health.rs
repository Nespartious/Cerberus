//! Health check endpoints.

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

/// Basic health check (is the server running?)
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
pub struct ReadyResponse {
    status: &'static str,
    redis: bool,
}

/// Readiness check (are all dependencies healthy?)
pub async fn ready_check(
    State(state): State<AppState>,
) -> Result<Json<ReadyResponse>, StatusCode> {
    // Check Redis connectivity
    let redis_ok = check_redis(&state).await;

    if redis_ok {
        Ok(Json(ReadyResponse {
            status: "ready",
            redis: true,
        }))
    } else {
        // Return 503 if not ready
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

async fn check_redis(state: &AppState) -> bool {
    let mut conn = state.redis.clone();
    let result: Result<String, _> = redis::cmd("PING").query_async(&mut conn).await;
    result.is_ok()
}

#[derive(Serialize)]
pub struct MetricsResponse {
    node_id: String,
    threat_level: u8,
    // Prometheus-compatible metrics would go here
    // For now, just basic stats
}

/// Metrics endpoint (for monitoring)
pub async fn metrics(
    State(state): State<AppState>,
) -> Json<MetricsResponse> {
    let level = state.get_threat_level().await;
    
    Json(MetricsResponse {
        node_id: state.node_id.clone(),
        threat_level: level.value(),
    })
}
