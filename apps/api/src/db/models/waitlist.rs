use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::waitlist;

/// Waitlist entry for a confirmed sign-up.
///
/// `approved_at` is the gate consumed by `resolve_user`: null is pending,
/// non-null grants access. `user_id` is set when an approved applicant
/// first signs in and an `api.users` row is provisioned (audit trail).
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = waitlist)]
pub struct WaitlistEntry {
    pub id: Uuid,
    pub github_id: i64,
    pub github_username: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub user_id: Option<Uuid>,
}

/// New waitlist row payload for insertion.
#[derive(Debug, Insertable)]
#[diesel(table_name = waitlist)]
pub struct NewWaitlistEntry<'a> {
    pub github_id: i64,
    pub github_username: &'a str,
    pub email: &'a str,
}
