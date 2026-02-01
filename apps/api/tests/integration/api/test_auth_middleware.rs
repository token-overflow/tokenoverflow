//! Integration tests for auth middleware.
//!
//! Tests that require pool.get() (valid token + resolve_user) live here.

use axum::Router;
use axum::body::Body;
use axum::routing::get;
use http::{Request, StatusCode};
use tower::ServiceExt;

use tokenoverflow::api::middleware;

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../common/mod.rs");
}

use common::test_jwt::generate_test_jwt;

#[tokio::test]
async fn protected_route_succeeds_with_valid_token() {
    let db = IntegrationTestDb::new().await;
    let app_state =
        common::create_mock_app_state_with_users_and_pool(&["user_test_valid"], db.pool().clone());

    let app = Router::new()
        .route("/v1/test", get(|| async { "protected ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(app_state);

    let token = generate_test_jwt("user_test_valid", 3600);
    let req = Request::builder()
        .uri("/v1/test")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body, "protected ok");
}

#[tokio::test]
async fn mcp_passes_with_valid_token() {
    let db = IntegrationTestDb::new().await;
    let app_state =
        common::create_mock_app_state_with_users_and_pool(&["user_test_valid"], db.pool().clone());

    let app = Router::new()
        .route("/mcp", get(|| async { "mcp ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(app_state);

    let token = generate_test_jwt("user_test_valid", 3600);
    let req = Request::builder()
        .uri("/mcp")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body, "mcp ok");
}

#[tokio::test]
async fn missing_bearer_token_returns_401() {
    let db = IntegrationTestDb::new().await;
    let app_state =
        common::create_mock_app_state_with_users_and_pool(&["user_test_valid"], db.pool().clone());

    let app = Router::new()
        .route("/v1/test", get(|| async { "protected ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(app_state);

    let req = Request::builder()
        .uri("/v1/test")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_bearer_token_returns_401() {
    let db = IntegrationTestDb::new().await;
    let app_state =
        common::create_mock_app_state_with_users_and_pool(&["user_test_valid"], db.pool().clone());

    let app = Router::new()
        .route("/v1/test", get(|| async { "protected ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(app_state);

    let req = Request::builder()
        .uri("/v1/test")
        .header("Authorization", "Bearer not-a-jwt")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn expired_jwt_returns_401() {
    let db = IntegrationTestDb::new().await;
    let app_state =
        common::create_mock_app_state_with_users_and_pool(&["user_test_valid"], db.pool().clone());

    let app = Router::new()
        .route("/v1/test", get(|| async { "protected ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(app_state);

    let token = common::test_jwt::generate_expired_test_jwt("user_test_valid");
    let req = Request::builder()
        .uri("/v1/test")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn mcp_missing_token_returns_401_with_www_authenticate() {
    let db = IntegrationTestDb::new().await;
    let app_state =
        common::create_mock_app_state_with_users_and_pool(&["user_test_valid"], db.pool().clone());

    let app = Router::new()
        .route("/mcp", get(|| async { "mcp ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(app_state);

    let req = Request::builder().uri("/mcp").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        resp.headers().contains_key(http::header::WWW_AUTHENTICATE),
        "MCP 401 response must include WWW-Authenticate header"
    );
}
