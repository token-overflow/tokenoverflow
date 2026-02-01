//! Integration tests for answer routes (upvote/downvote).
//!
//! Uses a real testcontainers pool with mock repositories.

use axum::Router;
use axum::routing::post;
use tokenoverflow::api::routes::answers::{downvote, upvote};
use tokenoverflow::api::routes::questions::create_question;

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

use common::{fake_auth_layer, fake_voter_auth_layer, post_empty, post_json, read_json};

#[tokio::test]
async fn upvote_returns_200_for_existing_answer() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());

    let create_app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state.clone());

    let vote_app: Router = Router::new()
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_voter_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(create_app, "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    let upvote_resp = post_empty(vote_app, &format!("/v1/answers/{}/upvote", answer_id)).await;
    assert_eq!(upvote_resp.status().as_u16(), 200);

    let upvote_json = read_json(upvote_resp).await;
    assert_eq!(upvote_json["status"], "upvoted");
}

#[tokio::test]
async fn upvote_returns_404_for_nonexistent_answer() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_voter_auth_layer))
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
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_empty(app, "/v1/answers/not-a-uuid/upvote").await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn downvote_returns_200_for_existing_answer() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());

    let create_app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state.clone());

    let vote_app: Router = Router::new()
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_voter_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(create_app, "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    let downvote_resp = post_empty(vote_app, &format!("/v1/answers/{}/downvote", answer_id)).await;
    assert_eq!(downvote_resp.status().as_u16(), 200);

    let downvote_json = read_json(downvote_resp).await;
    assert_eq!(downvote_json["status"], "downvoted");
}

#[tokio::test]
async fn downvote_returns_404_for_nonexistent_answer() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_voter_auth_layer))
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
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let resp = post_empty(app, "/v1/answers/not-a-uuid/downvote").await;
    assert_eq!(resp.status().as_u16(), 422);
}

#[tokio::test]
async fn upvote_is_idempotent() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());

    let create_app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state.clone());

    let vote_app: Router = Router::new()
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_voter_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(create_app, "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    let upvote_uri = format!("/v1/answers/{}/upvote", answer_id);

    // First upvote
    let first_resp = post_empty(vote_app.clone(), &upvote_uri).await;
    assert_eq!(first_resp.status().as_u16(), 200);
    let first_json = read_json(first_resp).await;
    assert_eq!(first_json["status"], "upvoted");

    // Second upvote (idempotent)
    let second_resp = post_empty(vote_app, &upvote_uri).await;
    assert_eq!(second_resp.status().as_u16(), 200);
    let second_json = read_json(second_resp).await;
    assert_eq!(second_json["status"], "upvoted");

    // Verify the vote count stays at 1 via the mock store
    let votes = store.votes.lock().unwrap();
    let upvote_count = votes
        .iter()
        .filter(|v| v.answer_id.to_string() == answer_id && v.value == 1)
        .count();
    assert_eq!(
        upvote_count, 1,
        "Upvote should be idempotent (only 1 vote record)"
    );
}

#[tokio::test]
async fn downvote_is_idempotent() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());

    let create_app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state.clone());

    let vote_app: Router = Router::new()
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_voter_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(create_app, "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    let downvote_uri = format!("/v1/answers/{}/downvote", answer_id);

    // First downvote
    let first_resp = post_empty(vote_app.clone(), &downvote_uri).await;
    assert_eq!(first_resp.status().as_u16(), 200);
    let first_json = read_json(first_resp).await;
    assert_eq!(first_json["status"], "downvoted");

    // Second downvote (idempotent)
    let second_resp = post_empty(vote_app, &downvote_uri).await;
    assert_eq!(second_resp.status().as_u16(), 200);
    let second_json = read_json(second_resp).await;
    assert_eq!(second_json["status"], "downvoted");

    // Verify the vote count stays at 1 via the mock store
    let votes = store.votes.lock().unwrap();
    let downvote_count = votes
        .iter()
        .filter(|v| v.answer_id.to_string() == answer_id && v.value == -1)
        .count();
    assert_eq!(
        downvote_count, 1,
        "Downvote should be idempotent (only 1 vote record)"
    );
}

#[tokio::test]
async fn self_upvote_returns_403() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/answers/{id}/upvote", post(upvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    let resp = post_empty(app, &format!("/v1/answers/{}/upvote", answer_id)).await;
    assert_eq!(resp.status().as_u16(), 403);
}

#[tokio::test]
async fn self_downvote_returns_403() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let app: Router = Router::new()
        .route("/v1/questions", post(create_question))
        .route("/v1/answers/{id}/downvote", post(downvote))
        .layer(axum::middleware::from_fn(fake_auth_layer))
        .with_state(app_state);

    let create_body = common::QuestionRequestBuilder::new().build();
    let create_resp = post_json(app.clone(), "/v1/questions", create_body).await;
    let create_json = read_json(create_resp).await;
    let answer_id = create_json["answer_id"].as_str().unwrap();

    let resp = post_empty(app, &format!("/v1/answers/{}/downvote", answer_id)).await;
    assert_eq!(resp.status().as_u16(), 403);
}
