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
    token: String,
    /// Circuit ID making the request
    circuit_id: Option<String>,
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
    // TODO: Lookup passport token in Redis
    // TODO: Verify token hasn't expired
    // TODO: Check circuit isn't banned
    // TODO: Update last-seen timestamp

    tracing::debug!(
        token = %params.token,
        circuit_id = ?params.circuit_id,
        "Validating passport"
    );

    // Placeholder - always fail for now
    StatusCode::UNAUTHORIZED
}
