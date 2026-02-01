//! HTTP route handlers for Fortify.

use axum::{
    Router,
    routing::{get, post},
    extract::State,
    http::StatusCode,
    Json,
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

mod captcha;
mod health;
mod passport;

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Static pages (serve CAPTCHA gate)
        .route("/", get(serve_captcha_page))
        .route("/captcha.html", get(serve_captcha_page))

        // Health & Status
        .route("/health", get(health::health_check))
        .route("/ready", get(health::ready_check))
        .route("/metrics", get(health::metrics))

        // CAPTCHA endpoints
        .route("/challenge", get(captcha::get_challenge))
        .route("/verify", post(captcha::verify_challenge))

        // Passport validation (for HAProxy/Nginx)
        .route("/validate", get(passport::validate_passport))

        // Protected backend (mock for testing)
        .route("/app/", get(protected_app))
        .route("/app/{*path}", get(protected_app))

        // Circuit info (for debugging/admin)
        .route("/circuit/{circuit_id}", get(get_circuit_info))

        // Admin endpoints (protected by randomized path in production)
        .nest("/admin", admin_routes())

        // Add shared state
        .with_state(state)
}

/// Admin routes (threat dial, circuit management, etc.)
fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/threat-level", get(get_threat_level).post(set_threat_level))
        .route("/circuits/{circuit_id}", get(get_circuit_info).delete(ban_circuit))
        .route("/stats", get(get_stats))
}

// === Circuit Handlers ===

async fn get_circuit_info(
    State(state): State<AppState>,
    axum::extract::Path(circuit_id): axum::extract::Path<String>,
) -> Result<Json<cerberus_common::CircuitInfo>, StatusCode> {
    let mut redis = state.redis.clone();

    match state.circuit_tracker.get(&mut redis, &circuit_id).await {
        Ok(Some(info)) => Ok(Json(info)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!(error = %e, circuit_id = %circuit_id, "Failed to get circuit");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn ban_circuit(
    State(state): State<AppState>,
    axum::extract::Path(circuit_id): axum::extract::Path<String>,
) -> StatusCode {
    let mut redis = state.redis.clone();

    match state.circuit_tracker.ban(&mut redis, &circuit_id, "Admin ban").await {
        Ok(()) => {
            tracing::info!(circuit_id = %circuit_id, "Circuit banned by admin");
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!(error = %e, circuit_id = %circuit_id, "Failed to ban circuit");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
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

#[derive(Serialize)]
struct StatsResponse {
    node_id: String,
    threat_level: u8,
    version: &'static str,
}

async fn get_stats(
    State(state): State<AppState>,
) -> Json<StatsResponse> {
    let level = state.get_threat_level().await;
    Json(StatsResponse {
        node_id: state.node_id.clone(),
        threat_level: level.value(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

// === Static Page Serving ===

/// Serve the CAPTCHA page
async fn serve_captcha_page() -> Response {
    // Embedded HTML for zero-dependency serving
    const CAPTCHA_HTML: &str = include_str!("../../../../static/captcha.html");
    Html(CAPTCHA_HTML).into_response()
}

// === Protected Backend (Mock) ===

/// Protected app endpoint - requires valid passport token
async fn protected_app(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Check for passport token
    let token = params.get("passport_token");
    
    match token {
        Some(t) => {
            let mut redis = state.redis.clone();
            match state.captcha_verifier.validate_passport(&mut redis, t).await {
                Ok(true) => {
                    // Valid passport - show protected content
                    Html(format!(r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sigil - Welcome</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #e0e0e0;
            margin: 0;
        }}
        .container {{
            background: rgba(255, 255, 255, 0.05);
            border-radius: 16px;
            padding: 40px;
            max-width: 600px;
            text-align: center;
            border: 1px solid rgba(255, 255, 255, 0.1);
        }}
        h1 {{ color: #4a9eff; margin-bottom: 16px; }}
        .success {{ color: #6bff6b; font-size: 3rem; margin-bottom: 16px; }}
        .info {{ background: rgba(74, 158, 255, 0.1); padding: 16px; border-radius: 8px; margin: 20px 0; font-family: monospace; font-size: 0.9rem; word-break: break-all; }}
        .backend {{ color: #888; margin-top: 24px; font-size: 0.85rem; }}
        .backend a {{ color: #4a9eff; text-decoration: none; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="success">✓</div>
        <h1>Welcome to Sigil</h1>
        <p>You have successfully passed human verification.</p>
        <div class="info">
            <strong>Passport Token:</strong><br>
            {token_preview}...
        </div>
        <p>Your passport is valid for 10 minutes. You can now access the protected service.</p>
        <p class="backend">
            In production, this would proxy to:<br>
            <a href="#">sigilahzwq5u34gdh2bl3ymokyc7kobika55kyhztsucdoub73hz7qid.onion</a>
        </p>
    </div>
</body>
</html>"##, token_preview = &t[..t.len().min(20)])).into_response()
                }
                Ok(false) => {
                    // Invalid/expired passport - redirect to CAPTCHA
                    axum::response::Redirect::to("/").into_response()
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to validate passport");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        None => {
            // No passport - redirect to CAPTCHA
            axum::response::Redirect::to("/").into_response()
        }
    }
}
