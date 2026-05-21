use async_trait::async_trait;
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

use crate::db::models::{NewWaitlistEntry, WaitlistEntry};
use crate::db::waitlist;
use crate::error::AppError;
use crate::services::repository::{WaitlistInsert, WaitlistRepository};

pub struct PgWaitlistRepository;

#[async_trait]
impl WaitlistRepository<AsyncPgConnection> for PgWaitlistRepository {
    async fn insert(
        &self,
        conn: &mut AsyncPgConnection,
        github_id: i64,
        github_username: &str,
        email: &str,
    ) -> Result<WaitlistInsert, AppError> {
        let new_entry = NewWaitlistEntry {
            github_id,
            github_username,
            email,
        };

        // ON CONFLICT (github_id) DO NOTHING returns no rows on conflict, which
        // we surface as `already_existed = true`. The caller does not need the
        // existing row's id/created_at, so we look it up only on the conflict
        // path to keep the insert path a single round-trip.
        let inserted = diesel::insert_into(waitlist::table)
            .values(&new_entry)
            .on_conflict(waitlist::github_id)
            .do_nothing()
            .returning((waitlist::id, waitlist::created_at))
            .get_result::<(uuid::Uuid, chrono::DateTime<chrono::Utc>)>(conn)
            .await
            .optional()?;

        match inserted {
            Some((id, created_at)) => Ok(WaitlistInsert {
                id,
                created_at,
                already_existed: false,
            }),
            None => {
                let (id, created_at) = waitlist::table
                    .filter(waitlist::github_id.eq(github_id))
                    .select((waitlist::id, waitlist::created_at))
                    .first::<(uuid::Uuid, chrono::DateTime<chrono::Utc>)>(conn)
                    .await?;

                Ok(WaitlistInsert {
                    id,
                    created_at,
                    already_existed: true,
                })
            }
        }
    }

    async fn find_by_github_id(
        &self,
        conn: &mut AsyncPgConnection,
        github_id: i64,
    ) -> Result<Option<WaitlistEntry>, AppError> {
        let entry = waitlist::table
            .filter(waitlist::github_id.eq(github_id))
            .select(WaitlistEntry::as_select())
            .first(conn)
            .await
            .optional()?;

        Ok(entry)
    }

    async fn set_user_id(
        &self,
        conn: &mut AsyncPgConnection,
        github_id: i64,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        // Idempotent: re-stamping the same user_id is a no-op. We do not
        // assert the row exists because `set_user_id` is only called after
        // the gate has already observed the row, and a missing row at this
        // point would indicate a concurrent admin scrub (acceptable miss).
        diesel::update(waitlist::table.filter(waitlist::github_id.eq(github_id)))
            .set(waitlist::user_id.eq(user_id))
            .execute(conn)
            .await?;
        Ok(())
    }
}
