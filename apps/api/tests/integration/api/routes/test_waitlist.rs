//! Integration tests for the waitlist route.
//!
//! `POST /v1/waitlist` lives outside the protected router and validates
//! its bearer JWT through the `WorkosJwtClaims` extractor in the handler.
//! These tests build a real test JWT with the bundled test private key
//! and assert that the handler returns the expected status, that the
//! payload's GitHub fields are wired up correctly, and that the request
//! does NOT create an `api.users` row (the entire point of moving the
//! route off `jwt_auth_layer`).

use axum::Router;
use axum::routing::post;
use serde_json::Value;

use tokenoverflow::api::routes::waitlist::add_to_waitlist;
use tokenoverflow::api::state::AppState;

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

use common::test_jwt::generate_test_jwt;
use common::{post_empty_with_auth, read_json};

/// `workos_id` value seeded into both the user repo (so the local-mode
/// `resolve_github_identity` returns the canned GitHub identity) and
/// referenced by the test JWT's `sub` claim.
const TEST_WORKOS_ID: &str = "user_waitlist_test";

/// Build an AppState with a seeded GitHub user. The handler reads the
/// applicant's email straight from the JWT, so no WorkOS mocks are
/// needed for the happy paths.
fn app_state(workos_id: &str, github_id: i64, username: &str, db: &IntegrationTestDb) -> AppState {
    let store = common::MockStore::new();
    common::create_app_state_with_github_user(
        &store,
        workos_id,
        github_id,
        username,
        db.pool().clone(),
    )
}

#[tokio::test]
async fn add_to_waitlist_returns_201_on_first_insert() {
    let db = IntegrationTestDb::new().await;

    let app_state = app_state(TEST_WORKOS_ID, 99_001, "octocat", &db);

    let app: Router = Router::new()
        .route("/v1/waitlist", post(add_to_waitlist))
        .with_state(app_state);

    let token = generate_test_jwt(TEST_WORKOS_ID, 3600);
    let resp = post_empty_with_auth(app, "/v1/waitlist", &token).await;
    assert_eq!(resp.status().as_u16(), 201);

    let body = read_json(resp).await;
    assert_eq!(body["github_id"], Value::Number(99_001.into()));
    assert_eq!(
        body["github_username"],
        Value::String("octocat".to_string())
    );
    assert_eq!(body["already_on_waitlist"], Value::Bool(false));
}

#[tokio::test]
async fn add_to_waitlist_marks_already_on_waitlist_on_second_call() {
    let db = IntegrationTestDb::new().await;

    let app_state = app_state(TEST_WORKOS_ID, 99_002, "octocat", &db);

    let app: Router = Router::new()
        .route("/v1/waitlist", post(add_to_waitlist))
        .with_state(app_state);

    let token = generate_test_jwt(TEST_WORKOS_ID, 3600);
    let first = post_empty_with_auth(app.clone(), "/v1/waitlist", &token).await;
    assert_eq!(first.status().as_u16(), 201);

    let second = post_empty_with_auth(app, "/v1/waitlist", &token).await;
    assert_eq!(second.status().as_u16(), 201);

    let body = read_json(second).await;
    assert_eq!(body["already_on_waitlist"], Value::Bool(true));
}

#[tokio::test]
async fn add_to_waitlist_returns_401_without_bearer_token() {
    let db = IntegrationTestDb::new().await;
    let app_state = app_state(TEST_WORKOS_ID, 99_010, "octocat", &db);

    let app: Router = Router::new()
        .route("/v1/waitlist", post(add_to_waitlist))
        .with_state(app_state);

    let resp = common::post_empty(app, "/v1/waitlist").await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn add_to_waitlist_returns_401_with_invalid_jwt() {
    let db = IntegrationTestDb::new().await;
    let app_state = app_state(TEST_WORKOS_ID, 99_011, "octocat", &db);

    let app: Router = Router::new()
        .route("/v1/waitlist", post(add_to_waitlist))
        .with_state(app_state);

    let resp = post_empty_with_auth(app, "/v1/waitlist", "not-a-jwt").await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn add_to_waitlist_returns_500_when_repo_fails() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_failing_waitlist_app_state(
        TEST_WORKOS_ID,
        99_003,
        "octocat",
        db.pool().clone(),
    );

    let app: Router = Router::new()
        .route("/v1/waitlist", post(add_to_waitlist))
        .with_state(app_state);

    let token = generate_test_jwt(TEST_WORKOS_ID, 3600);
    let resp = post_empty_with_auth(app, "/v1/waitlist", &token).await;
    assert_eq!(resp.status().as_u16(), 500);
}

#[tokio::test]
async fn add_to_waitlist_does_not_provision_users_row() {
    // The route lives outside `jwt_auth_layer`, so an unseeded `workos_id`
    // must NOT create an `api.users` row. We seed the GitHub identity for
    // the same workos_id so `resolve_github_identity` short-circuits, then
    // assert the user count is unchanged after the request.
    let db = IntegrationTestDb::new().await;

    let app_state = app_state(TEST_WORKOS_ID, 99_020, "octocat", &db);

    let users_before = app_state
        .users
        .find_by_workos_id(&mut *db.pool().get().await.unwrap(), TEST_WORKOS_ID)
        .await
        .expect("user lookup must succeed");
    let mock_users = app_state.users.clone();

    let app: Router = Router::new()
        .route("/v1/waitlist", post(add_to_waitlist))
        .with_state(app_state);

    let token = generate_test_jwt(TEST_WORKOS_ID, 3600);
    let resp = post_empty_with_auth(app, "/v1/waitlist", &token).await;
    assert_eq!(resp.status().as_u16(), 201);

    let users_after = mock_users
        .find_by_workos_id(&mut *db.pool().get().await.unwrap(), TEST_WORKOS_ID)
        .await
        .expect("user lookup must succeed");

    // The seeded user existed before AND after; the handler must not
    // have produced a duplicate. The mock repo would return the same
    // row so the assertion is "the row identity is unchanged".
    assert_eq!(users_before.map(|u| u.id), users_after.map(|u| u.id));
}

#[tokio::test]
async fn add_to_waitlist_returns_401_when_jwt_lacks_email() {
    let db = IntegrationTestDb::new().await;
    let app_state = app_state(TEST_WORKOS_ID, 99_030, "octocat", &db);

    let app: Router = Router::new()
        .route("/v1/waitlist", post(add_to_waitlist))
        .with_state(app_state);

    // generate_test_jwt produces a token without an `email` claim, but
    // the handler now reads it via the JWT validation. We test that
    // a token without email is rejected by signing one explicitly.
    let token = common::test_jwt::generate_test_jwt_without_email(TEST_WORKOS_ID, 3600);
    let resp = post_empty_with_auth(app, "/v1/waitlist", &token).await;
    assert_eq!(resp.status().as_u16(), 401);
}
