use axum::response::IntoResponse;
use http::StatusCode;
use tokenoverflow::error::{AppError, diesel_fk_not_found};

#[test]
fn validation_error_returns_422() {
    let error = AppError::Validation("Field must be at least 10 characters".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn not_found_error_returns_404() {
    let error = AppError::NotFound("Question not found".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn embedding_unavailable_returns_503() {
    let error = AppError::EmbeddingUnavailable("OpenAI API timeout".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let msg = json["error"].as_str().unwrap();

    // Must NOT leak raw error details
    assert!(
        !msg.contains("OpenAI"),
        "Error message should not contain raw details: {}",
        msg
    );
    assert_eq!(msg, "Embedding service temporarily unavailable");
}

#[test]
fn database_error_returns_500() {
    let diesel_error = diesel::result::Error::NotFound;
    let error = AppError::Database(diesel_error);
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn internal_error_returns_500() {
    let error = AppError::Internal("Unexpected error".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn validator_errors_convert_to_validation_error() {
    use validator::Validate;

    #[derive(Validate)]
    struct TestRequest {
        #[validate(length(min = 10))]
        field: String,
    }

    let req = TestRequest {
        field: "short".to_string(),
    };

    let validation_result = req.validate();
    assert!(validation_result.is_err());

    let error: AppError = validation_result.unwrap_err().into();

    match error {
        AppError::Validation(msg) => {
            assert!(msg.contains("field"), "Should mention the field name");
        }
        _ => panic!("Expected Validation error"),
    }
}

#[test]
fn bb8_run_error_converts_to_internal() {
    use diesel_async::pooled_connection::bb8::RunError;

    let bb8_error: RunError = RunError::TimedOut;
    let error: AppError = bb8_error.into();

    match error {
        AppError::Internal(msg) => {
            assert!(
                msg.contains("Pool error"),
                "Should contain 'Pool error': {}",
                msg
            );
        }
        _ => panic!("Expected Internal error, got {:?}", error),
    }
}

#[test]
fn diesel_fk_not_found_returns_not_found_for_fk_violation() {
    let err = diesel::result::Error::DatabaseError(
        diesel::result::DatabaseErrorKind::ForeignKeyViolation,
        Box::new("fk_questions".to_string()),
    );
    let id = 42i64;
    let result = diesel_fk_not_found("Question", id, err);
    match result {
        AppError::NotFound(msg) => {
            assert!(msg.contains("Question"), "Should mention entity: {}", msg);
            assert!(msg.contains("42"), "Should mention ID: {}", msg);
        }
        _ => panic!("Expected NotFound, got {:?}", result),
    }
}

#[test]
fn diesel_fk_not_found_falls_through_for_other_errors() {
    let err = diesel::result::Error::NotFound;
    let result = diesel_fk_not_found("Answer", 999_999i64, err);
    assert!(
        matches!(result, AppError::Database(_)),
        "Expected Database error, got {:?}",
        result
    );
}

#[test]
fn unauthorized_error_returns_401() {
    let error = AppError::Unauthorized("Invalid token".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unauthorized_error_does_not_leak_details() {
    let error = AppError::Unauthorized("JWT expired at 2024-01-01".to_string());
    let response = error.into_response();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let msg = json["error"].as_str().unwrap();

    // Must NOT leak the specific JWT error
    assert_eq!(msg, "Unauthorized");
    assert!(
        !msg.contains("JWT expired"),
        "Error message should not leak internal details: {}",
        msg
    );
}
