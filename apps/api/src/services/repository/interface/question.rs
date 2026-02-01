use async_trait::async_trait;
use diesel_async::AsyncPgConnection;
use uuid::Uuid;

use crate::api::types::{CreateQuestionResponse, QuestionWithAnswers};
use crate::error::AppError;

/// Contract for question persistence operations.
///
/// Accepts `Vec<f32>` for embeddings (not pgvector types) to keep
/// Diesel-specific types out of the trait interface.
#[async_trait]
pub trait QuestionRepository<Conn: Send = AsyncPgConnection>: Send + Sync {
    /// Insert a question and its initial answer in a single transaction.
    async fn create(
        &self,
        conn: &mut Conn,
        title: &str,
        body: &str,
        answer_body: &str,
        embedding: Vec<f32>,
        submitted_by: Uuid,
    ) -> Result<CreateQuestionResponse, AppError>;

    /// Fetch a question by ID with all its answers.
    async fn get_by_id(&self, conn: &mut Conn, id: Uuid) -> Result<QuestionWithAnswers, AppError>;

    /// Check if a question exists.
    async fn exists(&self, conn: &mut Conn, id: Uuid) -> Result<bool, AppError>;
}
