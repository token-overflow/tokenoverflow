#![allow(dead_code)]

use async_trait::async_trait;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokenoverflow::api::state::AppState;
use tokenoverflow::config::AuthConfig;
use tokenoverflow::external::embedding::{EmbeddingError, EmbeddingService};
use tokenoverflow::services::{AuthService, TagResolver};

use super::mock_repository::{
    FailingAnswerRepository, FailingQuestionRepository, FailingSearchRepository,
    FailingTagRepository, FailingUserRepository, MockAnswerRepository, MockQuestionRepository,
    MockSearchRepository, MockStore, MockTagRepository, MockUserRepository,
};
use super::noop_conn::NoopConn;

/// Mock embedding adapter for testing.
///
/// Generates deterministic embeddings based on text hash.
/// Same input text always produces the same embedding vector.
pub struct MockEmbedding {
    dimension: usize,
}

impl MockEmbedding {
    pub fn new() -> Self {
        Self { dimension: 256 }
    }
}

impl Default for MockEmbedding {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingService for MockEmbedding {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let seed = hasher.finish();

        let mut rng = StdRng::seed_from_u64(seed);
        let embedding: Vec<f32> = (0..self.dimension)
            .map(|_| rng.gen_range(-1.0..1.0))
            .collect();

        // Normalize to unit vector for cosine similarity
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        Ok(embedding.iter().map(|x| x / magnitude).collect())
    }
}

/// Mock embedding that always fails.
pub struct FailingMockEmbedding;

#[async_trait]
impl EmbeddingService for FailingMockEmbedding {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Err(EmbeddingError::Api("mock embedding failure".to_string()))
    }
}

/// Create a non-connecting pool for test AppState.
///
/// Uses `build_unchecked` so no actual connection attempt is made during
/// construction. Any operation that tries to use the pool will fail.
fn create_dummy_pool() -> tokenoverflow::db::DbPool {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(
        "postgres://invalid:invalid@127.0.0.1:1/nonexistent",
    );
    Pool::builder()
        .max_size(1)
        .connection_timeout(Duration::from_millis(1))
        .build_unchecked(config)
}

