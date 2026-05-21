//! Integration tests for `AuthService::resolve_github_identity`.
//!
//! Two paths matter:
//! 1. Local mode: a row exists in `api.users` keyed by `workos_id`. The
//!    helper short-circuits and returns `(github_id, username)` without
//!    a network call.
//! 2. Production mode: the local lookup misses; the helper falls through
//!    to the WorkOS Management API (mocked here via wiremock) and the
//!    GitHub `/user/{id}` endpoint, returning the resolved pair.
//!
//! The third documented branch (local lookup misses AND no WorkOS API
//! key) is covered by `resolve_user_handles_missing_api_key` in
//! `test_auth.rs`; reproducing it here would be redundant.

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use tokenoverflow::config::AuthConfig;
use tokenoverflow::services::AuthService;

mod common {
    include!("../../common/mod.rs");
}

use common::mock_repository::MockUserRepository;

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
        mock_server_uri.to_string(),
        format!("{}/jwks", mock_server_uri),
        0,
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

#[tokio::test]
async fn local_path_short_circuits_with_seeded_user() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TEST_JWKS_JSON, "application/json"))
        .mount(&mock_server)
        .await;

    // No identities/user mocks: if the helper hits WorkOS, the assertion
    // below fails because wiremock returns 404 for unmounted paths.
    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let user_repo = MockUserRepository::new();
    user_repo.seed_github_user("workos_local_seed", 555_000, "octocat-local");
    let mut conn = common::NoopConn;

    let (github_id, github_login) = service
        .resolve_github_identity(&user_repo, &mut conn, "workos_local_seed")
        .await
        .expect("local path must short-circuit on the seeded user");

    assert_eq!(github_id, 555_000);
    assert_eq!(github_login, "octocat-local");
}

#[tokio::test]
async fn workos_path_resolves_when_local_miss() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TEST_JWKS_JSON, "application/json"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/user_management/users/workos_no_local_row/identities",
        ))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "777777", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .expect(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/user/777777"))
        .and(header("User-Agent", "TokenOverflow API"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": "octocat-fresh",
            "id": 777_777
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let (github_id, github_login) = service
        .resolve_github_identity(&user_repo, &mut conn, "workos_no_local_row")
        .await
        .expect("workos path must resolve when local row is missing");

    assert_eq!(github_id, 777_777);
    assert_eq!(github_login, "octocat-fresh");
}

#[tokio::test]
async fn local_user_without_github_id_falls_through_to_workos() {
    // The seeded `system` user in MockUserRepository::new has `github_id =
    // None`. The helper must not return that incomplete row; it must fall
    // through to the WorkOS branch.
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TEST_JWKS_JSON, "application/json"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/user_management/users/system/identities"))
        .and(header("Authorization", "Bearer sk_test_fake_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "idp_id": "1", "type": "OAuth", "provider": "GithubOAuth" }
        ])))
        .expect(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/user/1"))
        .and(header("User-Agent", "TokenOverflow API"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": "fallback-login",
            "id": 1
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = auth_config_for_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let user_repo = MockUserRepository::new();
    let mut conn = common::NoopConn;

    let (github_id, github_login) = service
        .resolve_github_identity(&user_repo, &mut conn, "system")
        .await
        .expect("incomplete local row must trigger workos fallback");

    assert_eq!(github_id, 1);
    assert_eq!(github_login, "fallback-login");
}
