//! HTTP route handlers for Fortify.

use axum::{
    Form, Json, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

mod captcha;
mod health;
mod passport;

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Static pages (serve CAPTCHA gate with embedded challenge)
        .route("/", get(serve_captcha_page))
        .route("/captcha.html", get(serve_captcha_page))
        // Health & Status
        .route("/health", get(health::health_check))
        .route("/ready", get(health::ready_check))
        .route("/metrics", get(health::metrics))
        // CAPTCHA endpoints (JSON API for JS-enabled clients)
        .route("/challenge", get(captcha::get_challenge))
        // Verification - supports both JSON and form POST
        .route("/verify", post(verify_form))
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
        .route(
            "/threat-level",
            get(get_threat_level).post(set_threat_level),
        )
        .route(
            "/circuits/{circuit_id}",
            get(get_circuit_info).delete(ban_circuit),
        )
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

    match state
        .circuit_tracker
        .ban(&mut redis, &circuit_id, "Admin ban")
        .await
    {
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

async fn get_threat_level(State(state): State<AppState>) -> Json<ThreatLevelResponse> {
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

async fn get_stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let level = state.get_threat_level().await;
    Json(StatsResponse {
        node_id: state.node_id.clone(),
        threat_level: level.value(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

// === Static Page Serving ===

/// Form data for CAPTCHA verification (no-JS fallback)
#[derive(Deserialize)]
pub struct VerifyForm {
    pub challenge_id: String,
    pub answer: String,
}

/// Handle form POST verification (works without JavaScript)
async fn verify_form(
    State(state): State<AppState>,
    Form(form): Form<VerifyForm>,
) -> Response {
    let mut redis = state.redis.clone();

    let result = state
        .captcha_verifier
        .verify(&mut redis, &form.challenge_id, &form.answer, None)
        .await;

    match result {
        Ok(captcha_result) if captcha_result.success => {
            if let Some(token) = captcha_result.passport_token {
                // Redirect to protected app with passport token
                Redirect::to(&format!("/app/?passport_token={}", urlencoding::encode(&token)))
                    .into_response()
            } else {
                // Success but no token - show error
                serve_captcha_page_with_error(state, "Verification succeeded but no token generated").await
            }
        }
        Ok(_) => {
            // Wrong answer - show new challenge with error
            serve_captcha_page_with_error(state, "Incorrect code. Please try again.").await
        }
        Err(e) => {
            tracing::error!(error = %e, "CAPTCHA verification failed");
            serve_captcha_page_with_error(state, "Verification error. Please try again.").await
        }
    }
}

/// Serve the CAPTCHA page with an embedded challenge (no JavaScript required)
async fn serve_captcha_page(State(state): State<AppState>) -> Response {
    serve_captcha_page_inner(state, None).await
}

/// Serve CAPTCHA page with an error message
async fn serve_captcha_page_with_error(state: AppState, error: &str) -> Response {
    serve_captcha_page_inner(state, Some(error.to_string())).await
}

/// Inner function to render CAPTCHA page
async fn serve_captcha_page_inner(state: AppState, error: Option<String>) -> Response {
    let mut redis = state.redis.clone();
    let threat_level = state.get_threat_level().await;
    let difficulty = threat_level.captcha_difficulty();

    // Generate a fresh CAPTCHA challenge
    let challenge = match state
        .captcha_generator
        .generate(&mut redis, None, difficulty)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "Failed to generate CAPTCHA");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate challenge").into_response();
        }
    };

    // Decode the base64 SVG to embed directly
    let svg_html = if challenge.image_data.starts_with("data:image/svg+xml;base64,") {
        let b64 = challenge.image_data.strip_prefix("data:image/svg+xml;base64,").unwrap();
        match BASE64.decode(b64) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => format!(r#"<img src="{}" alt="CAPTCHA">"#, challenge.image_data),
        }
    } else {
        format!(r#"<img src="{}" alt="CAPTCHA">"#, challenge.image_data)
    };

    // Build error HTML if present
    let error_html = match error {
        Some(msg) => format!(
            r#"<div class="error" style="display:block">{}</div>"#,
            html_escape(&msg)
        ),
        None => String::new(),
    };

    // Render the complete HTML page with embedded CAPTCHA
    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sigil - Verification Required</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #e0e0e0;
        }}
        .container {{
            background: rgba(255, 255, 255, 0.05);
            border-radius: 16px;
            padding: 40px;
            max-width: 420px;
            width: 90%;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            border: 1px solid rgba(255, 255, 255, 0.1);
        }}
        .brand {{
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 24px;
        }}
        .brand-logo {{ font-size: 2rem; }}
        .brand-text h1 {{ font-size: 1.4rem; color: #fff; margin-bottom: 4px; }}
        .brand-text .subtitle {{ color: #888; font-size: 0.85rem; }}
        .captcha-box {{
            background: #0f0f1a;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 20px;
            text-align: center;
        }}
        .captcha-image {{
            border-radius: 4px;
            margin-bottom: 16px;
            background: #1a1a2e;
            min-height: 80px;
            display: flex;
            align-items: center;
            justify-content: center;
            overflow: hidden;
        }}
        .captcha-image svg {{ max-width: 100%; height: auto; }}
        .instructions {{ font-size: 0.85rem; color: #aaa; }}
        .answer-input {{
            width: 100%;
            padding: 14px 16px;
            background: #2a2a4a;
            border: 2px solid transparent;
            border-radius: 8px;
            color: #fff;
            font-size: 1.2rem;
            font-family: monospace;
            letter-spacing: 4px;
            text-align: center;
            text-transform: uppercase;
            margin-bottom: 16px;
        }}
        .answer-input:focus {{ outline: none; border-color: #4a9eff; background: #2a3a5a; }}
        .submit-btn {{
            width: 100%;
            padding: 14px;
            background: linear-gradient(135deg, #4a9eff 0%, #3a7edf 100%);
            border: none;
            border-radius: 8px;
            color: white;
            font-size: 1rem;
            font-weight: 600;
            cursor: pointer;
        }}
        .submit-btn:hover {{ box-shadow: 0 4px 12px rgba(74, 158, 255, 0.4); }}
        .refresh-link {{
            display: block;
            text-align: center;
            margin-top: 16px;
            color: #888;
            text-decoration: none;
            font-size: 0.85rem;
        }}
        .refresh-link:hover {{ color: #aaa; }}
        .footer {{
            margin-top: 24px;
            text-align: center;
            font-size: 0.75rem;
            color: #666;
        }}
        .error {{
            background: rgba(255, 77, 77, 0.1);
            border: 1px solid rgba(255, 77, 77, 0.3);
            color: #ff6b6b;
            padding: 12px;
            border-radius: 8px;
            margin-bottom: 16px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="brand">
            <span class="brand-logo">🔒</span>
            <div class="brand-text">
                <h1>Sigil</h1>
                <p class="subtitle">Human verification required</p>
            </div>
        </div>

        {error_html}

        <form method="POST" action="/verify">
            <input type="hidden" name="challenge_id" value="{challenge_id}">

            <div class="captcha-box">
                <div class="captcha-image">
                    {svg_html}
                </div>
                <p class="instructions">{instructions}</p>
            </div>

            <input type="text"
                   class="answer-input"
                   name="answer"
                   placeholder="Enter code"
                   autocomplete="off"
                   autocapitalize="off"
                   spellcheck="false"
                   maxlength="8"
                   autofocus
                   required>

            <button type="submit" class="submit-btn">Verify</button>

            <a href="/" class="refresh-link">↻ New Challenge</a>
        </form>

        <div class="footer">
            Protected by Cerberus • No JavaScript required
        </div>
    </div>
</body>
</html>"##,
        error_html = error_html,
        challenge_id = html_escape(&challenge.challenge_id),
        svg_html = svg_html,
        instructions = html_escape(&challenge.instructions),
    );

    Html(html).into_response()
}

/// Simple HTML escaping for safety
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
            match state
                .captcha_verifier
                .validate_passport(&mut redis, t)
                .await
            {
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