/// Create a TagResolver from the seed data in a MockStore.
pub fn create_tag_resolver(store: &MockStore) -> TagResolver {
    let synonyms = {
        let s = store.synonyms.lock().unwrap();
        s.iter()
            .map(|s| (s.synonym.clone(), s.canonical.clone()))
            .collect()
    };
    let canonicals = {
        let t = store.tags.lock().unwrap();
        t.iter().map(|t| t.name.clone()).collect()
    };
    TagResolver::from_data(synonyms, canonicals)
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

/// Create `(NoopConn, MockStore)` for service-level unit tests.
/// No pool, no Docker, no external dependencies.
pub fn create_mock_service_deps() -> (NoopConn, MockStore) {
    let store = MockStore::with_seed_tags();
    (NoopConn, store)
}

/// Create a mock AppState using in-memory repos and mock embedding.
///
/// Uses seed tags from the migration so tag resolution works in tests.
/// The contained `pool` field is a dummy that will fail if actually used.
/// For tests that call `pool.get()` (handlers, MCP tools), use
/// `create_mock_app_state_with_pool()` with a real testcontainers pool.
pub fn create_mock_app_state() -> AppState {
    let store = MockStore::with_seed_tags();
    create_app_state_with_store(&store)
}

/// Create a mock AppState backed by a real pool.
///
/// For handler/MCP integration tests that need `pool.get()` to succeed.
pub fn create_mock_app_state_with_pool(pool: tokenoverflow::db::DbPool) -> AppState {
    let store = MockStore::with_seed_tags();
    create_app_state_with_store_and_pool(&store, pool)
}

/// Create an AppState where all repos and the embedding service always fail.
pub fn create_failing_mock_app_state() -> AppState {
    let pool = create_dummy_pool();
    let embedding = Arc::new(FailingMockEmbedding);
    let questions = Arc::new(FailingQuestionRepository);
    let answers = Arc::new(FailingAnswerRepository);
    let search = Arc::new(FailingSearchRepository);
    let tags = Arc::new(FailingTagRepository);
    let users = Arc::new(FailingUserRepository);
    // Empty resolver for failing state -- no tags to resolve
    let tag_resolver = Arc::new(TagResolver::from_data(
        std::collections::HashMap::new(),
        vec![],
    ));
    let auth_config = test_auth_config();
    let auth = Arc::new(AuthService::new(auth_config.clone()));
    AppState::new(
        pool,
        embedding,
        questions,
        answers,
        search,
        tags,
        users,
        tag_resolver,
        auth,
        auth_config,
        "http://localhost:8080".to_string(),
    )
}

/// Create an AppState where all repos and the embedding service always fail,
/// backed by a real pool.
pub fn create_failing_mock_app_state_with_pool(pool: tokenoverflow::db::DbPool) -> AppState {
    let embedding = Arc::new(FailingMockEmbedding);
    let questions = Arc::new(FailingQuestionRepository);
    let answers = Arc::new(FailingAnswerRepository);
    let search = Arc::new(FailingSearchRepository);
    let tags = Arc::new(FailingTagRepository);
    let users = Arc::new(FailingUserRepository);
    // Empty resolver for failing state -- no tags to resolve
    let tag_resolver = Arc::new(TagResolver::from_data(
        std::collections::HashMap::new(),
        vec![],
    ));
    let auth_config = test_auth_config();
    let auth = Arc::new(AuthService::new(auth_config.clone()));
    AppState::new(
        pool,
        embedding,
        questions,
        answers,
        search,
        tags,
        users,
        tag_resolver,
        auth,
        auth_config,
        "http://localhost:8080".to_string(),
    )
}

/// Create a mock AppState with pre-seeded test users.
pub fn create_mock_app_state_with_users(workos_ids: &[&str]) -> AppState {
    let store = MockStore::with_seed_tags();
    let pool = create_dummy_pool();
    let embedding = Arc::new(MockEmbedding::new());
    let questions = Arc::new(MockQuestionRepository::new(store.clone()));
    let answers = Arc::new(MockAnswerRepository::new(store.clone()));
    let search = Arc::new(MockSearchRepository::new(store.clone()));
    let tags = Arc::new(MockTagRepository::new(store.clone()));
    let users = Arc::new(MockUserRepository::new());
    for id in workos_ids {
        users.seed_user(id);
    }
    let tag_resolver = Arc::new(create_tag_resolver(&store));
    let auth_config = test_auth_config();
    let auth = Arc::new(AuthService::new(auth_config.clone()));
    AppState::new(
        pool,
        embedding,
        questions,
        answers,
        search,
        tags,
        users,
        tag_resolver,
        auth,
        auth_config,
        "http://localhost:8080".to_string(),
    )
}

/// Create a mock AppState with pre-seeded test users, backed by a real pool.
pub fn create_mock_app_state_with_users_and_pool(
    workos_ids: &[&str],
    pool: tokenoverflow::db::DbPool,
) -> AppState {
    let store = MockStore::with_seed_tags();
    let embedding = Arc::new(MockEmbedding::new());
    let questions = Arc::new(MockQuestionRepository::new(store.clone()));
    let answers = Arc::new(MockAnswerRepository::new(store.clone()));
    let search = Arc::new(MockSearchRepository::new(store.clone()));
    let tags = Arc::new(MockTagRepository::new(store.clone()));
    let users = Arc::new(MockUserRepository::new());
    for id in workos_ids {
        users.seed_user(id);
    }
    let tag_resolver = Arc::new(create_tag_resolver(&store));
    let auth_config = test_auth_config();
    let auth = Arc::new(AuthService::new(auth_config.clone()));
    AppState::new(
        pool,
        embedding,
        questions,
        answers,
        search,
        tags,
        users,
        tag_resolver,
        auth,
        auth_config,
        "http://localhost:8080".to_string(),
    )
}

/// Create an AppState backed by the given MockStore (uses dummy pool).
pub fn create_app_state_with_store(store: &MockStore) -> AppState {
    let pool = create_dummy_pool();
    create_app_state_with_store_and_pool(store, pool)
}

/// Create an AppState backed by the given MockStore and a real pool.
pub fn create_app_state_with_store_and_pool(
    store: &MockStore,
    pool: tokenoverflow::db::DbPool,
) -> AppState {
    let embedding = Arc::new(MockEmbedding::new());
    let questions = Arc::new(MockQuestionRepository::new(store.clone()));
    let answers = Arc::new(MockAnswerRepository::new(store.clone()));
    let search = Arc::new(MockSearchRepository::new(store.clone()));
    let tags = Arc::new(MockTagRepository::new(store.clone()));
    let users = Arc::new(MockUserRepository::new());
    let tag_resolver = Arc::new(create_tag_resolver(store));
    let auth_config = test_auth_config();
    let auth = Arc::new(AuthService::new(auth_config.clone()));
    AppState::new(
        pool,
        embedding,
        questions,
        answers,
        search,
        tags,
        users,
        tag_resolver,
        auth,
        auth_config,
        "http://localhost:8080".to_string(),
    )
}
