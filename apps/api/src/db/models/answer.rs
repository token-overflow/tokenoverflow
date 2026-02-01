use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::answers;

/// Database entity for answers
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = answers)]
pub struct Answer {
    pub id: Uuid,
    pub question_id: Uuid,
    pub body: String,
    pub submitted_by: Uuid,
    pub upvotes: i32,
    pub downvotes: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New answer for insertion
#[derive(Debug, Insertable)]
#[diesel(table_name = answers)]
pub struct NewAnswer {
    pub question_id: Uuid,
    pub body: String,
    pub submitted_by: Uuid,
}
