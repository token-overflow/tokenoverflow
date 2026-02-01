use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use tokenoverflow::config::AuthConfig;
use tokenoverflow::services::AuthService;

mod common {
    include!("../../common/mod.rs");
}

use common::mock_repository::MockUserRepository;
use common::test_jwt::generate_test_jwt_custom;

/// JWKS JSON matching the test private key (same as tests/assets/auth/test_jwks.json).
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

fn auth_config_for_mock(mock_server_uri: &str) -> AuthConfig {
    let mut config = AuthConfig::new(
        "client_test".to_string(),
        mock_server_uri.to_string(),
        format!("{}/jwks", mock_server_uri),
        0, // no caching to ensure each test hits the mock
        "tokenoverflow-test".to_string(),
        vec!["http://localhost:8080".to_string()],
        "http://localhost:8080".to_string(),
        mock_server_uri.to_string(),
    );
    config.set_workos_api_key_for_test("sk_test_fake_key".to_string());
    config.set_github_oauth_for_test(
        "test_client_id".to_string(),
        "test_client_secret".to_string(),
    );
    config
}

/// Mount wiremock stubs for the JIT provisioning happy path:
/// WorkOS identities and GitHub user.
async fn mount_jit_provisioning_mocks(mock_server: &MockServer) {
    // WorkOS identities
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_new_jit/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "12345", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .mount(mock_server)
        .await;

    // GitHub user
    Mock::given(method("GET"))
        .and(path("/user/12345"))
        .and(header("User-Agent", "TokenOverflow API"))
        .and(header(
            "Authorization",
            "Basic dGVzdF9jbGllbnRfaWQ6dGVzdF9jbGllbnRfc2VjcmV0",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": "testuser",
            "id": 12345
        })))
        .mount(mock_server)
        .await;
}

// -- HTTP JWKS loading --

#[tokio::test]
async fn validate_jwt_fetches_jwks_over_http() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TEST_JWKS_JSON, "application/json"))
        .expect(1..)
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let token = generate_test_jwt_custom(
        "user_http_jwks",
        "tokenoverflow-test",
        "http://localhost:8080",
        3600,
    );

    let claims = service
        .validate_jwt(&token)
        .await
        .expect("should validate JWT using HTTP-fetched JWKS");

    assert_eq!(claims.sub, "user_http_jwks");
}

#[tokio::test]
async fn validate_jwt_returns_error_when_jwks_endpoint_fails() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let token = generate_test_jwt_custom(
        "user_fail",
        "tokenoverflow-test",
        "http://localhost:8080",
        3600,
    );

    let result = service.validate_jwt(&token).await;
    assert!(
        result.is_err(),
        "should fail when JWKS endpoint returns 500"
    );
}

// -- resolve_user with JIT provisioning via WorkOS API --

#[tokio::test]
async fn resolve_user_returns_existing_user() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TEST_JWKS_JSON, "application/json"))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let user_repo = MockUserRepository::new();
    user_repo.seed_user("user_existing");
    let mut conn = common::NoopConn;

    let user = service
        .resolve_user(&user_repo, &mut conn, "user_existing")
        .await
        .expect("should find existing user");

    assert_eq!(user.workos_id, "user_existing");
}

#[tokio::test]
async fn resolve_user_creates_user_via_workos_api() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TEST_JWKS_JSON, "application/json"))
        .mount(&mock_server)
        .await;

    mount_jit_provisioning_mocks(&mock_server).await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let user = service
        .resolve_user(&user_repo, &mut conn, "user_new_jit")
        .await
        .expect("should create user from WorkOS profile");

    assert_eq!(user.workos_id, "user_new_jit");
    assert_eq!(user.username, "testuser");
    assert_eq!(user.github_id, Some(12345));
}

#[tokio::test]
async fn resolve_user_handles_missing_api_key() {
    let mock_server = MockServer::start().await;

    // Config without WorkOS API key set
    let config = AuthConfig::new(
        "client_test".to_string(),
        mock_server.uri(),
        format!("{}/jwks", mock_server.uri()),
        0,
        "tokenoverflow-test".to_string(),
        vec!["http://localhost:8080".to_string()],
        "http://localhost:8080".to_string(),
        mock_server.uri(),
    );

    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_no_key")
        .await;

    assert!(
        result.is_err(),
        "should fail when WorkOS API key is not configured"
    );
}

