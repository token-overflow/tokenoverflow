// Embedder trait and error types for generating text embeddings.

use std::fmt;

/// Output dimension matching Voyage AI's voyage-code-3 model.
pub const OUTPUT_DIMENSION: usize = 256;

/// Error returned by embedding operations.
#[derive(Debug)]
pub struct EmbedError(pub String);

impl fmt::Display for EmbedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Trait for generating text embeddings.
pub trait Embedder: Send + Sync {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
}
