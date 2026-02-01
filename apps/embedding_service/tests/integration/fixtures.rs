use std::sync::{Arc, LazyLock};

use embedding_service::api::routes;
use embedding_service::api::state::AppState;
use embedding_service::model::{Embedder, EmbeddingModel};
use tokio::net::TcpListener;

pub static TEST_MODEL: LazyLock<Arc<dyn Embedder>> =
    LazyLock::new(|| Arc::new(EmbeddingModel::new().expect("Failed to create test model")));

pub struct TestServer {
    pub base_url: String,
}

impl TestServer {
    pub async fn start(model: Arc<dyn Embedder>) -> Self {
        let app_state = AppState::new(model);
        let app = routes::configure().with_state(app_state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self { base_url }
    }
}
