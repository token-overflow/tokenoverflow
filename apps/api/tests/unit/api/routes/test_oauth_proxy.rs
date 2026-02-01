use axum::Router;
use axum::body::Body;
use axum::routing::get;
use http::{Request, StatusCode};
use tokenoverflow::api::routes::oauth_proxy::authorize;
use tower::ServiceExt;

mod common {
    include!("../../../common/mod.rs");
}

#[tokio::test]
async fn authorize_adds_scope_when_missing() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/oauth2/authorize", get(authorize))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/oauth2/authorize?client_id=abc&state=xyz")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .expect("redirect must have a Location header")
        .to_str()
        .unwrap();

    // The proxy should have injected scope=openid+profile+offline_access
    assert!(
        location.contains("scope=openid+profile+offline_access")
            || location.contains("scope=openid%20profile%20offline_access"),
        "Location should contain scope=openid profile offline_access, got: {}",
        location
    );
    // Original params must still be present
    assert!(
        location.contains("client_id=abc"),
        "Location should preserve client_id, got: {}",
        location
    );
    assert!(
        location.contains("state=xyz"),
        "Location should preserve state, got: {}",
        location
    );
}

#[tokio::test]
async fn authorize_preserves_valid_scope() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/oauth2/authorize", get(authorize))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/oauth2/authorize?client_id=abc&scope=email")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .expect("redirect must have a Location header")
        .to_str()
        .unwrap();

    // Valid scope should be preserved
    assert!(
        location.contains("scope=email"),
        "Location should preserve valid scope, got: {}",
        location
    );
}

#[tokio::test]
async fn authorize_replaces_empty_scope() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/oauth2/authorize", get(authorize))
        .with_state(app_state);

    // Claude Code sends scope="" (empty quoted string)
    let request = Request::builder()
        .uri("/oauth2/authorize?client_id=abc&scope=%22%22")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .expect("redirect must have a Location header")
        .to_str()
        .unwrap();

    assert!(
        location.contains("scope=openid+profile+offline_access")
            || location.contains("scope=openid%20profile%20offline_access"),
        "Location should inject scope when empty, got: {}",
        location
    );
}

#[tokio::test]
async fn authorize_preserves_all_query_params() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/oauth2/authorize", get(authorize))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/oauth2/authorize?client_id=my_client&state=random_state&redirect_uri=http%3A%2F%2Flocalhost%3A3000%2Fcallback&response_type=code")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .expect("redirect must have a Location header")
        .to_str()
        .unwrap();

    assert!(
        location.contains("client_id=my_client"),
        "Location should preserve client_id, got: {}",
        location
    );
    assert!(
        location.contains("state=random_state"),
        "Location should preserve state, got: {}",
        location
    );
    assert!(
        location.contains("redirect_uri="),
        "Location should preserve redirect_uri, got: {}",
        location
    );
    assert!(
        location.contains("response_type=code"),
        "Location should preserve response_type, got: {}",
        location
    );
}

#[tokio::test]
async fn authorize_redirects_to_authkit() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/oauth2/authorize", get(authorize))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/oauth2/authorize?client_id=abc")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .expect("redirect must have a Location header")
        .to_str()
        .unwrap();

    // Mock state uses authkit_url = "http://localhost:8080"
    assert!(
        location.starts_with("http://localhost:8080/oauth2/authorize?"),
        "Location should start with authkit_url/oauth2/authorize, got: {}",
        location
    );
}
