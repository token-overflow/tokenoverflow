use crate::error::AppError;
use crate::services::AuthService;
use crate::services::repository::{UserRepository, WaitlistRepository};

/// Outcome of `WaitlistService::add` exposed to callers.
#[derive(Debug)]
pub struct WaitlistAddResult {
    pub github_id: i64,
    pub github_username: String,
    pub already_on_waitlist: bool,
}

pub struct WaitlistService;

impl WaitlistService {
    /// Add the workos-authenticated applicant to the waitlist.
    ///
    /// The handler caller has only validated the bearer JWT (via
    /// `WorkosJwtClaims`); no `api.users` row exists yet for fresh
    /// sign-ups. The handler passes `email` straight from the JWT's
    /// `email` claim (sourced from AuthKit's `email` OAuth scope), so
    /// this path performs exactly one WorkOS round-trip via
    /// `resolve_github_identity` and one DB insert.
    pub async fn add<Conn: Send>(
        auth: &AuthService,
        user_repo: &(dyn UserRepository<Conn> + Sync),
        waitlist_repo: &(dyn WaitlistRepository<Conn> + Sync),
        conn: &mut Conn,
        workos_id: &str,
        email: &str,
    ) -> Result<WaitlistAddResult, AppError> {
        let (github_id, github_username) = auth
            .resolve_github_identity(user_repo, conn, workos_id)
            .await?;
        let inserted = waitlist_repo
            .insert(conn, github_id, &github_username, email)
            .await?;
        Ok(WaitlistAddResult {
            github_id,
            github_username,
            already_on_waitlist: inserted.already_existed,
        })
    }
}
