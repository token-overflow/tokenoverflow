use std::time::Duration;

use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

use crate::external::embedding::service::EmbeddingError;

const DEFAULT_BASE_URL: &str = "https://api.voyageai.com/v1";

pub struct VoyageClient {
    pub(super) client: reqwest_middleware::ClientWithMiddleware,
    pub(super) base_url: String,
    pub(super) model: String,
    pub(super) output_dimension: u32,
    pub(super) api_key: String,
}

impl VoyageClient {
    /// Create a new Voyage AI client.
    ///
    /// - `base_url`: If provided, uses that endpoint. Defaults to Voyage AI API.
    /// - `model`: The embedding model (e.g., "voyage-code-3").
    /// - `output_dimension`: Desired embedding dimension (e.g., 256).
    /// - `api_key`: Voyage AI API key for authentication.
    pub fn new(
        base_url: Option<&str>,
        model: &str,
        output_dimension: u32,
        api_key: &str,
    ) -> Result<Self, EmbeddingError> {
        let base_url = base_url.unwrap_or(DEFAULT_BASE_URL).to_string();

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(2), Duration::from_secs(8))
            .build_with_max_retries(2); // 2 retries = 3 total attempts

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            client,
            base_url,
            model: model.to_string(),
            output_dimension,
            api_key: api_key.to_string(),
        })
    }
}
