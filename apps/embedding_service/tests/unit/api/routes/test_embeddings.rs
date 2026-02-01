use axum::Router;
use axum::body::Body;
use axum::http::StatusCode;
use axum::routing::post;
use embedding_service::api::routes::embeddings::create_embeddings;
use embedding_service::api::state::AppState;
use embedding_service::model::{EmbedError, Embedder, OUTPUT_DIMENSION};
use http::Request;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

use super::TEST_MODEL;

fn create_app() -> Router {
    let app_state = AppState::new(TEST_MODEL.clone());
    Router::new()
        .route("/v1/embeddings", post(create_embeddings))
        .with_state(app_state)
}

fn post_embeddings(body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

async fn response_json(app: Router, request: Request<Body>) -> Value {
    let resp = app.oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn embeddings_single_input_returns_correct_dimension() {
    let app = create_app();

    let request = post_embeddings(&serde_json::json!({
        "input": "test query",
        "model": "voyage-code-3"
    }));

    let resp = app.oneshot(request).await.unwrap();
    assert!(resp.status().is_success());

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    // Verify Voyage AI-compatible response structure
    assert_eq!(body["object"], "list");
    assert!(body["data"].is_array());
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    // Verify embedding dimension
    let embedding = &body["data"][0]["embedding"];
    assert!(embedding.is_array());
    assert_eq!(embedding.as_array().unwrap().len(), OUTPUT_DIMENSION);

    // Verify model name
    assert_eq!(body["model"], "voyage-code-3");

    // Verify usage has only total_tokens
    assert!(body["usage"]["total_tokens"].as_u64().unwrap() > 0);
    assert!(body["usage"]["prompt_tokens"].is_null());
}

#[tokio::test]
async fn embeddings_multiple_inputs_returns_multiple_vectors() {
    let app = create_app();

    let request = post_embeddings(&serde_json::json!({
        "input": ["first text", "second text"],
        "model": "voyage-code-3"
    }));

    let resp = app.oneshot(request).await.unwrap();
    assert!(resp.status().is_success());

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    // Verify two embeddings returned
    assert_eq!(body["data"].as_array().unwrap().len(), 2);

    // Verify both have correct dimensions
    for (i, data) in body["data"].as_array().unwrap().iter().enumerate() {
        assert_eq!(data["index"], i);
        assert_eq!(
            data["embedding"].as_array().unwrap().len(),
            OUTPUT_DIMENSION
        );
    }
}

#[tokio::test]
async fn embeddings_are_deterministic() {
    // First request
    let body1 = response_json(
        create_app(),
        post_embeddings(&serde_json::json!({
            "input": "deterministic test",
            "model": "ignored"
        })),
    )
    .await;

    // Second request with same input
    let body2 = response_json(
        create_app(),
        post_embeddings(&serde_json::json!({
            "input": "deterministic test",
            "model": "ignored"
        })),
    )
    .await;

    // Embeddings should be identical for same input
    assert_eq!(body1["data"][0]["embedding"], body2["data"][0]["embedding"]);
}

#[tokio::test]
async fn similar_texts_have_similar_embeddings() {
    let app = create_app();

    // Get embeddings for three texts
    let body = response_json(
        app,
        post_embeddings(&serde_json::json!({
            "input": [
                "null pointer exception in java",
                "runtime exception thrown in code",
                "sunny day weather forecast"
            ],
            "model": "ignored"
        })),
    )
    .await;

    let error_emb: Vec<f32> = body["data"][0]["embedding"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();

    let exception_emb: Vec<f32> = body["data"][1]["embedding"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();

    let weather_emb: Vec<f32> = body["data"][2]["embedding"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();

    // Calculate cosine similarities
    let sim_error_exception = cosine_similarity(&error_emb, &exception_emb);
    let sim_error_weather = cosine_similarity(&error_emb, &weather_emb);

    // Error/exception texts should be more similar than error/weather
    assert!(
        sim_error_exception > sim_error_weather,
        "Expected error/exception similarity ({}) > error/weather similarity ({})",
        sim_error_exception,
        sim_error_weather
    );
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (mag_a * mag_b)
}

struct FailingEmbedder;

impl Embedder for FailingEmbedder {
    fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Err(EmbedError("model failure".to_string()))
    }
}

#[tokio::test]
async fn embeddings_returns_500_on_model_failure() {
    let app_state = AppState::new(Arc::new(FailingEmbedder));
    let app = Router::new()
        .route("/v1/embeddings", post(create_embeddings))
        .with_state(app_state);

    let request = post_embeddings(&serde_json::json!({
        "input": "test",
        "model": "test"
    }));

    let resp = app.oneshot(request).await.unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    // Voyage error format uses "detail"
    assert_eq!(json["detail"], "model failure");
}
