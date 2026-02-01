use async_trait::async_trait;
use diesel_async::AsyncPgConnection;
use uuid::Uuid;

use crate::error::AppError;

/// Contract for answer persistence operations.
#[async_trait]
pub trait AnswerRepository<Conn: Send = AsyncPgConnection>: Send + Sync {
    /// Insert an answer for a question.
    async fn create(
        &self,
        conn: &mut Conn,
        question_id: Uuid,
        body: &str,
        submitted_by: Uuid,
    ) -> Result<Uuid, AppError>;

    /// Record an upvote (idempotent per user).
    async fn upvote(&self, conn: &mut Conn, answer_id: Uuid, user_id: Uuid)
    -> Result<(), AppError>;

    /// Record a downvote (idempotent per user).
    async fn downvote(
        &self,
        conn: &mut Conn,
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError>;

    /// Return the `submitted_by` UUID for an answer.
    async fn get_submitted_by(&self, conn: &mut Conn, answer_id: Uuid) -> Result<Uuid, AppError>;

    /// Check if an answer exists.
    async fn exists(&self, conn: &mut Conn, id: Uuid) -> Result<bool, AppError>;
}
