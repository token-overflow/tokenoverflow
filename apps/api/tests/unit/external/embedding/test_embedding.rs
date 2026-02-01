use tokenoverflow::external::embedding::{EmbeddingService, VoyageClient};

mod common {
    include!("../../../common/mod.rs");
}

use common::MockEmbedding;

#[test]
fn client_new_with_none_base_url_succeeds() {
    let result = VoyageClient::new(None, "voyage-code-3", 256, "voy-test");
    assert!(result.is_ok());
}

#[test]
fn client_new_with_some_base_url_succeeds() {
    let result = VoyageClient::new(
        Some("http://localhost:8000"),
        "voyage-code-3",
        256,
        "voy-test",
    );
    assert!(result.is_ok());
}

#[tokio::test]
async fn mock_generates_deterministic_vectors() {
    let mock = MockEmbedding::new();

    let text = "test input";
    let embedding1 = mock.embed(text).await.unwrap();
    let embedding2 = mock.embed(text).await.unwrap();

    assert_eq!(embedding1, embedding2);
}

#[tokio::test]
async fn mock_generates_correct_dimension() {
    let mock = MockEmbedding::new();

    let embedding = mock.embed("test").await.unwrap();

    assert_eq!(embedding.len(), 256);
}

#[tokio::test]
async fn mock_generates_normalized_vectors() {
    let mock = MockEmbedding::new();

    let embedding = mock.embed("test").await.unwrap();
    let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

    // Should be approximately 1.0 (unit vector)
    assert!((magnitude - 1.0).abs() < 0.0001);
}

#[tokio::test]
async fn mock_generates_different_vectors_for_different_text() {
    let mock = MockEmbedding::new();

    let embedding1 = mock.embed("hello").await.unwrap();
    let embedding2 = mock.embed("world").await.unwrap();

    assert_ne!(embedding1, embedding2);
}
