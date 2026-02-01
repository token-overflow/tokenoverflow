use std::sync::Arc;

use diesel_async::AsyncPgConnection;

use crate::config::AuthConfig;
use crate::db::DbPool;
use crate::external::embedding::EmbeddingService;
use crate::services::repository::{
    AnswerRepository, QuestionRepository, SearchRepository, TagRepository, UserRepository,
};
use crate::services::{AuthService, TagResolver};

/// Application state shared across all request handlers
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool (retained for health check endpoint)
    pub pool: DbPool,

    /// Embedding adapter (OpenAI or mock)
    pub embedding: Arc<dyn EmbeddingService>,

    /// Question persistence
    pub questions: Arc<dyn QuestionRepository<AsyncPgConnection> + Sync>,

    /// Answer persistence
    pub answers: Arc<dyn AnswerRepository<AsyncPgConnection> + Sync>,

    /// Search persistence
    pub search: Arc<dyn SearchRepository<AsyncPgConnection> + Sync>,

    /// Tag persistence
    pub tags: Arc<dyn TagRepository<AsyncPgConnection> + Sync>,

    /// User persistence
    pub users: Arc<dyn UserRepository<AsyncPgConnection> + Sync>,

    /// In-memory tag resolver (synonym + canonical + Jaro-Winkler)
    pub tag_resolver: Arc<TagResolver>,

    /// Authentication service (JWKS + JWT validation + user resolution)
    pub auth: Arc<AuthService>,

    /// Auth configuration (for well-known metadata endpoints)
    pub auth_config: AuthConfig,

    /// API base URL (e.g., `https://api.tokenoverflow.io`)
    pub api_base_url: String,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: DbPool,
        embedding: Arc<dyn EmbeddingService>,
        questions: Arc<dyn QuestionRepository<AsyncPgConnection> + Sync>,
        answers: Arc<dyn AnswerRepository<AsyncPgConnection> + Sync>,
        search: Arc<dyn SearchRepository<AsyncPgConnection> + Sync>,
        tags: Arc<dyn TagRepository<AsyncPgConnection> + Sync>,
        users: Arc<dyn UserRepository<AsyncPgConnection> + Sync>,
        tag_resolver: Arc<TagResolver>,
        auth: Arc<AuthService>,
        auth_config: AuthConfig,
        api_base_url: String,
    ) -> Self {
        Self {
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
            api_base_url,
        }
    }
}
