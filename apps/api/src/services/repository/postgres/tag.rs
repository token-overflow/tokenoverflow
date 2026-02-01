use std::collections::HashMap;

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

use crate::db::{question_tags, tag_synonyms, tags};
use crate::error::AppError;

use crate::services::repository::TagRepository;

pub struct PgTagRepository;

#[async_trait]
impl TagRepository<AsyncPgConnection> for PgTagRepository {
    async fn load_synonyms(
        &self,
        conn: &mut AsyncPgConnection,
    ) -> Result<HashMap<String, String>, AppError> {
        let rows: Vec<(String, String)> = tag_synonyms::table
            .inner_join(tags::table)
            .select((tag_synonyms::synonym, tags::name))
            .load(conn)
            .await?;

        Ok(rows.into_iter().collect())
    }

    async fn load_canonicals(&self, conn: &mut AsyncPgConnection) -> Result<Vec<String>, AppError> {
        let names: Vec<String> = tags::table
            .select(tags::name)
            .order(tags::name.asc())
            .load(conn)
            .await?;

        Ok(names)
    }

    async fn find_tag_ids(
        &self,
        conn: &mut AsyncPgConnection,
        names: &[String],
    ) -> Result<Vec<(String, Uuid)>, AppError> {
        if names.is_empty() {
            return Ok(vec![]);
        }

        let rows: Vec<(String, Uuid)> = tags::table
            .filter(tags::name.eq_any(names))
            .select((tags::name, tags::id))
            .load(conn)
            .await?;

        Ok(rows)
    }

    async fn link_tags_to_question(
        &self,
        conn: &mut AsyncPgConnection,
        question_id: Uuid,
        tag_ids: &[Uuid],
    ) -> Result<(), AppError> {
        if tag_ids.is_empty() {
            return Ok(());
        }

        let values: Vec<(Uuid, Uuid)> = tag_ids.iter().map(|&tid| (question_id, tid)).collect();

        diesel::insert_into(question_tags::table)
            .values(
                values
                    .iter()
                    .map(|(qid, tid)| {
                        (
                            question_tags::question_id.eq(qid),
                            question_tags::tag_id.eq(tid),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .on_conflict_do_nothing()
            .execute(conn)
            .await?;

        Ok(())
    }

    async fn get_question_tags(
        &self,
        conn: &mut AsyncPgConnection,
        question_id: Uuid,
    ) -> Result<Vec<String>, AppError> {
        let names: Vec<String> = question_tags::table
            .inner_join(tags::table)
            .filter(question_tags::question_id.eq(question_id))
            .select(tags::name)
            .load(conn)
            .await?;

        Ok(names)
    }
}
