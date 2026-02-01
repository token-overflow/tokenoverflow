use embedding_service::model::Embedder;
use embedding_service::model::EmbeddingModel;
use std::sync::{Arc, LazyLock};

// Shared model across all tests to avoid lock contention during initialization
pub static TEST_MODEL: LazyLock<Arc<dyn Embedder>> =
    LazyLock::new(|| Arc::new(EmbeddingModel::new().expect("Failed to create test model")));
