#![allow(dead_code)]

// HTTP client for e2e tests - always hits a running API.

use http::StatusCode;
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokenoverflow::config::Config;

use super::test_jwt::generate_test_jwt;

/// HTTP client for e2e tests - always hits a running API.
///
/// Automatically includes a valid Bearer token (as the seed `system` user)
/// so authenticated endpoints work out of the box.
pub struct TestClient {
    client: Client,
    base_url: String,
}

impl TestClient {
    /// Create from configuration file based on TOKENOVERFLOW_ENV.
    pub fn from_config() -> Self {
        let config = Config::load().expect("Failed to load config");
        Self::with_subject(&config.api.base_url, "system")
    }

    /// Create a client authenticated as the `test-voter` user.
    pub fn voter() -> Self {
        let config = Config::load().expect("Failed to load config");
        Self::with_subject(&config.api.base_url, "test-voter")
    }

    pub fn with_subject(base_url: &str, sub: &str) -> Self {
        let token = generate_test_jwt(sub, 3600);
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .expect("Bearer token must be valid header"),
        );

        Self {
            client: Client::builder()
                .default_headers(headers)
                .build()
                .expect("Failed to build HTTP client"),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn get(&self, path: &str) -> TestResponse {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .expect("HTTP request failed");
        TestResponse::from_reqwest(response).await
    }

    pub async fn post<B: Serialize>(&self, path: &str, body: &B) -> TestResponse {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .json(body)
            .send()
            .await
            .expect("HTTP request failed");
        TestResponse::from_reqwest(response).await
    }

    pub async fn post_empty(&self, path: &str) -> TestResponse {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .send()
            .await
            .expect("HTTP request failed");
        TestResponse::from_reqwest(response).await
    }
}

/// Unified response type for test assertions.
pub struct TestResponse {
    status: StatusCode,
    bytes: Vec<u8>,
}

impl TestResponse {
    async fn from_reqwest(response: reqwest::Response) -> Self {
        let status = StatusCode::from_u16(response.status().as_u16()).unwrap();
        let bytes = response
            .bytes()
            .await
            .expect("Failed to read body")
            .to_vec();
        Self { status, bytes }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.bytes).expect("Failed to parse JSON")
    }
}
