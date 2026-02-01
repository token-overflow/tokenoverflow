// Application state shared across all request handlers.

use std::sync::Arc;

use crate::model::Embedder;

#[derive(Clone)]
pub struct AppState {
    pub model: Arc<dyn Embedder>,
}

impl AppState {
    pub fn new(model: Arc<dyn Embedder>) -> Self {
        Self { model }
    }
}
