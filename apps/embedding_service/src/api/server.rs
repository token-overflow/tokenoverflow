// Server initialization and startup.

use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use crate::api::routes;
use crate::api::state::AppState;
use crate::model::{Embedder, EmbeddingModel};

// Tokio runtime bootstrap — needs a running server to exercise.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run() -> std::io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(std::io::Error::other)?
        .block_on(async_run())
}

// Server startup: env parsing, model loading, TCP bind.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn async_run() -> std::io::Result<()> {
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    info!("Loading embedding model...");
    let model: Arc<dyn Embedder> =
        Arc::new(EmbeddingModel::new().expect("Failed to load embedding model"));
    info!("Embedding model loaded successfully");

    let app_state = AppState::new(model);
    let bind_addr = format!("{}:{}", host, port);

    info!("Starting embedding service on {}", bind_addr);

    let app = routes::configure().with_state(app_state);

    let listener = TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await
}
