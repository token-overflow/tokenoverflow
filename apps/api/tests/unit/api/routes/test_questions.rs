//! Unit tests for question routes.
//!
//! Uses in-memory mock repositories — no external dependencies.
//! A fake auth layer injects AuthenticatedUser so handlers can extract it
//! without needing real JWT tokens.

use axum::Router;
use axum::routing::{get, post};
use tokenoverflow::api::routes::questions::{add_answer, create_question, get_question};

mod common {
    include!("../../../common/mod.rs");
}

use common::{fake_auth_layer, get_request, post_json, read_json};

#[tokio::test]
async fn create_question_returns_201_with_valid_request() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_json(
        app,
        "/v1/questions",
        common::QuestionRequestBuilder::new().build(),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 201);

    // Verify response contains question_id and answer_id as UUID strings
    let json = read_json(resp).await;
    assert!(json["question_id"].is_string());
    assert!(json["answer_id"].is_string());
}

#[tokio::test]
async fn create_question_validates_title_length() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Title too short (less than 10 chars)
    let body = common::QuestionRequestBuilder::new().title("Short").build();
    let resp = post_json(app, "/v1/questions", body).await;

    assert_eq!(resp.status().as_u16(), 422); // Validation error
}

#[tokio::test]
async fn create_question_validates_body_length() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Body too short
    let body = common::QuestionRequestBuilder::new().body("Short").build();
    let resp = post_json(app, "/v1/questions", body).await;

    assert_eq!(resp.status().as_u16(), 422); // Validation error
}

#[tokio::test]
async fn get_question_returns_question_with_answers() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // First create a question
    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let question_id = create_json["question_id"].as_str().unwrap();

    // Now get the question
    let get_resp = get_request(app, &format!("/v1/questions/{}", question_id)).await;

    assert_eq!(get_resp.status().as_u16(), 200);

    let get_json = read_json(get_resp).await;
    assert_eq!(get_json["id"].as_str().unwrap(), question_id);
    assert!(get_json["answers"].is_array());
    assert_eq!(get_json["answers"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn get_question_returns_404_for_nonexistent_id() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Valid UUID format but does not exist
    let resp = get_request(app, "/v1/questions/00000000-0000-0000-0000-000000000099").await;

    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn get_question_returns_422_for_invalid_id() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = get_request(app, "/v1/questions/not-a-uuid").await;

    assert_eq!(resp.status().as_u16(), 422); // Invalid path parameter
}

#[tokio::test]
async fn get_question_returns_422_for_numeric_id() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Old-style numeric ID should be rejected
    let resp = get_request(app, "/v1/questions/42").await;

    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn add_answer_returns_201_for_existing_question() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/questions/{id}/answers", post(add_answer))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // First create a question
    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let question_id = create_json["question_id"].as_str().unwrap();

    // Add an answer
    let answer_body = common::AnswerRequestBuilder::new().build();
    let answer_resp = post_json(
        app,
        &format!("/v1/questions/{}/answers", question_id),
        answer_body,
    )
    .await;

    assert_eq!(answer_resp.status().as_u16(), 201);

    let answer_json = read_json(answer_resp).await;
    assert!(answer_json["id"].is_string());
}

#[tokio::test]
async fn add_answer_returns_404_for_nonexistent_question() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions/{id}/answers", post(add_answer))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let answer_body = common::AnswerRequestBuilder::new().build();
    let resp = post_json(
        app,
        "/v1/questions/00000000-0000-0000-0000-000000000099/answers",
        answer_body,
    )
    .await;

    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn add_answer_validates_body_length() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/questions/{id}/answers", post(add_answer))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // First create a question
    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let question_id = create_json["question_id"].as_str().unwrap();

    // Try to add an answer with body too short
    let answer_body = common::AnswerRequestBuilder::new().body("Short").build();
    let answer_resp = post_json(
        app,
        &format!("/v1/questions/{}/answers", question_id),
        answer_body,
    )
    .await;

    assert_eq!(answer_resp.status().as_u16(), 422); // Validation error
}

#[tokio::test]
async fn add_answer_returns_422_for_invalid_id() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions/{id}/answers", post(add_answer))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let answer_body = common::AnswerRequestBuilder::new().build();
    let resp = post_json(app, "/v1/questions/not-a-uuid/answers", answer_body).await;

    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn create_question_returns_503_when_embedding_fails() {
    let app_state = common::create_failing_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_json(
        app,
        "/v1/questions",
        common::QuestionRequestBuilder::new().build(),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 503);
}
