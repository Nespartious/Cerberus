//! Passport validation endpoint (called by Nginx/HAProxy).

use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct ValidateQuery {
    /// Passport token to validate
    pub token: String,
    /// Circuit ID making the request
    pub circuit_id: Option<String>,
}

/// Validate a passport token
///
/// Returns:
/// - 200: Valid passport
/// - 401: Invalid or expired passport
/// - 403: Circuit is banned
/// - 429: Rate limited
///
/// This endpoint is designed to be called by Nginx auth_request
/// or HAProxy's http-request lua action.
pub async fn validate_passport(
    State(state): State<AppState>,
    Query(params): Query<ValidateQuery>,
) -> StatusCode {
    let mut redis = state.redis.clone();

    // Check if circuit is allowed (if provided)
    if let Some(ref circuit_id) = params.circuit_id {
        match state.circuit_tracker.is_allowed(&mut redis, circuit_id).await {
            Ok((false, _)) => return StatusCode::FORBIDDEN,
            Err(e) => {
                tracing::error!(error = %e, "Failed to check circuit status");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
            _ => {}
        }

        // Check rate limit
        match state
            .circuit_tracker
            .check_rate_limit(
                &mut redis,
                circuit_id,
                state.config.rate_limit.max_requests_per_minute,
            )
            .await
        {
            Ok((false, _)) => return StatusCode::TOO_MANY_REQUESTS,
            Err(e) => {
                tracing::error!(error = %e, "Failed to check rate limit");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
            _ => {}
        }
    }

    // Validate the passport token
    match state
        .captcha_verifier
        .validate_passport(&mut redis, &params.token)
        .await
    {
        Ok(true) => {
            tracing::debug!(token = %params.token, "Passport validated");
            StatusCode::OK
        }
        Ok(false) => {
            tracing::debug!(token = %params.token, "Invalid passport");
            StatusCode::UNAUTHORIZED
        }
        Err(e) => {
            tracing::error!(error = %e, "Passport validation error");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
