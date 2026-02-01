// Embeddings endpoint - Voyage AI-compatible API.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::api::state::AppState;
use crate::types::{EmbeddingData, EmbeddingInput, EmbeddingRequest, EmbeddingResponse, Usage};

/// Generate embeddings for input text(s)
///
/// Voyage AI-compatible endpoint that accepts single or multiple text inputs
/// and returns 256-dimensional embedding vectors.
pub async fn create_embeddings(
    State(state): State<AppState>,
    Json(body): Json<EmbeddingRequest>,
) -> impl IntoResponse {
    let texts: Vec<String> = match &body.input {
        EmbeddingInput::Single(s) => vec![s.clone()],
        EmbeddingInput::Multiple(v) => v.clone(),
    };

    let embeddings = match state.model.embed(&texts) {
        Ok(emb) => emb,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
                .into_response();
        }
    };

    let data: Vec<EmbeddingData> = embeddings
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| EmbeddingData {
            object: "embedding",
            embedding,
            index,
        })
        .collect();

    // Estimate tokens (rough approximation: ~4 chars per token)
    let total_chars: usize = texts.iter().map(|t| t.len()).sum();
    let tokens = (total_chars / 4).max(1) as u32;

    let response = EmbeddingResponse {
        object: "list",
        data,
        model: "voyage-code-3".to_string(),
        usage: Usage {
            total_tokens: tokens,
        },
    };

    Json(response).into_response()
}
