//! Unit tests for answer routes (upvote/downvote).
//!
//! Uses in-memory mock repositories — no external dependencies.
//! A fake auth layer injects AuthenticatedUser so handlers can extract it
//! without needing real JWT tokens.

use axum::Router;
use axum::routing::post;
use tokenoverflow::api::routes::answers::{downvote, upvote};
use tokenoverflow::api::routes::questions::create_question;

mod common {
    include!("../../../common/mod.rs");
}

use common::{fake_auth_layer, post_empty, post_json, read_json};

#[tokio::test]
async fn upvote_returns_200_for_existing_answer() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // First create a question (which creates an initial answer)
    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    // Upvote the answer
    let upvote_resp = post_empty(app, &format!("/v1/answers/{}/upvote", answer_id)).await;

    assert_eq!(upvote_resp.status().as_u16(), 200);

    let upvote_json = read_json(upvote_resp).await;
    assert_eq!(upvote_json["status"], "upvoted");
}

#[tokio::test]
async fn upvote_is_idempotent() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Create a question
    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    // Upvote twice
    let upvote_resp1 = post_empty(app.clone(), &format!("/v1/answers/{}/upvote", answer_id)).await;
    assert_eq!(upvote_resp1.status().as_u16(), 200);

    let upvote_resp2 = post_empty(app, &format!("/v1/answers/{}/upvote", answer_id)).await;
    assert_eq!(upvote_resp2.status().as_u16(), 200);
}

#[tokio::test]
async fn upvote_returns_404_for_nonexistent_answer() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_empty(
        app,
        "/v1/answers/00000000-0000-0000-0000-000000000099/upvote",
    )
    .await;

    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn upvote_returns_422_for_invalid_id() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_empty(app, "/v1/answers/not-a-uuid/upvote").await;

    assert_eq!(resp.status().as_u16(), 422); // Invalid path parameter
}

#[tokio::test]
async fn downvote_returns_200_for_existing_answer() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Create a question
    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    // Downvote the answer
    let downvote_resp = post_empty(app, &format!("/v1/answers/{}/downvote", answer_id)).await;

    assert_eq!(downvote_resp.status().as_u16(), 200);

    let downvote_json = read_json(downvote_resp).await;
    assert_eq!(downvote_json["status"], "downvoted");
}

#[tokio::test]
async fn downvote_is_idempotent() {
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store(&store);
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    // Create a question
    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    // Downvote twice
    let downvote_resp1 =
        post_empty(app.clone(), &format!("/v1/answers/{}/downvote", answer_id)).await;
    assert_eq!(downvote_resp1.status().as_u16(), 200);

    let downvote_resp2 = post_empty(app, &format!("/v1/answers/{}/downvote", answer_id)).await;
    assert_eq!(downvote_resp2.status().as_u16(), 200);
}

#[tokio::test]
async fn downvote_returns_404_for_nonexistent_answer() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_empty(
        app,
        "/v1/answers/00000000-0000-0000-0000-000000000099/downvote",
    )
    .await;

    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn downvote_returns_422_for_invalid_id() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_empty(app, "/v1/answers/not-a-uuid/downvote").await;

    assert_eq!(resp.status().as_u16(), 422); // Invalid path parameter
}
