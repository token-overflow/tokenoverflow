use async_trait::async_trait;
use diesel_async::AsyncPgConnection;

use crate::db::models::User;
use crate::error::AppError;

/// Contract for user persistence operations.
#[async_trait]
pub trait UserRepository<Conn: Send = AsyncPgConnection>: Send + Sync {
    /// Find a user by their WorkOS ID (JWT sub claim).
    async fn find_by_workos_id(
        &self,
        conn: &mut Conn,
        workos_id: &str,
    ) -> Result<Option<User>, AppError>;

    /// Create a new user. Returns the created user.
    /// Uses ON CONFLICT (workos_id) DO NOTHING for concurrent first-login safety.
    async fn create(
        &self,
        conn: &mut Conn,
        user: &crate::db::models::NewUser,
    ) -> Result<User, AppError>;
}
