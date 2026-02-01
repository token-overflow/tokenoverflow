//! Unit tests for search routes.
//!
//! Uses in-memory mock repositories — no external dependencies.
//! A fake auth layer injects AuthenticatedUser for routes that need it
//! (e.g., create_question) without requiring real JWT tokens.

use axum::Router;
use axum::routing::post;
use tokenoverflow::api::routes::questions::create_question;
use tokenoverflow::api::routes::search::search;

mod common {
    include!("../../../common/mod.rs");
}

use common::{fake_auth_layer, post_json, read_json};

#[tokio::test]
async fn search_returns_empty_results_when_no_questions() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/search", post(search))
        .with_state(app_state);

    let response = post_json(
        app,
        "/v1/search",
        common::SearchRequestBuilder::new().build(),
    )
    .await;

    assert_eq!(response.status().as_u16(), 200);

    let json = read_json(response).await;
    assert!(json["questions"].is_array());
    assert_eq!(json["questions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn search_returns_matching_questions() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/search", post(search))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Create a question about Rust async
    let create_body = common::QuestionRequestBuilder::new()
        .title("How to handle async errors in Rust?")
        .body("I'm having trouble understanding how to properly handle errors in async Rust code. Can someone explain?")
        .build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    assert_eq!(create_resp.status().as_u16(), 201);

    // Search for async error handling
    let search_body = common::SearchRequestBuilder::new()
        .query("How do I handle errors in async Rust code?")
        .build();
    let search_resp = post_json(app, "/v1/search", search_body).await;

    assert_eq!(search_resp.status().as_u16(), 200);

    let search_json = read_json(search_resp).await;
    let questions = search_json["questions"].as_array().unwrap();
    assert!(!questions.is_empty());

    // Verify the search result structure
    let first_result = &questions[0];
    assert!(first_result["id"].is_string());
    assert!(first_result["title"].is_string());
    assert!(first_result["similarity"].is_number());
    assert!(first_result["answers"].is_array());
}

#[tokio::test]
async fn search_respects_limit_parameter() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/search", post(search))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Create multiple questions
    for i in 0..5 {
        let create_body = common::QuestionRequestBuilder::new()
            .title(format!("Question {} about Rust programming", i))
            .body(format!(
                "This is question number {} about various Rust topics.",
                i
            ))
            .build();
        let resp = post_json(app.clone(), "/v1/questions", create_body).await;
        assert_eq!(resp.status().as_u16(), 201);
    }

    // Search with limit of 2
    let search_body = common::SearchRequestBuilder::new()
        .query("Rust programming question")
        .limit(2)
        .build();
    let search_resp = post_json(app, "/v1/search", search_body).await;

    assert_eq!(search_resp.status().as_u16(), 200);

    let search_json = read_json(search_resp).await;
    let questions = search_json["questions"].as_array().unwrap();
    assert!(questions.len() <= 2);
}

#[tokio::test]
async fn search_validates_query_length() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/search", post(search))
        .with_state(app_state);

    // Query too short (less than 10 chars)
    let search_body = common::SearchRequestBuilder::new().query("Short").build();
    let resp = post_json(app, "/v1/search", search_body).await;

    assert_eq!(resp.status().as_u16(), 422); // Validation error
}

#[tokio::test]
async fn search_validates_limit_range() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/search", post(search))
        .with_state(app_state);

    // Limit too high (max is 10)
    let search_body = common::SearchRequestBuilder::new()
        .query("Valid search query here")
        .limit(100)
        .build();
    let resp = post_json(app, "/v1/search", search_body).await;

    assert_eq!(resp.status().as_u16(), 422); // Validation error
}

#[tokio::test]
async fn search_results_ordered_by_similarity() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/search", post(search))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Create questions with varying relevance
    let create_body1 = common::QuestionRequestBuilder::new()
        .title("How to use tokio for async Rust?")
        .body("I want to learn about tokio runtime for async programming in Rust.")
        .build();
    let resp1 = post_json(app.clone(), "/v1/questions", create_body1).await;
    assert_eq!(resp1.status().as_u16(), 201);

    let create_body2 = common::QuestionRequestBuilder::new()
        .title("Best practices for Python web development")
        .body("Looking for guidance on building web apps with Flask and Django.")
        .build();
    let resp2 = post_json(app.clone(), "/v1/questions", create_body2).await;
    assert_eq!(resp2.status().as_u16(), 201);

    // Search for tokio
    let search_body = common::SearchRequestBuilder::new()
        .query("How do I use tokio in Rust for async programming?")
        .build();
    let search_resp = post_json(app, "/v1/search", search_body).await;

    assert_eq!(search_resp.status().as_u16(), 200);

    let search_json = read_json(search_resp).await;
    let questions = search_json["questions"].as_array().unwrap();

    // Results should be ordered by similarity (descending)
    if questions.len() >= 2 {
        let sim1 = questions[0]["similarity"].as_f64().unwrap();
        let sim2 = questions[1]["similarity"].as_f64().unwrap();
        assert!(
            sim1 >= sim2,
            "Results should be ordered by similarity: {} >= {}",
            sim1,
            sim2
        );
    }
}

#[tokio::test]
async fn search_returns_503_when_embedding_fails() {
    let app_state = common::create_failing_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/search", post(search))
        .with_state(app_state);

    let resp = post_json(
        app,
        "/v1/search",
        common::SearchRequestBuilder::new().build(),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 503);
}
