//! Unit tests for WaitlistService.
//!
//! Uses in-memory mock repositories with NoopConn. The GitHub identity
//! lookup short-circuits via the seeded `MockUserRepository`. The email
//! comes straight from the caller (sourced in production from the JWT's
//! `email` claim) so no external service mocks are needed.

use tokenoverflow::config::AuthConfig;
use tokenoverflow::services::{AuthService, WaitlistService};

mod common {
    include!("../../common/mod.rs");
}

const TEST_WORKOS_ID: &str = "test-applicant";
const TEST_GITHUB_ID: i64 = 12345;
const TEST_GITHUB_LOGIN: &str = "octocat";
const TEST_EMAIL: &str = "octocat@example.test";

/// Build an `AuthService` whose config matches the test fixtures.
/// `resolve_github_identity` short-circuits on seeded users so no
/// network endpoints are exercised.
fn auth_service() -> AuthService {
    let config = AuthConfig::new(
        "http://localhost:8080".to_string(),
        "http://localhost:8080/jwks".to_string(),
        0,
        "tokenoverflow-test".to_string(),
        vec!["http://localhost:8080".to_string()],
        "http://localhost:8080".to_string(),
        "http://localhost:8080".to_string(),
    );
    AuthService::new(config)
}

#[tokio::test]
async fn add_first_time_returns_already_on_waitlist_false() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let users = common::MockUserRepository::new();
    users.seed_github_user(TEST_WORKOS_ID, TEST_GITHUB_ID, TEST_GITHUB_LOGIN);
    let waitlist = common::MockWaitlistRepository::new(store.clone());
    let auth = auth_service();

    let result = WaitlistService::add(
        &auth,
        &users,
        &waitlist,
        &mut conn,
        TEST_WORKOS_ID,
        TEST_EMAIL,
    )
    .await;

    let outcome = result.expect("first insert must succeed");
    assert_eq!(outcome.github_id, TEST_GITHUB_ID);
    assert_eq!(outcome.github_username, TEST_GITHUB_LOGIN);
    assert!(!outcome.already_on_waitlist);

    let stored_email = {
        let entries = store.waitlist.lock().unwrap();
        entries
            .iter()
            .find(|e| e.github_id == TEST_GITHUB_ID)
            .map(|e| e.email.clone())
    };
    assert_eq!(stored_email.as_deref(), Some(TEST_EMAIL));
}

#[tokio::test]
async fn add_second_time_returns_already_on_waitlist_true() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let users = common::MockUserRepository::new();
    users.seed_github_user(TEST_WORKOS_ID, TEST_GITHUB_ID, TEST_GITHUB_LOGIN);
    let waitlist = common::MockWaitlistRepository::new(store.clone());
    let auth = auth_service();

    WaitlistService::add(
        &auth,
        &users,
        &waitlist,
        &mut conn,
        TEST_WORKOS_ID,
        TEST_EMAIL,
    )
    .await
    .expect("first insert must succeed");

    let result = WaitlistService::add(
        &auth,
        &users,
        &waitlist,
        &mut conn,
        TEST_WORKOS_ID,
        TEST_EMAIL,
    )
    .await;

    let outcome = result.expect("second insert must succeed");
    assert_eq!(outcome.github_id, TEST_GITHUB_ID);
    assert_eq!(outcome.github_username, TEST_GITHUB_LOGIN);
    assert!(outcome.already_on_waitlist);
}

#[tokio::test]
async fn add_propagates_repository_errors() {
    let mut conn = common::NoopConn;
    let users = common::MockUserRepository::new();
    users.seed_github_user(TEST_WORKOS_ID, TEST_GITHUB_ID, TEST_GITHUB_LOGIN);
    let waitlist = common::FailingWaitlistRepository;
    let auth = auth_service();

    let result = WaitlistService::add(
        &auth,
        &users,
        &waitlist,
        &mut conn,
        TEST_WORKOS_ID,
        TEST_EMAIL,
    )
    .await;

    assert!(result.is_err());
}
