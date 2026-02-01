use axum::Router;
use axum::body::Body;
use axum::routing::get;
use http::{Request, StatusCode};
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

use tokenoverflow::api::routes::health::{HealthResponse, health_check};
use tokenoverflow::api::state::AppState;
use tokenoverflow::config::AuthConfig;
use tokenoverflow::services::repository::{
    PgAnswerRepository, PgQuestionRepository, PgSearchRepository, PgTagRepository, PgUserRepository,
};
use tokenoverflow::services::{AuthService, TagResolver};

use crate::test_db::IntegrationTestDb;

/// Stub embedding for health-check tests (never called).
struct StubEmbedding;

#[async_trait::async_trait]
impl tokenoverflow::external::embedding::EmbeddingService for StubEmbedding {
    async fn embed(
        &self,
        _text: &str,
    ) -> Result<Vec<f32>, tokenoverflow::external::embedding::EmbeddingError> {
        unreachable!("health check should not call embedding")
    }
}

/// Create a test AuthConfig pointing to the test JWKS file.
fn test_auth_config() -> AuthConfig {
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
        "http://localhost:8080".to_string(),
        "http://localhost:8080".to_string(),
    )
}

#[tokio::test]
async fn health_check_with_real_database_returns_connected() {
    let db = IntegrationTestDb::new().await;
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
    let auth_config = test_auth_config();
    let auth = Arc::new(AuthService::new(auth_config.clone()));

    let state = AppState::new(
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
    );

    let app: Router = Router::new()
        .route("/health", get(health_check))
        .with_state(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: HealthResponse = serde_json::from_slice(&body).expect("valid JSON");

    assert_eq!(health.status, "ok");
}
