//! MCP error-surface coverage for waitlist denials.
//!
//! When `resolve_user` returns `WaitlistPending` / `WaitlistRequired` on
//! a `/mcp` request, the response must:
//! - Use status 403 (token is valid; the user lacks access).
//! - Include an RFC 6750 S3.1 `WWW-Authenticate` challenge:
//!   `error="insufficient_scope"`, `error_description="WAITLIST_..."`,
//!   `scope="openid profile offline_access"`, `resource_metadata="..."`.
//!   `error_description` carries the stable enum so clients branch on it
//!   without a vendor-specific header.
//! - Carry a JSON body containing the error code.
//!
//! Without this surface a stale-token-style 401 would force Claude Code
//! into a token-refresh loop that never resolves the underlying issue.
//!
//! REST callers receive the standard `AppError::IntoResponse` mapping:
//! 403 + `{ "error": "WAITLIST_PENDING" | "WAITLIST_REQUIRED" }`.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::routing::post;
use http::{Request, StatusCode};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use tower::ServiceExt;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use tokenoverflow::api::middleware;
use tokenoverflow::api::state::AppState;
use tokenoverflow::config::AuthConfig;
use tokenoverflow::services::AuthService;

mod common {
    include!("../../common/mod.rs");
}

use common::mock_repository::{
    FailingAnswerRepository, FailingQuestionRepository, FailingSearchRepository,
    FailingTagRepository, MockStore, MockUserRepository, MockWaitlistRepository,
};

use crate::test_db::IntegrationTestDb;

/// JWKS JSON matching the test private key.
const TEST_JWKS_JSON: &str = r#"{
    "keys": [{
        "kty": "RSA",
        "use": "sig",
        "alg": "RS256",
        "kid": "test-key-1",
        "n": "tPcSfr_BXeW9Su0L-PiLOeDh72kUCBWqRmPMQMg4WFzF7MT8C7_xuqGWuDt45BqvXtHP3Pn0YjsMDdT0v0le5huWmtMsp-3LxHB4XzyzUbzznVAxdlWyE6WuXzLoRrXNweaKM2BVGu9bspoSZvWsbQdgiOZq-ZXAq8E4aLrqtrdBdywfVYiUydVEX97m_zbhyIPPhmx_9ztBEQfhqnXjQKZnASe13Kd3t4a2vqFdgxWPoE38P2M-5NkNKEkJZVifx--2iVn6bxWRAXh_KWRp6-FvfCRhsCOwv0oaq1o-M9gP4FEf32493zuLHvl1q7kcygUVoTX0nYqirEa3R33A2Q",
        "e": "AQAB"
    }]
}"#;

/// Build an AppState whose JWKS, WorkOS identities, and GitHub user
/// endpoints all live on the supplied `MockServer`. The audience matches
/// the existing test JWT helpers so signed tokens validate cleanly.
fn build_state(
    mock_server: &MockServer,
    api_base_url: &str,
    users: Arc<MockUserRepository>,
    waitlist: Arc<MockWaitlistRepository>,
    pool: tokenoverflow::db::DbPool,
) -> AppState {
    let store = MockStore::with_seed_tags();
    let mut config = AuthConfig::new(
        mock_server.uri(),
        format!("{}/jwks", mock_server.uri()),
        0,
        "tokenoverflow-test".to_string(),
        vec!["http://localhost:8080".to_string()],
        "http://localhost:8080".to_string(),
        mock_server.uri(),
    );
    config.set_workos_api_key_for_test("sk_test_fake_key".to_string());
    config.set_github_oauth_for_test(
        "test_client_id".to_string(),
        "test_client_secret".to_string(),
    );
    let auth = Arc::new(AuthService::new(config.clone()));
    let questions = Arc::new(FailingQuestionRepository);
    let answers = Arc::new(FailingAnswerRepository);
    let search = Arc::new(FailingSearchRepository);
    let tags = Arc::new(FailingTagRepository);
    let tag_resolver = Arc::new(common::create_tag_resolver(&store));
    AppState::new(
        pool,
        Arc::new(common::MockEmbedding::new()),
        questions,
        answers,
        search,
        tags,
        users,
        waitlist,
        tag_resolver,
        auth,
        config,
        api_base_url.to_string(),
    )
}

async fn mount_jwks(mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TEST_JWKS_JSON, "application/json"))
        .mount(mock_server)
        .await;
}

async fn mount_workos_github_pair(
    mock_server: &MockServer,
    workos_id: &str,
    gh_id: i64,
    gh_login: &str,
) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/user_management/users/{}/identities",
            workos_id
        )))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": gh_id.to_string(), "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .mount(mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/user/{}", gh_id)))
        .and(header("User-Agent", "TokenOverflow API"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": gh_login,
            "id": gh_id
        })))
        .mount(mock_server)
        .await;
}

/// Sign a JWT against the bundled test private key. Inlined so the
/// audience matches the localhost expectation regardless of the
/// wiremock URI we feed the AuthService.
fn sign_jwt(sub: &str) -> String {
    let private_key = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets/auth/test_private_key.pem"),
    )
    .expect("test private key must exist");
    let key = EncodingKey::from_rsa_pem(&private_key).expect("test key must parse");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-key-1".to_string());
    let now = jsonwebtoken::get_current_timestamp();
    let claims = serde_json::json!({
        "sub": sub,
        "email": "test@example.test",
        "iss": "tokenoverflow-test",
        "aud": "http://localhost:8080",
        "exp": now + 3600,
        "iat": now,
    });
    encode(&header, &claims, &key).unwrap()
}

