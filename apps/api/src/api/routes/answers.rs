use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use crate::api::extractors::AuthenticatedUser;
use crate::api::state::AppState;
use crate::api::types::VoteResponse;
use crate::error::AppError;
use crate::services::AnswerService;

/// Upvote an answer. Idempotent.
pub async fn upvote(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id_str): Path<String>,
) -> Response {
    let answer_id: Uuid = match id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return AppError::Validation("Invalid answer ID format".to_string()).into_response();
        }
    };

    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return AppError::Internal(e.to_string()).into_response(),
    };

    match AnswerService::upvote(&mut *conn, state.answers.as_ref(), answer_id, user.id).await {
        Ok(()) => Json(VoteResponse {
            status: "upvoted".to_string(),
        })
        .into_response(),
        Err(e) => e.into_response(),
    }
}

/// Downvote an answer. Idempotent.
pub async fn downvote(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id_str): Path<String>,
) -> Response {
    let answer_id: Uuid = match id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return AppError::Validation("Invalid answer ID format".to_string()).into_response();
        }
    };

    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return AppError::Internal(e.to_string()).into_response(),
    };

    match AnswerService::downvote(&mut *conn, state.answers.as_ref(), answer_id, user.id).await {
        Ok(()) => Json(VoteResponse {
            status: "downvoted".to_string(),
        })
        .into_response(),
        Err(e) => e.into_response(),
    }
}
