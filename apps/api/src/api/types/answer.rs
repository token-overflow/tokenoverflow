use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::db::models::Answer;

/// Answer in API responses
#[derive(Debug, Serialize)]
pub struct AnswerResponse {
    pub id: Uuid,
    pub body: String,
    pub upvotes: i32,
    pub downvotes: i32,
    pub created_at: DateTime<Utc>,
}

impl From<Answer> for AnswerResponse {
    fn from(answer: Answer) -> Self {
        Self {
            id: answer.id,
            body: answer.body,
            upvotes: answer.upvotes,
            downvotes: answer.downvotes,
            created_at: answer.created_at,
        }
    }
}

/// Request body for POST /v1/questions/{id}/answers
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateAnswerRequest {
    #[validate(length(min = 10, max = 50000))]
    pub body: String,
}

/// Response for POST /v1/answers/{id}/upvote and /downvote
#[derive(Debug, Serialize)]
pub struct VoteResponse {
    pub status: String,
}
