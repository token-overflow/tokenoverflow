//! Integration tests for question routes.
//!
//! Uses a real testcontainers pool with mock repositories.

use axum::Router;
use axum::routing::{get, post};
use tokenoverflow::api::routes::questions::{add_answer, create_question, get_question};

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

use common::{fake_auth_layer, get_request, post_json, read_json};

#[tokio::test]
async fn create_question_returns_201_with_valid_request() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
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
    let json = read_json(resp).await;
    assert!(json["question_id"].is_string());
    assert!(json["answer_id"].is_string());
}

#[tokio::test]
async fn create_question_validates_title_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let body = common::QuestionRequestBuilder::new().title("Short").build();
    let resp = post_json(app, "/v1/questions", body).await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn create_question_validates_body_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let body = common::QuestionRequestBuilder::new().body("Short").build();
    let resp = post_json(app, "/v1/questions", body).await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn get_question_returns_question_with_answers() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let question_id = create_json["question_id"].as_str().unwrap();

    let get_resp = get_request(app, &format!("/v1/questions/{}", question_id)).await;
    assert_eq!(get_resp.status().as_u16(), 200);

    let get_json = read_json(get_resp).await;
    assert_eq!(get_json["id"].as_str().unwrap(), question_id);
    assert!(get_json["answers"].is_array());
    assert_eq!(get_json["answers"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn get_question_returns_404_for_nonexistent_id() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = get_request(app, "/v1/questions/00000000-0000-0000-0000-000000000099").await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn get_question_returns_422_for_invalid_id() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = get_request(app, "/v1/questions/not-a-uuid").await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn get_question_returns_422_for_numeric_id() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = get_request(app, "/v1/questions/42").await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn add_answer_returns_201_for_existing_question() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/questions/{id}/answers", post(add_answer))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let question_id = create_json["question_id"].as_str().unwrap();

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
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
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
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/questions/{id}/answers", post(add_answer))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let question_id = create_json["question_id"].as_str().unwrap();

    let answer_body = common::AnswerRequestBuilder::new().body("Short").build();
    let answer_resp = post_json(
        app,
        &format!("/v1/questions/{}/answers", question_id),
        answer_body,
    )
    .await;

    assert_eq!(answer_resp.status().as_u16(), 422);
}

#[tokio::test]
async fn add_answer_returns_422_for_invalid_id() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
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
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_failing_mock_app_state_with_pool(db.pool().clone());
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

#[tokio::test]
async fn create_question_validates_answer_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let body = common::QuestionRequestBuilder::new()
        .answer("Short")
        .build();
    let resp = post_json(app, "/v1/questions", body).await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn create_question_with_tags_returns_201() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::with_seed_tags();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/questions/{id}", get(get_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let body = common::QuestionRequestBuilder::new()
        .tags(vec!["rust".to_string(), "python".to_string()])
        .build();
    let resp = post_json(app.clone(), "/v1/questions", body).await;
    assert_eq!(resp.status().as_u16(), 201);

    let json = read_json(resp).await;
    let question_id = json["question_id"].as_str().unwrap();

    let get_resp = get_request(app, &format!("/v1/questions/{}", question_id)).await;
    assert_eq!(get_resp.status().as_u16(), 200);
    let get_json = read_json(get_resp).await;
    let tags = get_json["tags"].as_array().unwrap();
    assert!(tags.iter().any(|t| t.as_str() == Some("rust")));
    assert!(tags.iter().any(|t| t.as_str() == Some("python")));
}

#[tokio::test]
async fn create_question_validates_tag_count() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let body = common::QuestionRequestBuilder::new()
        .tags(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
            "f".to_string(),
        ])
        .build();
    let resp = post_json(app, "/v1/questions", body).await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn create_question_validates_tag_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let long_tag = "a".repeat(36);
    let body = common::QuestionRequestBuilder::new()
        .tags(vec![long_tag])
        .build();
    let resp = post_json(app, "/v1/questions", body).await;
    assert_eq!(resp.status().as_u16(), 422);
}
