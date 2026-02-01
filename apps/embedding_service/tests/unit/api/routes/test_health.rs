use axum::Router;
use axum::body::Body;
use axum::routing::get;
use embedding_service::api::routes::health::health_check;
use http::Request;
use tower::ServiceExt;

#[tokio::test]
async fn health_check_returns_ok_status() {
    let app: Router = Router::new().route("/health", get(health_check));

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(request).await.unwrap();

    assert!(resp.status().is_success());
}

#[tokio::test]
async fn health_check_returns_valid_json() {
    let app: Router = Router::new().route("/health", get(health_check));

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Failed to parse JSON");

    assert_eq!(json["status"], "ok");
}
