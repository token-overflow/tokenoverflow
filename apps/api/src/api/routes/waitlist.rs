use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Serialize;

use crate::api::extractors::WorkosJwtClaims;
use crate::api::state::AppState;
use crate::error::AppError;
use crate::services::WaitlistService;

/// Response body for `POST /v1/waitlist`.
///
/// `already_on_waitlist` is true when the row already existed for this
/// `github_id` (the `ON CONFLICT DO NOTHING` no-op path).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AddToWaitlistResponse {
    pub github_id: i64,
    pub github_username: String,
    pub already_on_waitlist: bool,
}

/// POST /v1/waitlist
///
/// The handler resolves the GitHub identity through `AuthService` (DB
/// first then WorkOS Management API), reads the email straight from the
/// validated JWT (via `WorkosJwtClaims`), and inserts idempotently into
/// `api.waitlist`. No `api.users` row is created on this path.
#[utoipa::path(
    post,
    path = "/v1/waitlist",
    tag = "waitlist",
    responses(
        (status = 201, description = "Waitlist entry created or already present", body = AddToWaitlistResponse),
        (status = 401, description = "Missing or invalid bearer token"),
        (status = 500, description = "Internal server error"),
    ),
)]
pub async fn add_to_waitlist(claims: WorkosJwtClaims, State(state): State<AppState>) -> Response {
    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return AppError::Internal(e.to_string()).into_response(),
    };

    match WaitlistService::add(
        state.auth.as_ref(),
        state.users.as_ref(),
        state.waitlist.as_ref(),
        &mut *conn,
        &claims.workos_id,
        &claims.email,
    )
    .await
    {
        Ok(result) => (
            StatusCode::CREATED,
            Json(AddToWaitlistResponse {
                github_id: result.github_id,
                github_username: result.github_username,
                already_on_waitlist: result.already_on_waitlist,
            }),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}
