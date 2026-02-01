use uuid::Uuid;

use crate::error::AppError;

use crate::services::repository::AnswerRepository;

pub struct AnswerService;

impl AnswerService {
    /// Add an answer to an existing question
    pub async fn create<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn AnswerRepository<Conn> + Sync),
        question_id: Uuid,
        body: &str,
        submitted_by: Uuid,
    ) -> Result<Uuid, AppError> {
        repo.create(conn, question_id, body, submitted_by).await
    }

    /// Upvote an answer
    ///
    /// Idempotent: calling twice has no additional effect.
    pub async fn upvote<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn AnswerRepository<Conn> + Sync),
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        Self::guard_self_vote(conn, repo, answer_id, user_id).await?;
        repo.upvote(conn, answer_id, user_id).await
    }

    /// Downvote an answer
    ///
    /// Idempotent: calling twice has no additional effect.
    pub async fn downvote<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn AnswerRepository<Conn> + Sync),
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        Self::guard_self_vote(conn, repo, answer_id, user_id).await?;
        repo.downvote(conn, answer_id, user_id).await
    }

    async fn guard_self_vote<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn AnswerRepository<Conn> + Sync),
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        let author_id = repo.get_submitted_by(conn, answer_id).await?;
        if author_id == user_id {
            return Err(AppError::Forbidden(
                "You cannot vote on your own answer".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if an answer exists
    pub async fn exists<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn AnswerRepository<Conn> + Sync),
        id: Uuid,
    ) -> Result<bool, AppError> {
        repo.exists(conn, id).await
    }
}
