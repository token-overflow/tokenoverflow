use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::models::{NewUser, User};
use crate::db::users;
use crate::error::AppError;
use crate::services::repository::UserRepository;

pub struct PgUserRepository;

#[async_trait]
impl UserRepository<AsyncPgConnection> for PgUserRepository {
    async fn find_by_workos_id(
        &self,
        conn: &mut AsyncPgConnection,
        workos_id: &str,
    ) -> Result<Option<User>, AppError> {
        let user = users::table
            .filter(users::workos_id.eq(workos_id))
            .select(User::as_select())
            .first(conn)
            .await
            .optional()?;

        Ok(user)
    }

    async fn create(
        &self,
        conn: &mut AsyncPgConnection,
        new_user: &NewUser,
    ) -> Result<User, AppError> {
        // ON CONFLICT DO NOTHING returns no rows on conflict, handled below.
        let user = diesel::insert_into(users::table)
            .values(new_user)
            .on_conflict(users::workos_id)
            .do_nothing()
            .get_result::<User>(conn)
            .await
            .optional()?;

        match user {
            Some(user) => Ok(user),
            None => {
                // Concurrent insert won the race; fetch the existing row.
                users::table
                    .filter(users::workos_id.eq(&new_user.workos_id))
                    .select(User::as_select())
                    .first(conn)
                    .await
                    .map_err(Into::into)
            }
        }
    }
}
