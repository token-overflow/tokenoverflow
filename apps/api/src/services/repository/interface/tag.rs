use std::collections::HashMap;

use async_trait::async_trait;
use diesel_async::AsyncPgConnection;
use uuid::Uuid;

use crate::error::AppError;

/// Contract for tag persistence operations.
#[async_trait]
pub trait TagRepository<Conn: Send = AsyncPgConnection>: Send + Sync {
    /// Load all synonym mappings: synonym -> canonical_name.
    async fn load_synonyms(&self, conn: &mut Conn) -> Result<HashMap<String, String>, AppError>;

    /// Load all canonical tag names.
    async fn load_canonicals(&self, conn: &mut Conn) -> Result<Vec<String>, AppError>;

    /// Find tag IDs for a list of canonical names.
    /// Returns only the names that exist in the tags table.
    async fn find_tag_ids(
        &self,
        conn: &mut Conn,
        names: &[String],
    ) -> Result<Vec<(String, Uuid)>, AppError>;

    /// Insert question_tags rows for a given question.
    async fn link_tags_to_question(
        &self,
        conn: &mut Conn,
        question_id: Uuid,
        tag_ids: &[Uuid],
    ) -> Result<(), AppError>;

    /// Get tag names for a question via the join table.
    async fn get_question_tags(
        &self,
        conn: &mut Conn,
        question_id: Uuid,
    ) -> Result<Vec<String>, AppError>;
}
