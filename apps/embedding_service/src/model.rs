// Embedding model wrapper using fastembed-rs.
//
// Concrete adapter around fastembed. Error paths depend on fastembed internals
// and cannot be triggered in unit tests. The Embedder trait (in embedder.rs)
// provides the testable boundary. Happy-path tests live in test_model.rs.

// Re-export so existing `model::Embedder`, `model::EmbedError`, etc. still work.
pub use crate::embedder::{EmbedError, Embedder, OUTPUT_DIMENSION};

pub struct EmbeddingModel {
    model: fastembed::TextEmbedding,
}

impl EmbeddingModel {
    pub fn new() -> Result<Self, fastembed::Error> {
        // Use BGE-small-en-v1.5 model (384-dim, ~32MB)
        // Good quality embeddings with fast inference
        let model = fastembed::TextEmbedding::try_new(
            fastembed::InitOptions::new(fastembed::EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(true),
        )?;

        Ok(Self { model })
    }
}

impl Embedder for EmbeddingModel {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let embeddings = self
            .model
            .embed(texts.to_vec(), None)
            .map_err(|e| EmbedError(e.to_string()))?;

        // Truncate each embedding from 384 to 256 dims to match voyage-code-3
        let truncated = embeddings
            .into_iter()
            .map(|emb| emb.into_iter().take(OUTPUT_DIMENSION).collect())
            .collect();

        Ok(truncated)
    }
}
