use diesel::prelude::*;
use uuid::Uuid;

use crate::db::votes;

/// New vote for insertion/upsert
#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = votes)]
pub struct NewVote {
    pub answer_id: Uuid,
    pub user_id: Uuid,
    pub value: i32,
}
