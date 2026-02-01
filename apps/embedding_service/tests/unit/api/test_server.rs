use axum::Router;
use axum::body::Body;
use embedding_service::api::routes;
use embedding_service::api::state::AppState;
use http::Request;
use tower::ServiceExt;

fn create_test_app_state() -> AppState {
    AppState::new(super::routes::TEST_MODEL.clone())
}

#[tokio::test]
async fn app_configures_health_endpoint() {
    let app_state = create_test_app_state();
    let app: Router = routes::configure().with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(request).await.unwrap();

    assert!(resp.status().is_success());
}

#[tokio::test]
async fn app_configures_embeddings_endpoint() {
    let app_state = create_test_app_state();
    let app: Router = routes::configure().with_state(app_state);

    let body = serde_json::json!({
        "input": "test",
        "model": "test-model"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(request).await.unwrap();

    assert!(resp.status().is_success());
}

#[tokio::test]
async fn app_returns_404_for_unknown_routes() {
    let app_state = create_test_app_state();
    let app: Router = routes::configure().with_state(app_state);

    let request = Request::builder()
        .uri("/unknown/route")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(request).await.unwrap();

    assert_eq!(resp.status().as_u16(), 404);
}
