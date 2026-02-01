#![allow(dead_code)]

// Axum request/response helpers for unit tests that use tower::ServiceExt::oneshot.

use axum::body::Body;
use axum::extract::Request as AxumRequest;
use axum::middleware::Next;
use axum::response::Response as AxumResponse;
use axum::Router;
use http::Request;
use tower::ServiceExt;

use tokenoverflow::api::extractors::AuthenticatedUser;
use tokenoverflow::constants::SYSTEM_USER_ID;

/// Test user ID injected by `fake_auth_layer` (matches seeded system user)
pub const TEST_AUTH_USER_ID: uuid::Uuid = SYSTEM_USER_ID;

/// Middleware that injects a fake `AuthenticatedUser` into request extensions.
///
/// Used by route-level unit tests to test handlers in isolation without
/// needing real JWT tokens or the auth middleware.
pub async fn fake_auth_layer(mut req: AxumRequest, next: Next) -> AxumResponse {
    req.extensions_mut().insert(AuthenticatedUser {
        id: TEST_AUTH_USER_ID,
        workos_id: "test-user".to_string(),
    });
    next.run(req).await
}

/// Middleware that injects `TEST_VOTER_ID` as the authenticated user.
///
/// Used by vote route tests where the voter must differ from the question author.
pub async fn fake_voter_auth_layer(mut req: AxumRequest, next: Next) -> AxumResponse {
    req.extensions_mut().insert(AuthenticatedUser {
        id: super::TEST_VOTER_ID,
        workos_id: "test-voter".to_string(),
    });
    next.run(req).await
}

pub async fn post_json(
    app: Router,
    uri: &str,
    body: impl serde::Serialize,
) -> axum::response::Response {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    app.oneshot(request).await.unwrap()
}

pub async fn post_empty(app: Router, uri: &str) -> axum::response::Response {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    app.oneshot(request).await.unwrap()
}

pub async fn get_request(app: Router, uri: &str) -> axum::response::Response {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    app.oneshot(request).await.unwrap()
}

pub async fn read_json(response: axum::response::Response) -> serde_json::Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).expect("Failed to parse JSON")
}
