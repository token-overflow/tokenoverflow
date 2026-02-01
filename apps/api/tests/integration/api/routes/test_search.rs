//! Integration tests for search routes.
//!
//! Uses a real testcontainers pool with mock repositories.

use axum::Router;
use axum::routing::post;
use tokenoverflow::api::routes::questions::create_question;
use tokenoverflow::api::routes::search::search;

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

use common::{fake_auth_layer, post_json, read_json};

#[tokio::test]
async fn search_returns_empty_results_when_no_questions() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
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
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/search", post(search))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new()
        .title("How to handle async errors in Rust?")
        .body("I'm having trouble understanding how to properly handle errors in async Rust code. Can someone explain?")
        .build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    assert_eq!(create_resp.status().as_u16(), 201);

    let search_body = common::SearchRequestBuilder::new()
        .query("How do I handle errors in async Rust code?")
        .build();
    let search_resp = post_json(app, "/v1/search", search_body).await;
    assert_eq!(search_resp.status().as_u16(), 200);

    let search_json = read_json(search_resp).await;
    let questions = search_json["questions"].as_array().unwrap();
    assert!(!questions.is_empty());
}

#[tokio::test]
async fn search_validates_query_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/search", post(search))
        .with_state(app_state);

    let search_body = common::SearchRequestBuilder::new().query("Short").build();
    let resp = post_json(app, "/v1/search", search_body).await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn search_validates_limit_range() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/search", post(search))
        .with_state(app_state);

    let search_body = common::SearchRequestBuilder::new()
        .query("Valid search query here")
        .limit(100)
        .build();
    let resp = post_json(app, "/v1/search", search_body).await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn search_returns_503_when_embedding_fails() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_failing_mock_app_state_with_pool(db.pool().clone());
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

#[tokio::test]
async fn search_with_tags_returns_results() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::with_seed_tags();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/search", post(search))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Create a question with tags
    let create_body = common::QuestionRequestBuilder::new()
        .title("How to handle async errors in Rust?")
        .body("I'm having trouble understanding how to properly handle errors in async Rust code.")
        .tags(vec!["rust".to_string(), "python".to_string()])
        .build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    assert_eq!(create_resp.status().as_u16(), 201);

    // Search with matching tag filter
    let search_body = common::SearchRequestBuilder::new()
        .query("How do I handle errors in async Rust code?")
        .tags(vec!["rust".to_string()])
        .build();
    let search_resp = post_json(app, "/v1/search", search_body).await;
    assert_eq!(search_resp.status().as_u16(), 200);

    let search_json = read_json(search_resp).await;
    let questions = search_json["questions"].as_array().unwrap();
    assert!(
        !questions.is_empty(),
        "Search with matching tags should return results"
    );
}
