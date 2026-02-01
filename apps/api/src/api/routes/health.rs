use axum::Json;
use axum::extract::State;
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::api::state::AppState;

#[derive(Serialize, Deserialize, Debug)]
pub struct HealthResponse {
    pub status: String,
}

/// Health check endpoint
///
/// Returns 200 with `{"status":"ok"}` when the database is reachable.
/// Returns 503 with `{"status":"unhealthy"}` when the database connection
/// fails. Internal details (database status, version numbers) are never
/// exposed on this public endpoint.
pub async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    match check_db(&state).await {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok".to_string(),
            }),
        ),
        Err(e) => {
            tracing::warn!("Health check failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "unhealthy".to_string(),
                }),
            )
        }
    }
}

async fn check_db(state: &AppState) -> Result<(), String> {
    // Getting a connection from the pool validates DB connectivity
    // (bb8 runs connection health checks internally)
    let _conn = state
        .pool
        .get()
        .await
        .map_err(|e| format!("database connection failed: {}", e))?;
    Ok(())
}
