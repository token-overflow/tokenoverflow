use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::external::embedding::client::VoyageClient;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Empty response from embedding service")]
    EmptyResponse,

    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Service interface for embedding generation
///
/// Implementations can use real APIs (Voyage AI) or mocks for testing.
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Generate embedding vector for the given text.
    ///
    /// Returns a 256-dimensional vector for voyage-code-3 model.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}

#[derive(Serialize)]
struct VoyageRequest<'a> {
    input: &'a str,
    model: &'a str,
    output_dimension: u32,
    output_dtype: &'a str,
}

#[derive(Deserialize)]
struct VoyageResponse {
    data: Vec<VoyageEmbeddingData>,
}

#[derive(Deserialize)]
struct VoyageEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct VoyageErrorResponse {
    detail: String,
}

// Needs a live Voyage AI-compatible API; tests use MockEmbeddingService instead.
// E2E: tests/e2e/external/test_embedding.rs exercises the real Voyage client.
#[async_trait]
#[cfg_attr(coverage_nightly, coverage(off))]
impl EmbeddingService for VoyageClient {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let url = format!("{}/embeddings", self.base_url);

        let body = VoyageRequest {
            input: text,
            model: &self.model,
            output_dimension: self.output_dimension,
            output_dtype: "float",
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| EmbeddingError::Network(e.to_string()))?;

        let status = response.status();

        if status.is_success() {
            let result: VoyageResponse = response
                .json()
                .await
                .map_err(|e| EmbeddingError::Api(e.to_string()))?;

            return result
                .data
                .into_iter()
                .next()
                .map(|e| e.embedding)
                .ok_or(EmbeddingError::EmptyResponse);
        }

        let error: VoyageErrorResponse = response.json().await.unwrap_or(VoyageErrorResponse {
            detail: "Unknown API error".to_string(),
        });
        Err(EmbeddingError::Api(error.detail))
    }
}
