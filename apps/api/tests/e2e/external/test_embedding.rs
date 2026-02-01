// Integration tests for the embedding service.
// These tests verify that the embedding_service produces real semantic embeddings.
//
// These tests require the embedding_service to be running:
//   docker compose up -d embedding_service

use tokenoverflow::config::Config;
use tokenoverflow::external::embedding::{EmbeddingService, VoyageClient};

/// Calculate cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (mag_a * mag_b)
}

#[tokio::test]
async fn test_embedding_returns_correct_dimension() {
    let config = Config::load().expect("Failed to load config");
    let client = VoyageClient::new(
        config.embedding.base_url.as_deref(),
        &config.embedding.model,
        config.embedding.output_dimension,
        config.embedding.api_key().unwrap_or(""),
    )
    .expect("Failed to create Voyage client");

    let embedding = client.embed("test query").await.expect("Failed to embed");

    assert_eq!(embedding.len(), 256, "Embedding dimension should be 256");
}

#[tokio::test]
async fn test_embeddings_are_deterministic() {
    let config = Config::load().expect("Failed to load config");
    let client = VoyageClient::new(
        config.embedding.base_url.as_deref(),
        &config.embedding.model,
        config.embedding.output_dimension,
        config.embedding.api_key().unwrap_or(""),
    )
    .expect("Failed to create Voyage client");

    let text = "deterministic test query";
    let embedding1 = client.embed(text).await.expect("Failed to embed");
    let embedding2 = client.embed(text).await.expect("Failed to embed");

    assert_eq!(
        embedding1, embedding2,
        "Same input should produce identical embeddings"
    );
}

#[tokio::test]
async fn test_similar_texts_have_similar_embeddings() {
    let config = Config::load().expect("Failed to load config");
    let client = VoyageClient::new(
        config.embedding.base_url.as_deref(),
        &config.embedding.model,
        config.embedding.output_dimension,
        config.embedding.api_key().unwrap_or(""),
    )
    .expect("Failed to create Voyage client");

    // Get embeddings for semantically related and unrelated texts
    let error_vec = client
        .embed("null pointer exception in java")
        .await
        .expect("Failed to embed error text");

    let exception_vec = client
        .embed("runtime exception thrown in code")
        .await
        .expect("Failed to embed exception text");

    let weather_vec = client
        .embed("sunny day weather forecast")
        .await
        .expect("Failed to embed weather text");

    // Calculate cosine similarities
    let sim_error_exception = cosine_similarity(&error_vec, &exception_vec);
    let sim_error_weather = cosine_similarity(&error_vec, &weather_vec);

    // Error/exception texts should be more similar than error/weather
    assert!(
        sim_error_exception > sim_error_weather,
        "Expected error/exception similarity ({:.4}) > error/weather similarity ({:.4})",
        sim_error_exception,
        sim_error_weather
    );
}

#[tokio::test]
async fn test_different_texts_produce_different_embeddings() {
    let config = Config::load().expect("Failed to load config");
    let client = VoyageClient::new(
        config.embedding.base_url.as_deref(),
        &config.embedding.model,
        config.embedding.output_dimension,
        config.embedding.api_key().unwrap_or(""),
    )
    .expect("Failed to create Voyage client");

    let rust_vec = client
        .embed("How to handle errors in Rust?")
        .await
        .expect("Failed to embed");

    let python_vec = client
        .embed("How to handle errors in Python?")
        .await
        .expect("Failed to embed");

    // Same structure, different language - should be similar but not identical
    let similarity = cosine_similarity(&rust_vec, &python_vec);

    assert!(
        similarity < 0.99,
        "Different texts should produce different embeddings, got similarity: {}",
        similarity
    );
    assert!(
        similarity > 0.5,
        "Similar concepts should have reasonable similarity, got: {}",
        similarity
    );
}
