use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{get, post};
use http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use http::{Method, Request, StatusCode};
use std::time::Duration;
use tokenoverflow::api::middleware::trace_id;
use tower::ServiceBuilder;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;

/// Build a minimal router with the full production middleware stack.
fn app_with_middleware() -> Router {
    Router::new()
        .route("/test", get(|| async { "ok" }))
        // Use Bytes extractor so DefaultBodyLimit is enforced
        .route("/echo", post(|_body: Bytes| async { "ok" }))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn(trace_id))
                .layer(DefaultBodyLimit::max(100 * 1024))
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::X_FRAME_OPTIONS,
                    HeaderValue::from_static("DENY"),
                ))
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods([Method::GET, Method::POST])
                        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
                        .max_age(Duration::from_secs(3600)),
                ),
        )
}

// -- Security Headers --

#[tokio::test]
async fn response_includes_x_content_type_options_nosniff() {
    let app = app_with_middleware();
    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
}

#[tokio::test]
async fn response_includes_x_frame_options_deny() {
    let app = app_with_middleware();
    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
}

// -- CORS --

#[tokio::test]
async fn cors_preflight_does_not_allow_x_api_key_header() {
    // x-api-key was removed because no API key auth exists.
    let app = app_with_middleware();
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/echo")
        .header("origin", "http://example.com")
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", "x-api-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    // tower-http CorsLayer returns 200 for preflight but omits
    // access-control-allow-headers when the requested header is not allowed.
    let allow_headers = resp
        .headers()
        .get("access-control-allow-headers")
        .map(|v| v.to_str().unwrap().to_lowercase());
    assert!(
        allow_headers.is_none() || !allow_headers.as_deref().unwrap_or("").contains("x-api-key"),
        "CORS should not allow x-api-key header, got: {:?}",
        allow_headers
    );
}

#[tokio::test]
async fn cors_preflight_allows_authorization_header() {
    let app = app_with_middleware();
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/echo")
        .header("origin", "http://example.com")
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", "authorization")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let allow_headers = resp
        .headers()
        .get("access-control-allow-headers")
        .unwrap()
        .to_str()
        .unwrap()
        .to_lowercase();
    assert!(
        allow_headers.contains("authorization"),
        "CORS should allow authorization header, got: {}",
        allow_headers
    );
}

#[tokio::test]
async fn cors_preflight_allows_content_type_header() {
    let app = app_with_middleware();
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/echo")
        .header("origin", "http://example.com")
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", "content-type")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let allow_headers = resp
        .headers()
        .get("access-control-allow-headers")
        .unwrap()
        .to_str()
        .unwrap()
        .to_lowercase();
    assert!(
        allow_headers.contains("content-type"),
        "CORS should allow content-type header, got: {}",
        allow_headers
    );
}

#[tokio::test]
async fn cors_allows_any_origin() {
    let app = app_with_middleware();
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/echo")
        .header("origin", "http://example.com")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "*"
    );
}

// -- Trace ID --

#[tokio::test]
async fn trace_id_always_present_in_response() {
    let app = app_with_middleware();
    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();

    let trace_id = resp
        .headers()
        .get("x-trace-id")
        .expect("X-Trace-Id header must always be present");

    // In local mode (no Lambda context), trace_id should be a valid UUID
    let id_str = trace_id.to_str().unwrap();
    assert!(
        uuid::Uuid::parse_str(id_str).is_ok(),
        "Expected a valid UUID trace ID, got: {}",
        id_str
    );
}

#[tokio::test]
async fn trace_id_is_unique_per_request() {
    let app = app_with_middleware();

    let req1 = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let resp1 = app.clone().oneshot(req1).await.unwrap();
    let id1 = resp1
        .headers()
        .get("x-trace-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let req2 = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let resp2 = app.oneshot(req2).await.unwrap();
    let id2 = resp2
        .headers()
        .get("x-trace-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    assert_ne!(id1, id2, "Each request must get a unique trace ID");
}

// -- Body Size Limit --

#[tokio::test]
async fn body_over_100kb_returns_413() {
    let app = app_with_middleware();
    // Create a body just over 100KB
    let oversized = vec![b'x'; 100 * 1024 + 1];
    let req = Request::builder()
        .method("POST")
        .uri("/echo")
        .header("content-type", "application/json")
        .body(Body::from(oversized))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn body_under_100kb_accepted() {
    let app = app_with_middleware();
    let small = vec![b'x'; 1024];
    let req = Request::builder()
        .method("POST")
        .uri("/echo")
        .header("content-type", "application/json")
        .body(Body::from(small))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
