use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel_async::AsyncPgConnection;
use uuid::Uuid;

use crate::db::models::WaitlistEntry;
use crate::error::AppError;

/// Result of a waitlist insert. `already_existed` is true when the row was
/// already present (the `ON CONFLICT (github_id) DO NOTHING` no-op path).
#[derive(Debug, Clone)]
pub struct WaitlistInsert {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub already_existed: bool,
}

/// Contract for waitlist persistence operations.
#[async_trait]
pub trait WaitlistRepository<Conn: Send = AsyncPgConnection>: Send + Sync {
    /// Insert a waitlist row for the given GitHub identity. If a row already
    /// exists for `github_id`, returns the existing row with
    /// `already_existed = true`.
    async fn insert(
        &self,
        conn: &mut Conn,
        github_id: i64,
        github_username: &str,
        email: &str,
    ) -> Result<WaitlistInsert, AppError>;

    /// Look up a waitlist row by `github_id`. Returns `None` when the
    /// applicant has not yet applied through the BFF. Used by the
    /// `resolve_user` waitlist gate to decide between the
    /// `WaitlistPending`, `WaitlistRequired`, and approved branches.
    async fn find_by_github_id(
        &self,
        conn: &mut Conn,
        github_id: i64,
    ) -> Result<Option<WaitlistEntry>, AppError>;

    /// Stamp `user_id` on the waitlist row identified by `github_id`. Called
    /// from the gate after an approved applicant first signs in and an
    /// `api.users` row is provisioned, so the waitlist row remains as an
    /// audit trail of the rollout.
    async fn set_user_id(
        &self,
        conn: &mut Conn,
        github_id: i64,
        user_id: Uuid,
    ) -> Result<(), AppError>;
}
