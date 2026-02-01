use axum::Router;
use axum::routing::{get, post};

use super::embeddings;
use super::health;
use crate::api::state::AppState;

/// Configure all routes for the application
pub fn configure() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/v1/embeddings", post(embeddings::create_embeddings))
}
