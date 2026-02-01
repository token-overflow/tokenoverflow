use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use validator::Validate;

use crate::api::state::AppState;
use crate::api::types::{SearchRequest, SearchResponse};
use crate::error::AppError;
use crate::services::SearchService;

/// POST /v1/search
///
/// Search for questions using semantic similarity.
pub async fn search(State(state): State<AppState>, Json(req): Json<SearchRequest>) -> Response {
    if let Err(e) = req.validate() {
        return AppError::from(e).into_response();
    }

    let limit = req.limit.unwrap_or(5);
    let tags = req.tags.as_deref();

    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return AppError::Internal(e.to_string()).into_response(),
    };

    // Perform search
    match SearchService::search(
        &mut *conn,
        state.search.as_ref(),
        state.embedding.as_ref(),
        &state.tag_resolver,
        &req.query,
        tags,
        limit,
    )
    .await
    {
        Ok(questions) => Json(SearchResponse { questions }).into_response(),
        Err(e) => e.into_response(),
    }
}
