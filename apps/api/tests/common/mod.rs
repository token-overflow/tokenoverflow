// Common test utilities.

/// A voter identity distinct from `SYSTEM_USER_ID`, used in vote tests to avoid
/// triggering the self-vote guard.
#[allow(dead_code)]
pub const TEST_VOTER_ID: uuid::Uuid = uuid::Uuid::from_bytes([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02,
]);

#[allow(clippy::duplicate_mod)]
pub mod axum_helpers;
#[allow(clippy::duplicate_mod)]
pub mod fixtures;
#[allow(clippy::duplicate_mod)]
pub mod http_client;
#[allow(clippy::duplicate_mod)]
pub mod mock_repository;
#[allow(clippy::duplicate_mod)]
mod mock_embedding;
#[allow(clippy::duplicate_mod)]
pub mod noop_conn;
#[allow(clippy::duplicate_mod)]
pub mod test_jwt;

// Re-exports for easy test imports
#[allow(unused_imports)]
pub use axum_helpers::{
    fake_auth_layer, fake_voter_auth_layer, get_request, post_empty, post_json, read_json,
};
#[allow(unused_imports)]
pub use fixtures::{AnswerRequestBuilder, QuestionRequestBuilder, SearchRequestBuilder};
#[allow(unused_imports)]
pub use http_client::TestClient;
#[allow(unused_imports)]
pub use mock_embedding::{
    create_app_state_with_store, create_app_state_with_store_and_pool,
    create_failing_mock_app_state, create_failing_mock_app_state_with_pool,
    create_mock_app_state, create_mock_app_state_with_pool,
    create_mock_app_state_with_users, create_mock_app_state_with_users_and_pool,
    create_mock_service_deps, create_tag_resolver, FailingMockEmbedding, MockEmbedding,
};
#[allow(unused_imports)]
pub use mock_repository::{
    FailingAnswerRepository, FailingQuestionRepository, FailingSearchRepository,
    FailingTagRepository, FailingUserRepository, MockAnswerRepository, MockQuestionRepository,
    MockSearchRepository, MockStore, MockTagRepository, MockUserRepository,
};
#[allow(unused_imports)]
pub use noop_conn::NoopConn;
#[allow(unused_imports)]
pub use test_jwt::{
    generate_expired_test_jwt, generate_test_jwt, generate_test_jwt_custom,
    generate_test_jwt_with_kid,
};
