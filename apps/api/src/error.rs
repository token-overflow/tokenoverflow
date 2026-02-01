use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Embedding service unavailable: {0}")]
    EmbeddingUnavailable(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Maps a diesel FK violation to `NotFound`; all other diesel errors fall through to `Database`.
pub fn diesel_fk_not_found(
    entity: &str,
    id: impl std::fmt::Display,
    err: diesel::result::Error,
) -> AppError {
    match err {
        diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::ForeignKeyViolation,
            _,
        ) => AppError::NotFound(format!("{} {} not found", entity, id)),
        other => other.into(),
    }
}

// Allow conversion from bb8 pool errors
impl From<diesel_async::pooled_connection::bb8::RunError> for AppError {
    fn from(err: diesel_async::pooled_connection::bb8::RunError) -> Self {
        AppError::Internal(format!("Pool error: {}", err))
    }
}

// Allow conversion from validator errors
impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        // Collect all field errors into a human-readable message
        let messages: Vec<String> = err
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                let error_msgs: Vec<&str> = errors.iter().map(|e| e.code.as_ref()).collect();
                format!("{}: {}", field, error_msgs.join(", "))
            })
            .collect();
        AppError::Validation(messages.join("; "))
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "Forbidden".to_string()),
            AppError::EmbeddingUnavailable(msg) => {
                tracing::error!("Embedding service error: {}", msg);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Embedding service temporarily unavailable".to_string(),
                )
            }
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, axum::Json(ErrorResponse { error: message })).into_response()
    }
}
