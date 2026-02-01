use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use diesel_async::AsyncConnection;
use http::StatusCode;
use uuid::Uuid;
use validator::Validate;

use crate::api::extractors::AuthenticatedUser;
use crate::api::state::AppState;
use crate::api::types::{CreateAnswerRequest, CreateQuestionRequest};
use crate::error::AppError;
use crate::services::{AnswerService, QuestionService};

/// POST /v1/questions
///
/// Create a new question with an initial answer.
pub async fn create_question(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<CreateQuestionRequest>,
) -> Response {
    if let Err(e) = req.validate() {
        return AppError::from(e).into_response();
    }

    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return AppError::Internal(e.to_string()).into_response(),
    };

    match (*conn)
        .transaction::<_, AppError, _>(|conn| {
            Box::pin(async move {
                QuestionService::create(
                    conn,
                    state.questions.as_ref(),
                    state.tags.as_ref(),
                    state.embedding.as_ref(),
                    &state.tag_resolver,
                    &req.title,
                    &req.body,
                    &req.answer,
                    req.tags.as_deref(),
                    user.id,
                )
                .await
            })
        })
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/questions/{id}
///
/// Get a question with all its answers.
pub async fn get_question(State(state): State<AppState>, Path(id_str): Path<String>) -> Response {
    let id: Uuid = match id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return AppError::Validation("Invalid question ID format".to_string()).into_response();
        }
    };

    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return AppError::Internal(e.to_string()).into_response(),
    };

    match QuestionService::get_by_id(&mut *conn, state.questions.as_ref(), id).await {
        Ok(question) => Json(question).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /v1/questions/{id}/answers
///
/// Add an answer to an existing question.
pub async fn add_answer(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    Json(req): Json<CreateAnswerRequest>,
) -> Response {
    let question_id: Uuid = match id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return AppError::Validation("Invalid question ID format".to_string()).into_response();
        }
    };

    if let Err(e) = req.validate() {
        return AppError::from(e).into_response();
    }

    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return AppError::Internal(e.to_string()).into_response(),
    };

    match AnswerService::create(
        &mut *conn,
        state.answers.as_ref(),
        question_id,
        &req.body,
        user.id,
    )
    .await
    {
        Ok(answer_id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": answer_id })),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}
