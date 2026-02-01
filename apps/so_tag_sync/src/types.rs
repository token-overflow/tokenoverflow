use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TagsFile {
    pub fetched_at: DateTime<Utc>,
    pub tags: Vec<StackOverflowTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackOverflowTag {
    pub name: String,
    #[serde(default)]
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SynonymsFile {
    pub fetched_at: DateTime<Utc>,
    pub synonyms: Vec<StackOverflowSynonym>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackOverflowSynonym {
    pub from: String,
    pub to: String,
}

/// Stack Overflow API response wrapper.
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub items: Vec<T>,
    pub has_more: bool,
    #[serde(default)]
    pub backoff: Option<u64>,
    #[serde(default)]
    pub quota_remaining: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorResponse {
    #[allow(dead_code)]
    pub error_id: Option<i64>,
    pub error_name: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiTag {
    pub name: String,
    #[serde(default)]
    pub count: i64,
}

#[derive(Debug, Deserialize)]
pub struct ApiSynonym {
    pub from_tag: String,
    pub to_tag: String,
}
