// Voyage AI-compatible request/response types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct EmbeddingRequest {
    pub input: EmbeddingInput,
    #[serde(default)]
    #[allow(dead_code)]
    pub model: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub output_dimension: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub output_dtype: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub input_type: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub truncation: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Serialize)]
pub struct EmbeddingResponse {
    pub object: &'static str,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingData {
    pub object: &'static str,
    pub embedding: Vec<f32>,
    pub index: usize,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub total_tokens: u32,
}
