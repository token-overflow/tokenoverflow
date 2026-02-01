use chrono::{DateTime, Utc};
use diesel::prelude::*;
use pgvector::Vector;
use uuid::Uuid;

use crate::db::questions;

/// New question for insertion via Diesel
#[derive(Debug, Insertable)]
#[diesel(table_name = questions)]
pub struct NewQuestion {
    pub title: String,
    pub body: String,
    pub embedding: Vector,
    pub submitted_by: Uuid,
}

/// Question entity for queries (excludes embedding for efficiency)
#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = questions)]
pub struct Question {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}
