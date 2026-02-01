// Unit tests for auth middleware that do NOT require a real pool.
//
// Tests that fail before pool.get() (no token, invalid token, expired token)
// work with the dummy pool. Tests that succeed with valid token + resolve_user
// require a real pool and live in integration/api/test_auth_middleware.rs.

use axum::Router;
use axum::body::Body;
use axum::routing::get;
use http::{Request, StatusCode};
use tower::ServiceExt;

use tokenoverflow::api::middleware;
use tokenoverflow::api::routes::health::health_check;
use tokenoverflow::api::routes::well_known::oauth_protected_resource;

mod common {
    include!("../../common/mod.rs");
}

use common::test_jwt::generate_expired_test_jwt;

/// Build a router that mirrors the production route structure.
fn app_with_auth() -> Router {
    let app_state = common::create_mock_app_state_with_users(&["user_test_valid"]);

    let protected = Router::new()
        .route("/v1/test", get(|| async { "protected ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ));

    let mcp_protected = Router::new()
        .route("/mcp", get(|| async { "mcp ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::jwt_auth_layer,
        ));

    Router::new()
        .route("/health", get(health_check))
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth_protected_resource),
        )
        .merge(protected)
        .merge(mcp_protected)
        .with_state(app_state)
}

// -- Public routes should work without auth --

#[tokio::test]
async fn health_endpoint_accessible_without_auth() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Health endpoint should not require auth"
    );
}

#[tokio::test]
async fn well_known_endpoint_accessible_without_auth() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/.well-known/oauth-protected-resource")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// -- Protected routes require auth --

#[tokio::test]
async fn protected_route_returns_401_without_token() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/v1/test")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_returns_401_with_invalid_token() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/v1/test")
        .header("Authorization", "Bearer invalid-jwt-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_returns_401_with_expired_token() {
    let app = app_with_auth();
    let token = generate_expired_test_jwt("user_expired");
    let req = Request::builder()
        .uri("/v1/test")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// -- MCP route requires auth --

#[tokio::test]
async fn mcp_returns_401_without_token() {
    let app = app_with_auth();
    let req = Request::builder().uri("/mcp").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn mcp_401_includes_www_authenticate() {
    let app = app_with_auth();
    let req = Request::builder().uri("/mcp").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let www_auth = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("401 on /mcp must include WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(
        www_auth.starts_with("Bearer resource_metadata="),
        "WWW-Authenticate should start with 'Bearer resource_metadata=', got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("/.well-known/oauth-protected-resource"),
        "WWW-Authenticate should contain discovery URL, got: {}",
        www_auth
    );
}

#[tokio::test]
async fn mcp_returns_401_with_invalid_token() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/mcp")
        .header("Authorization", "Bearer invalid-jwt-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// -- MCP 401 with invalid token includes WWW-Authenticate --

#[tokio::test]
async fn mcp_invalid_token_401_includes_www_authenticate() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/mcp")
        .header("Authorization", "Bearer invalid-jwt-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let www_auth = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("401 on /mcp with invalid token must include WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(
        www_auth.starts_with("Bearer resource_metadata="),
        "WWW-Authenticate should start with 'Bearer resource_metadata=', got: {}",
        www_auth
    );
}

#[tokio::test]
async fn mcp_expired_token_401_includes_www_authenticate() {
    let app = app_with_auth();
    let token = generate_expired_test_jwt("user_expired");
    let req = Request::builder()
        .uri("/mcp")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let www_auth = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("401 on /mcp with expired token must include WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(
        www_auth.starts_with("Bearer resource_metadata="),
        "WWW-Authenticate should start with 'Bearer resource_metadata=', got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("scope=\"openid profile offline_access\""),
        "WWW-Authenticate should contain scope with offline_access, got: {}",
        www_auth
    );
}

#[tokio::test]
async fn rest_invalid_token_401_no_www_authenticate() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/v1/test")
        .header("Authorization", "Bearer invalid-jwt-token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    assert!(
        resp.headers().get(http::header::WWW_AUTHENTICATE).is_none(),
        "REST API 401 with invalid token should NOT include WWW-Authenticate"
    );
}

// -- MCP 401 scope in WWW-Authenticate --

#[tokio::test]
async fn mcp_401_includes_scope_in_www_authenticate() {
    let app = app_with_auth();
    let req = Request::builder().uri("/mcp").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let www_auth = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("401 on /mcp must include WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(
        www_auth.contains("scope=\"openid profile offline_access\""),
        "WWW-Authenticate should contain scope=\"openid profile offline_access\", got: {}",
        www_auth
    );
}

// -- MCP 403 forbidden response --

#[tokio::test]
async fn mcp_403_includes_insufficient_scope() {
    let resp = middleware::mcp_forbidden_response("http://localhost:8080", "openid profile admin");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let www_auth = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("403 must include WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(
        www_auth.contains("error=\"insufficient_scope\""),
        "WWW-Authenticate should contain error=\"insufficient_scope\", got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("scope=\"openid profile admin\""),
        "WWW-Authenticate should contain the required scope, got: {}",
        www_auth
    );
    assert!(
        www_auth.contains(
            "resource_metadata=\"http://localhost:8080/.well-known/oauth-protected-resource\""
        ),
        "WWW-Authenticate should contain resource_metadata URL, got: {}",
        www_auth
    );

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "Forbidden");
}

// -- Well-known endpoint content --

#[tokio::test]
async fn well_known_returns_correct_json_structure() {
    let app = app_with_auth();
    let req = Request::builder()
        .uri("/.well-known/oauth-protected-resource")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["resource"], "http://localhost:8080");
    assert!(json["authorization_servers"].is_array());
    assert!(!json["authorization_servers"].as_array().unwrap().is_empty());
    assert_eq!(
        json["bearer_methods_supported"],
        serde_json::json!(["header"])
    );
    assert_eq!(
        json["scopes_supported"],
        serde_json::json!(["openid", "profile", "offline_access"])
    );
}
