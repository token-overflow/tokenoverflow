use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{Double, Integer, Text};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use pgvector::Vector;
use uuid::Uuid;

use crate::api::types::{AnswerResponse, SearchResultQuestion};
use crate::db::models::Answer;
use crate::db::{answers, question_tags, tags};
use crate::error::AppError;

use crate::services::repository::SearchRepository;

/// Row returned from the pgvector similarity search query.
#[derive(QueryableByName)]
struct SearchRow {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub title: String,
    #[diesel(sql_type = Text)]
    pub body: String,
    #[diesel(sql_type = Double)]
    pub similarity: f64,
}

pub struct PgSearchRepository;

#[async_trait]
impl SearchRepository<AsyncPgConnection> for PgSearchRepository {
    async fn search(
        &self,
        conn: &mut AsyncPgConnection,
        embedding: Vec<f32>,
        tags: Option<&[String]>,
        limit: i32,
    ) -> Result<Vec<SearchResultQuestion>, AppError> {
        let query_vector = Vector::from(embedding);

        let has_tags = tags.is_some_and(|t| !t.is_empty());

        let questions: Vec<SearchRow> = if has_tags {
            // Filter by tags via join table
            diesel::sql_query(
                r#"
                SELECT DISTINCT
                    q.id,
                    q.title,
                    q.body,
                    1 - (q.embedding <=> $1::vector) AS similarity
                FROM api.questions q
                JOIN api.question_tags qt ON qt.question_id = q.id
                JOIN api.tags t ON t.id = qt.tag_id
                WHERE t.name = ANY($2::text[])
                ORDER BY similarity DESC
                LIMIT $3
                "#,
            )
            .bind::<pgvector::sql_types::Vector, _>(&query_vector)
            .bind::<diesel::sql_types::Array<Text>, _>(tags.unwrap())
            .bind::<Integer, _>(limit)
            .load(conn)
            .await?
        } else {
            diesel::sql_query(
                r#"
                SELECT
                    q.id,
                    q.title,
                    q.body,
                    1 - (q.embedding <=> $1::vector) AS similarity
                FROM api.questions q
                ORDER BY similarity DESC
                LIMIT $2
                "#,
            )
            .bind::<pgvector::sql_types::Vector, _>(&query_vector)
            .bind::<Integer, _>(limit)
            .load(conn)
            .await?
        };

        let question_ids: Vec<Uuid> = questions.iter().map(|q| q.id).collect();

        if question_ids.is_empty() {
            return Ok(vec![]);
        }

        let answer_rows: Vec<Answer> = answers::table
            .filter(answers::question_id.eq_any(&question_ids))
            .order((answers::upvotes.desc(), answers::created_at.asc()))
            .select(Answer::as_select())
            .load(conn)
            .await?;

        // Load tags for all returned questions via join table
        let tag_rows: Vec<(Uuid, String)> = question_tags::table
            .inner_join(tags::table)
            .filter(question_tags::question_id.eq_any(&question_ids))
            .select((question_tags::question_id, tags::name))
            .load(conn)
            .await?;

        let results = questions
            .into_iter()
            .map(|q| {
                let question_answers: Vec<AnswerResponse> = answer_rows
                    .iter()
                    .filter(|a| a.question_id == q.id)
                    .cloned()
                    .map(AnswerResponse::from)
                    .collect();

                let question_tags: Vec<String> = tag_rows
                    .iter()
                    .filter(|(qid, _)| *qid == q.id)
                    .map(|(_, name)| name.clone())
                    .collect();

                SearchResultQuestion {
                    id: q.id,
                    title: q.title,
                    body: q.body,
                    tags: question_tags,
                    similarity: q.similarity,
                    answers: question_answers,
                }
            })
            .collect();

        Ok(results)
    }
}
