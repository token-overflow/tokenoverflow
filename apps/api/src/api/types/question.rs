use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

use crate::api::types::AnswerResponse;

/// Validates each tag is 1-35 characters.
fn validate_tags(tags: &[String]) -> Result<(), ValidationError> {
    for tag in tags {
        if tag.is_empty() || tag.len() > 35 {
            return Err(ValidationError::new("tag_length"));
        }
    }
    Ok(())
}

/// Request body for POST /v1/search
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct SearchRequest {
    #[validate(length(min = 10, max = 10000))]
    pub query: String,

    #[validate(length(max = 5), custom(function = "validate_tags"))]
    pub tags: Option<Vec<String>>,

    #[validate(range(min = 1, max = 10))]
    pub limit: Option<i32>,
}

/// Response for POST /v1/search
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub questions: Vec<SearchResultQuestion>,
}

/// A question in search results with similarity score and answers
#[derive(Debug, Serialize)]
pub struct SearchResultQuestion {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub similarity: f64,
    pub answers: Vec<AnswerResponse>,
}

/// Request body for POST /v1/questions
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateQuestionRequest {
    #[validate(length(min = 10, max = 150))]
    pub title: String,

    #[validate(length(min = 10, max = 1500))]
    pub body: String,

    #[validate(length(min = 10, max = 50000))]
    pub answer: String,

    #[validate(length(max = 5), custom(function = "validate_tags"))]
    pub tags: Option<Vec<String>>,
}

/// Response for POST /v1/questions
#[derive(Debug, Serialize)]
pub struct CreateQuestionResponse {
    pub question_id: Uuid,
    pub answer_id: Uuid,
}

/// Response for GET /v1/questions/{id}
#[derive(Debug, Serialize)]
pub struct QuestionResponse {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Full question with all answers for GET /v1/questions/{id}
#[derive(Debug, Serialize)]
pub struct QuestionWithAnswers {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub answers: Vec<AnswerResponse>,
}
