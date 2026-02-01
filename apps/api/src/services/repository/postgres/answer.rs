use async_trait::async_trait;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

use crate::db::models::{NewAnswer, NewVote};
use crate::db::{answers, votes};
use crate::error::{AppError, diesel_fk_not_found};
use diesel::OptionalExtension;

use crate::services::repository::AnswerRepository;

pub struct PgAnswerRepository;

#[async_trait]
impl AnswerRepository<AsyncPgConnection> for PgAnswerRepository {
    async fn create(
        &self,
        conn: &mut AsyncPgConnection,
        question_id: Uuid,
        body: &str,
        submitted_by: Uuid,
    ) -> Result<Uuid, AppError> {
        let new_answer = NewAnswer {
            question_id,
            body: body.to_string(),
            submitted_by,
        };

        let answer_id: Uuid = diesel::insert_into(answers::table)
            .values(&new_answer)
            .returning(answers::id)
            .get_result(conn)
            .await
            .map_err(|e| diesel_fk_not_found("Question", question_id, e))?;

        Ok(answer_id)
    }

    async fn upvote(
        &self,
        conn: &mut AsyncPgConnection,
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.vote(conn, answer_id, user_id, 1).await
    }

    async fn downvote(
        &self,
        conn: &mut AsyncPgConnection,
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.vote(conn, answer_id, user_id, -1).await
    }

    async fn get_submitted_by(
        &self,
        conn: &mut AsyncPgConnection,
        answer_id: Uuid,
    ) -> Result<Uuid, AppError> {
        answers::table
            .filter(answers::id.eq(answer_id))
            .select(answers::submitted_by)
            .first::<Uuid>(conn)
            .await
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("Answer {} not found", answer_id)))
    }

    async fn exists(&self, conn: &mut AsyncPgConnection, id: Uuid) -> Result<bool, AppError> {
        let count: i64 = answers::table
            .filter(answers::id.eq(id))
            .count()
            .get_result(conn)
            .await?;

        Ok(count > 0)
    }
}

impl PgAnswerRepository {
    async fn vote(
        &self,
        conn: &mut AsyncPgConnection,
        answer_id: Uuid,
        user_id: Uuid,
        value: i32,
    ) -> Result<(), AppError> {
        let new_vote = NewVote {
            answer_id,
            user_id,
            value,
        };

        conn.transaction::<_, AppError, _>(|conn| {
            Box::pin(async move {
                diesel::insert_into(votes::table)
                    .values(&new_vote)
                    .on_conflict((votes::answer_id, votes::user_id))
                    .do_update()
                    .set(votes::value.eq(excluded(votes::value)))
                    .execute(conn)
                    .await
                    .map_err(|e| diesel_fk_not_found("Answer", answer_id, e))?;

                let upvote_count: i64 = votes::table
                    .filter(votes::answer_id.eq(answer_id))
                    .filter(votes::value.eq(1))
                    .count()
                    .get_result(conn)
                    .await?;

                let downvote_count: i64 = votes::table
                    .filter(votes::answer_id.eq(answer_id))
                    .filter(votes::value.eq(-1))
                    .count()
                    .get_result(conn)
                    .await?;

                diesel::update(answers::table.filter(answers::id.eq(answer_id)))
                    .set((
                        answers::upvotes.eq(upvote_count as i32),
                        answers::downvotes.eq(downvote_count as i32),
                    ))
                    .execute(conn)
                    .await?;

                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}
