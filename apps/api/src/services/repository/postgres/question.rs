use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use pgvector::Vector;
use uuid::Uuid;

use crate::api::types::{AnswerResponse, CreateQuestionResponse, QuestionWithAnswers};
use crate::db::models::{Answer, NewAnswer, NewQuestion, Question};
use crate::db::{answers, question_tags, questions, tags};
use crate::error::AppError;

use crate::services::repository::QuestionRepository;

pub struct PgQuestionRepository;

#[async_trait]
impl QuestionRepository<AsyncPgConnection> for PgQuestionRepository {
    async fn create(
        &self,
        conn: &mut AsyncPgConnection,
        title: &str,
        body: &str,
        answer_body: &str,
        embedding: Vec<f32>,
        submitted_by: Uuid,
    ) -> Result<CreateQuestionResponse, AppError> {
        let embedding_vector = Vector::from(embedding);

        let (question_id, answer_id) = conn
            .transaction::<_, diesel::result::Error, _>(|conn| {
                let title = title.to_string();
                let body = body.to_string();
                let answer_body = answer_body.to_string();

                Box::pin(async move {
                    let new_question = NewQuestion {
                        title,
                        body,
                        embedding: embedding_vector,
                        submitted_by,
                    };

                    let question_id: Uuid = diesel::insert_into(questions::table)
                        .values(&new_question)
                        .returning(questions::id)
                        .get_result(conn)
                        .await?;

                    let new_answer = NewAnswer {
                        question_id,
                        body: answer_body,
                        submitted_by,
                    };

                    let answer_id: Uuid = diesel::insert_into(answers::table)
                        .values(&new_answer)
                        .returning(answers::id)
                        .get_result(conn)
                        .await?;

                    Ok((question_id, answer_id))
                })
            })
            .await?;

        Ok(CreateQuestionResponse {
            question_id,
            answer_id,
        })
    }

    async fn get_by_id(
        &self,
        conn: &mut AsyncPgConnection,
        id: Uuid,
    ) -> Result<QuestionWithAnswers, AppError> {
        let question: Question = questions::table
            .find(id)
            .select(Question::as_select())
            .first(conn)
            .await
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("Question {} not found", id)))?;

        let tag_names: Vec<String> = question_tags::table
            .inner_join(tags::table)
            .filter(question_tags::question_id.eq(id))
            .select(tags::name)
            .load(conn)
            .await?;

        let answer_rows: Vec<Answer> = answers::table
            .filter(answers::question_id.eq(id))
            .order((answers::upvotes.desc(), answers::created_at.asc()))
            .load(conn)
            .await?;

        Ok(QuestionWithAnswers {
            id: question.id,
            title: question.title,
            body: question.body,
            tags: tag_names,
            created_at: question.created_at,
            answers: answer_rows.into_iter().map(AnswerResponse::from).collect(),
        })
    }

    async fn exists(&self, conn: &mut AsyncPgConnection, id: Uuid) -> Result<bool, AppError> {
        use diesel::dsl::count_star;

        let count: i64 = questions::table
            .filter(questions::id.eq(id))
            .select(count_star())
            .first(conn)
            .await?;

        Ok(count > 0)
    }
}
