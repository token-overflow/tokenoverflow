use async_trait::async_trait;
use diesel_async::AsyncPgConnection;

use crate::api::types::SearchResultQuestion;
use crate::error::AppError;

/// Contract for semantic search operations.
#[async_trait]
pub trait SearchRepository<Conn: Send = AsyncPgConnection>: Send + Sync {
    /// Search for questions by embedding similarity, optionally filtered by tags.
    async fn search(
        &self,
        conn: &mut Conn,
        embedding: Vec<f32>,
        tags: Option<&[String]>,
        limit: i32,
    ) -> Result<Vec<SearchResultQuestion>, AppError>;
}