async fn drive_mcp(state: AppState, token: &str) -> http::Response<Body> {
    let app = Router::new()
        .route("/mcp", post(|| async { "mcp ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    app.oneshot(req).await.unwrap()
}

async fn drive_rest(state: AppState, token: &str) -> http::Response<Body> {
    let app = Router::new()
        .route("/v1/test", post(|| async { "rest ok" }))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::jwt_auth_layer,
        ))
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/test")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    app.oneshot(req).await.unwrap()
}

#[tokio::test]
async fn mcp_waitlist_required_signals_required() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(&mock_server, "workos_mcp_required", 800_001, "octo-mcp").await;

    let db = IntegrationTestDb::new().await;
    let users = Arc::new(MockUserRepository::new());
    let waitlist = Arc::new(MockWaitlistRepository::new(MockStore::new()));
    let state = build_state(
        &mock_server,
        "http://api.test",
        users,
        waitlist,
        db.pool().clone(),
    );

    let token = sign_jwt("workos_mcp_required");
    let resp = drive_mcp(state, &token).await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(
        resp.headers().get("x-tokenoverflow-auth-status").is_none(),
        "MCP waitlist response must not emit any vendor-specific X- header (RFC 6648)",
    );

    let www_auth = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("MCP waitlist response must include WWW-Authenticate")
        .to_str()
        .unwrap();
    assert!(
        www_auth.contains("error=\"insufficient_scope\""),
        "WWW-Authenticate must use the standard insufficient_scope error, got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("error_description=\"WAITLIST_REQUIRED\""),
        "WWW-Authenticate must carry the cause in error_description, got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("scope=\"openid profile offline_access\""),
        "scope must list real OAuth scopes (not the waitlist code), got: {}",
        www_auth
    );

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "WAITLIST_REQUIRED");
}

#[tokio::test]
async fn mcp_waitlist_pending_signals_pending() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(&mock_server, "workos_mcp_pending", 800_002, "octo-pending").await;

    let db = IntegrationTestDb::new().await;
    let users = Arc::new(MockUserRepository::new());
    let store = MockStore::new();
    let waitlist = Arc::new(MockWaitlistRepository::new(store));
    waitlist.seed_pending(800_002, "octo-pending");
    let state = build_state(
        &mock_server,
        "http://api.test",
        users,
        waitlist,
        db.pool().clone(),
    );

    let token = sign_jwt("workos_mcp_pending");
    let resp = drive_mcp(state, &token).await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(
        resp.headers().get("x-tokenoverflow-auth-status").is_none(),
        "MCP waitlist response must not emit any vendor-specific X- header (RFC 6648)",
    );

    let www_auth = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("MCP waitlist response must include WWW-Authenticate")
        .to_str()
        .unwrap();
    assert!(
        www_auth.contains("error=\"insufficient_scope\""),
        "WWW-Authenticate must use the standard insufficient_scope error, got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("error_description=\"WAITLIST_PENDING\""),
        "WWW-Authenticate must carry the cause in error_description, got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("scope=\"openid profile offline_access\""),
        "scope must list real OAuth scopes (not the waitlist code), got: {}",
        www_auth
    );

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "WAITLIST_PENDING");
}

#[tokio::test]
async fn rest_waitlist_required_returns_standard_403_body() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(
        &mock_server,
        "workos_rest_required",
        800_003,
        "octo-rest-req",
    )
    .await;

    let db = IntegrationTestDb::new().await;
    let users = Arc::new(MockUserRepository::new());
    let waitlist = Arc::new(MockWaitlistRepository::new(MockStore::new()));
    let state = build_state(
        &mock_server,
        "http://api.test",
        users,
        waitlist,
        db.pool().clone(),
    );

    let token = sign_jwt("workos_rest_required");
    let resp = drive_rest(state, &token).await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    // No vendor-specific X- header anywhere (RFC 6648).
    assert!(
        resp.headers().get("x-tokenoverflow-auth-status").is_none(),
        "REST 403 must not carry any vendor-specific X- status header"
    );
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "WAITLIST_REQUIRED");
}

#[tokio::test]
async fn rest_waitlist_pending_returns_standard_403_body() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(
        &mock_server,
        "workos_rest_pending",
        800_004,
        "octo-rest-pen",
    )
    .await;

    let db = IntegrationTestDb::new().await;
    let users = Arc::new(MockUserRepository::new());
    let store = MockStore::new();
    let waitlist = Arc::new(MockWaitlistRepository::new(store));
    waitlist.seed_pending(800_004, "octo-rest-pen");
    let state = build_state(
        &mock_server,
        "http://api.test",
        users,
        waitlist,
        db.pool().clone(),
    );

    let token = sign_jwt("workos_rest_pending");
    let resp = drive_rest(state, &token).await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    // No vendor-specific X- header anywhere (RFC 6648).
    assert!(resp.headers().get("x-tokenoverflow-auth-status").is_none());
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "WAITLIST_PENDING");
}
