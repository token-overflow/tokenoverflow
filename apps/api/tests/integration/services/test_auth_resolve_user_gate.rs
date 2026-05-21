//! Coverage for the four `resolve_user` branches added by the waitlist
//! gate:
//!
//! 1. existing `api.users` row → return user (find-step short-circuits;
//!    gate never runs).
//! 2. `require_waitlist_approval = false` → JIT-provision (rollout-
//!    deprecation path).
//! 3. approved waitlist row → JIT-provision and stamp
//!    `api.waitlist.user_id`.
//! 4. pending waitlist row → `WaitlistPending`.
//! 5. missing waitlist row → `WaitlistRequired`.
//!
//! WorkOS / GitHub round-trips are mocked with wiremock; the local
//! short-circuit branch of `resolve_github_identity` is not exercised
//! here (covered by `test_resolve_github_identity.rs`).

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use tokenoverflow::config::AuthConfig;
use tokenoverflow::error::AppError;
use tokenoverflow::services::AuthService;

mod common {
    include!("../../common/mod.rs");
}

use common::mock_repository::{MockStore, MockUserRepository, MockWaitlistRepository};

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

/// Build a config for tests that need WorkOS/GitHub mocked. The gate
/// flag is left at its default (`true`); individual tests flip it as
/// needed.
fn config_with_workos_mock(mock_server_uri: &str) -> AuthConfig {
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

#[tokio::test]
async fn existing_user_short_circuits_before_gate() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    // No WorkOS mocks: a hit on the gate path would 404 and surface as
    // an error, proving we never get there.
    let config = config_with_workos_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let users = MockUserRepository::new();
    users.seed_user("workos_already_provisioned");
    let waitlist = MockWaitlistRepository::new(MockStore::new());
    let mut conn = common::NoopConn;

    let user = service
        .resolve_user(&users, &waitlist, &mut conn, "workos_already_provisioned")
        .await
        .expect("existing-user branch must return the seeded user");

    assert_eq!(user.workos_id, "workos_already_provisioned");
}

#[tokio::test]
async fn flag_off_provisions_via_workos() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(&mock_server, "workos_flag_off", 200_001, "octo-flag-off").await;

    let mut config = config_with_workos_mock(&mock_server.uri());
    config.set_require_waitlist_approval_for_test(false);
    let service = AuthService::new(config);

    let users = MockUserRepository::new();
    let waitlist = MockWaitlistRepository::new(MockStore::new());
    let mut conn = common::NoopConn;

    let user = service
        .resolve_user(&users, &waitlist, &mut conn, "workos_flag_off")
        .await
        .expect("flag-off branch must JIT-provision");

    assert_eq!(user.workos_id, "workos_flag_off");
    assert_eq!(user.username, "octo-flag-off");
    assert_eq!(user.github_id, Some(200_001));
}

#[tokio::test]
async fn approved_waitlist_provisions_and_stamps_user_id() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(&mock_server, "workos_approved", 300_002, "octo-approved").await;

    let config = config_with_workos_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let users = MockUserRepository::new();
    let store = MockStore::new();
    let waitlist = MockWaitlistRepository::new(store.clone());
    waitlist.seed_pending(300_002, "octo-approved");
    // Flip pending -> approved by overwriting the stored row directly.
    {
        let mut entries = store.waitlist.lock().unwrap();
        let entry = entries.iter_mut().find(|e| e.github_id == 300_002).unwrap();
        entry.approved_at = Some(chrono::Utc::now());
    }
    let mut conn = common::NoopConn;

    let user = service
        .resolve_user(&users, &waitlist, &mut conn, "workos_approved")
        .await
        .expect("approved branch must JIT-provision");

    assert_eq!(user.workos_id, "workos_approved");
    assert_eq!(user.github_id, Some(300_002));

    // Audit-trail stamp: the waitlist row's `user_id` column must now
    // reference the freshly created user row.
    let stamped_user_id = {
        let entries = store.waitlist.lock().unwrap();
        entries
            .iter()
            .find(|e| e.github_id == 300_002)
            .and_then(|e| e.user_id)
    };
    assert_eq!(stamped_user_id, Some(user.id));
}

#[tokio::test]
async fn pending_waitlist_returns_waitlist_pending() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(&mock_server, "workos_pending", 400_003, "octo-pending").await;

    let config = config_with_workos_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let users = MockUserRepository::new();
    let store = MockStore::new();
    let waitlist = MockWaitlistRepository::new(store.clone());
    waitlist.seed_pending(400_003, "octo-pending");
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&users, &waitlist, &mut conn, "workos_pending")
        .await;

    assert!(matches!(result, Err(AppError::WaitlistPending)));
}

#[tokio::test]
async fn missing_waitlist_returns_waitlist_required() {
    let mock_server = MockServer::start().await;
    mount_jwks(&mock_server).await;
    mount_workos_github_pair(&mock_server, "workos_unknown", 500_004, "octo-unknown").await;

    let config = config_with_workos_mock(&mock_server.uri());
    let service = AuthService::new(config);

    let users = MockUserRepository::new();
    let waitlist = MockWaitlistRepository::new(MockStore::new());
    let mut conn = common::NoopConn;

    let result = service
        .resolve_user(&users, &waitlist, &mut conn, "workos_unknown")
        .await;

    assert!(matches!(result, Err(AppError::WaitlistRequired)));
}