#[tokio::test]
async fn resolve_user_resolves_github_username_and_id() {
    let mock_server = MockServer::start().await;

    // WorkOS identities
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_minimal/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "583231", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .expect(1)
        .mount(&mock_server)
        .await;

    // GitHub user
    Mock::given(method("GET"))
        .and(path("/user/583231"))
        .and(header("User-Agent", "TokenOverflow API"))
        .and(header(
            "Authorization",
            "Basic dGVzdF9jbGllbnRfaWQ6dGVzdF9jbGllbnRfc2VjcmV0",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": "octocat",
            "id": 583231
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let user = service
        .resolve_user(&user_repo, &mut conn, "user_minimal")
        .await
        .expect("should resolve GitHub username and ID");

    assert_eq!(user.workos_id, "user_minimal");
    assert_eq!(user.username, "octocat");
    assert_eq!(user.github_id, Some(583231));
}

#[tokio::test]
async fn resolve_user_handles_identities_api_failure() {
    let mock_server = MockServer::start().await;

    // WorkOS identities returns 500
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_ident_fail/identities"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_ident_fail")
        .await;
    assert!(
        result.is_err(),
        "should fail when identities endpoint returns 500"
    );
}

#[tokio::test]
async fn resolve_user_handles_no_github_identity() {
    let mock_server = MockServer::start().await;

    // WorkOS identities returns empty array
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_no_gh/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_no_gh")
        .await;
    assert!(
        result.is_err(),
        "should fail when no GitHub identity is found"
    );
}

#[tokio::test]
async fn resolve_user_handles_github_api_failure() {
    let mock_server = MockServer::start().await;

    // WorkOS identities succeeds
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_gh_fail/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "99999", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .mount(&mock_server)
        .await;

    // GitHub API returns 500
    Mock::given(method("GET"))
        .and(path("/user/99999"))
        .and(header("User-Agent", "TokenOverflow API"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_gh_fail")
        .await;
    assert!(result.is_err(), "should fail when GitHub API returns 500");
}

#[tokio::test]
async fn resolve_user_handles_github_api_not_found() {
    let mock_server = MockServer::start().await;

    // WorkOS identities succeeds
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_gh_404/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "88888", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .mount(&mock_server)
        .await;

    // GitHub API returns 404
    Mock::given(method("GET"))
        .and(path("/user/88888"))
        .and(header("User-Agent", "TokenOverflow API"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "message": "Not Found"
        })))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_gh_404")
        .await;
    assert!(result.is_err(), "should fail when GitHub API returns 404");
}

#[tokio::test]
async fn resolve_user_falls_back_to_unauthenticated_github_api() {
    let mock_server = MockServer::start().await;

    // WorkOS identities succeeds
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_no_gh_token/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "77777", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .mount(&mock_server)
        .await;

    // GitHub user endpoint -- no Authorization header expected
    Mock::given(method("GET"))
        .and(path("/user/77777"))
        .and(header("User-Agent", "TokenOverflow API"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": "unauthuser",
            "id": 77777
        })))
        .mount(&mock_server)
        .await;

    // Config with WorkOS API key but without GitHub API token
    let mut config = AuthConfig::new(
        "client_test".to_string(),
        mock_server.uri(),
        format!("{}/jwks", mock_server.uri()),
        0,
        "tokenoverflow-test".to_string(),
        vec!["http://localhost:8080".to_string()],
        "http://localhost:8080".to_string(),
        mock_server.uri(),
    );
    config.set_workos_api_key_for_test("sk_test_fake_key".to_string());
    // Deliberately NOT setting github_oauth credentials

    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let user = service
        .resolve_user(&user_repo, &mut conn, "user_no_gh_token")
        .await
        .expect("should succeed with unauthenticated GitHub API call");

    assert_eq!(user.username, "unauthuser");
    assert_eq!(user.github_id, Some(77777));
}

#[tokio::test]
async fn resolve_user_handles_non_numeric_github_id() {
    let mock_server = MockServer::start().await;

    // WorkOS identities returns non-numeric idp_id
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_bad_id/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "not-a-number", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_bad_id")
        .await;
    assert!(result.is_err(), "should fail when GitHub ID is not numeric");
}

#[tokio::test]
async fn resolve_user_handles_malformed_identities_response() {
    let mock_server = MockServer::start().await;

    // WorkOS identities returns invalid JSON
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_bad_ident/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"not json"#, "application/json"))
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_bad_ident")
        .await;
    assert!(
        result.is_err(),
        "should fail when identities response is malformed"
    );
}

#[tokio::test]
async fn resolve_user_handles_malformed_github_user_response() {
    let mock_server = MockServer::start().await;

    // WorkOS identities succeeds
    Mock::given(method("GET"))
        .and(path("/user_management/users/user_bad_gh/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "44444", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .mount(&mock_server)
        .await;

    // GitHub API returns invalid JSON structure (missing required fields)
    Mock::given(method("GET"))
        .and(path("/user/44444"))
        .and(header("User-Agent", "TokenOverflow API"))
        .and(header(
            "Authorization",
            "Basic dGVzdF9jbGllbnRfaWQ6dGVzdF9jbGllbnRfc2VjcmV0",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"{"no_login": true}"#, "application/json"),
        )
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);
    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&user_repo, &mut conn, "user_bad_gh")
        .await;
    assert!(
        result.is_err(),
        "should fail when GitHub user response is malformed"
    );
}
