use uuid::Uuid;

use crate::api::types::{CreateQuestionResponse, QuestionWithAnswers};
use crate::error::AppError;
use crate::external::embedding::EmbeddingService;

use crate::services::TagResolver;
use crate::services::repository::{QuestionRepository, TagRepository};

pub struct QuestionService;

impl QuestionService {
    /// Create a new question with an initial answer
    ///
    /// Resolves tags through the tag registry, then generates embedding
    /// and delegates persistence to the repository.
    #[allow(clippy::too_many_arguments)]
    pub async fn create<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn QuestionRepository<Conn> + Sync),
        tag_repo: &(dyn TagRepository<Conn> + Sync),
        embedding: &dyn EmbeddingService,
        tag_resolver: &TagResolver,
        title: &str,
        body: &str,
        answer: &str,
        tags: Option<&[String]>,
        submitted_by: Uuid,
    ) -> Result<CreateQuestionResponse, AppError> {
        let embed_text = format!("{}\n\n{}", title, body);
        let embedding_vec = embedding
            .embed(&embed_text)
            .await
            .map_err(|e| AppError::EmbeddingUnavailable(e.to_string()))?;

        let response = repo
            .create(conn, title, body, answer, embedding_vec, submitted_by)
            .await?;

        // Resolve and link tags via the tag registry
        let resolved = tags
            .map(|t| tag_resolver.resolve_tags(t))
            .unwrap_or_default();

        if !resolved.is_empty() {
            let tag_pairs = tag_repo.find_tag_ids(conn, &resolved).await?;
            let tag_ids: Vec<Uuid> = tag_pairs.into_iter().map(|(_, id)| id).collect();
            tag_repo
                .link_tags_to_question(conn, response.question_id, &tag_ids)
                .await?;
        }

        Ok(response)
    }

    /// Get a question by ID with all its answers
    pub async fn get_by_id<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn QuestionRepository<Conn> + Sync),
        id: Uuid,
    ) -> Result<QuestionWithAnswers, AppError> {
        repo.get_by_id(conn, id).await
    }

    /// Check if a question exists
    pub async fn exists<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn QuestionRepository<Conn> + Sync),
        id: Uuid,
    ) -> Result<bool, AppError> {
        repo.exists(conn, id).await
    }
}
