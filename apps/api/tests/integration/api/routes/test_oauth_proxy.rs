use axum::Router;
use axum::body::Body;
use axum::routing::post;
use http::{Request, StatusCode};
use tower::ServiceExt;
use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use std::path::PathBuf;
use std::sync::Arc;

use tokenoverflow::api::routes::oauth_proxy::{register, token};
use tokenoverflow::api::state::AppState;
use tokenoverflow::config::AuthConfig;
use tokenoverflow::services::repository::{
    PgAnswerRepository, PgQuestionRepository, PgSearchRepository, PgTagRepository, PgUserRepository,
};
use tokenoverflow::services::{AuthService, TagResolver};

use crate::test_db::IntegrationTestDb;

/// Stub embedding for proxy tests (never called).
struct StubEmbedding;

#[async_trait::async_trait]
impl tokenoverflow::external::embedding::EmbeddingService for StubEmbedding {
    async fn embed(
        &self,
        _text: &str,
    ) -> Result<Vec<f32>, tokenoverflow::external::embedding::EmbeddingError> {
        unreachable!("oauth proxy tests should not call embedding")
    }
}

/// Create a test AuthConfig whose authkit_url points to the given mock server.
fn auth_config_for_mock(mock_server_uri: &str) -> AuthConfig {
    let jwks_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/assets/auth/test_jwks.json")
        .to_string_lossy()
        .to_string();

    AuthConfig::new(
        "client_test".to_string(),
        "http://localhost:8080".to_string(),
        format!("file://{}", jwks_path),
        0,
        "tokenoverflow-test".to_string(),
        vec!["http://localhost:8080".to_string()],
        mock_server_uri.to_string(),
        "http://localhost:8080".to_string(),
    )
}

/// Build an AppState with a real DB pool and authkit_url pointing at a mock server.
async fn build_state(db: &IntegrationTestDb, mock_server_uri: &str) -> AppState {
    let pool = db.pool().clone();

    let tag_repo = Arc::new(PgTagRepository);
    let tag_resolver = {
        let mut conn = pool.get().await.expect("pool checkout for tag init");
        Arc::new(
            TagResolver::new(tag_repo.as_ref(), &mut *conn)
                .await
                .expect("tag resolver init should succeed"),
        )
    };
    let users = Arc::new(PgUserRepository);
    let auth_config = auth_config_for_mock(mock_server_uri);
    let auth = Arc::new(AuthService::new(auth_config.clone()));

    AppState::new(
        pool.clone(),
        Arc::new(StubEmbedding),
        Arc::new(PgQuestionRepository),
        Arc::new(PgAnswerRepository),
        Arc::new(PgSearchRepository),
        tag_repo,
        users,
        tag_resolver,
        auth,
        auth_config,
        "http://localhost:8080".to_string(),
    )
}

#[tokio::test]
async fn token_proxy_forwards_to_authkit() {
    let db = IntegrationTestDb::new().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(body_string("grant_type=authorization_code&code=test_code"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "access_token": "mock_access_token",
                    "token_type": "Bearer",
                    "expires_in": 3600
                }))
                .insert_header("content-type", "application/json"),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let state = build_state(&db, &mock_server.uri()).await;
    let app: Router = Router::new()
        .route("/oauth2/token", post(token))
        .with_state(state);

    let request = Request::builder()
        .method("POST")
        .uri("/oauth2/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("grant_type=authorization_code&code=test_code"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    assert_eq!(json["access_token"], "mock_access_token");
    assert_eq!(json["token_type"], "Bearer");
}

#[tokio::test]
async fn token_proxy_returns_502_when_authkit_unreachable() {
    let db = IntegrationTestDb::new().await;

    // Point authkit_url to a port where nothing is listening
    let state = build_state(&db, "http://127.0.0.1:1").await;
    let app: Router = Router::new()
        .route("/oauth2/token", post(token))
        .with_state(state);

    let request = Request::builder()
        .method("POST")
        .uri("/oauth2/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("grant_type=authorization_code&code=test_code"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn register_proxy_forwards_json_to_authkit() {
    let db = IntegrationTestDb::new().await;
    let mock_server = MockServer::start().await;

    let register_body = serde_json::json!({
        "client_name": "test-client",
        "redirect_uris": ["http://localhost:3000/callback"]
    });

    Mock::given(method("POST"))
        .and(path("/oauth2/register"))
        .and(header("content-type", "application/json"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({
                    "client_id": "new_client_id",
                    "client_name": "test-client",
                    "redirect_uris": ["http://localhost:3000/callback"]
                }))
                .insert_header("content-type", "application/json"),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let state = build_state(&db, &mock_server.uri()).await;
    let app: Router = Router::new()
        .route("/oauth2/register", post(register))
        .with_state(state);

    let request = Request::builder()
        .method("POST")
        .uri("/oauth2/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&register_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    assert_eq!(json["client_id"], "new_client_id");
    assert_eq!(json["client_name"], "test-client");
}

#[tokio::test]
async fn register_proxy_returns_502_when_authkit_unreachable() {
    let db = IntegrationTestDb::new().await;

    let state = build_state(&db, "http://127.0.0.1:1").await;
    let app: Router = Router::new()
        .route("/oauth2/register", post(register))
        .with_state(state);

    let register_body = serde_json::json!({
        "client_name": "test-client",
        "redirect_uris": ["http://localhost:3000/callback"]
    });

    let request = Request::builder()
        .method("POST")
        .uri("/oauth2/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&register_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}
