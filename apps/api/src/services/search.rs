use crate::api::types::SearchResultQuestion;
use crate::error::AppError;
use crate::external::embedding::EmbeddingService;

use crate::services::TagResolver;
use crate::services::repository::SearchRepository;

pub struct SearchService;

impl SearchService {
    /// Search for questions using semantic similarity
    ///
    /// Resolves tags through the tag registry, generates embedding for the
    /// query, then delegates the vector search to the repository.
    pub async fn search<Conn: Send>(
        conn: &mut Conn,
        repo: &(dyn SearchRepository<Conn> + Sync),
        embedding: &dyn EmbeddingService,
        tag_resolver: &TagResolver,
        query: &str,
        tags: Option<&[String]>,
        limit: i32,
    ) -> Result<Vec<SearchResultQuestion>, AppError> {
        let query_embedding = embedding
            .embed(query)
            .await
            .map_err(|e| AppError::EmbeddingUnavailable(e.to_string()))?;

        let resolved = tags.map(|t| tag_resolver.resolve_tags(t));
        let resolved_ref = resolved.as_deref();

        repo.search(conn, query_embedding, resolved_ref, limit)
            .await
    }
}
