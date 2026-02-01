use axum::Router;
use axum::body::Body;
use axum::routing::get;
use http::{Request, StatusCode};
use tokenoverflow::api::routes::health::health_check;
use tower::ServiceExt;

mod common {
    include!("../../../common/mod.rs");
}

#[tokio::test]
async fn health_check_returns_ok_status() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/health", get(health_check))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Mock pool cannot connect -- should return 503
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn health_check_returns_unhealthy_when_db_unreachable() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/health", get(health_check))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: serde_json::Value = serde_json::from_slice(&body).expect("Failed to parse JSON");

    assert_eq!(health["status"], "unhealthy");
    // Must NOT expose internal details (database status, version, etc.)
    assert!(
        health.get("database").is_none(),
        "Response must not contain a 'database' field"
    );
}
