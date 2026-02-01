use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, HeaderMap, HeaderValue, USER_AGENT};
use tracing::{info, warn};

use crate::types::{
    ApiErrorResponse, ApiResponse, ApiSynonym, ApiTag, StackOverflowSynonym, StackOverflowTag,
};

pub struct StackOverflowClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    access_token: Option<String>,
}

const DEFAULT_BASE_URL: &str = "https://api.stackexchange.com/2.3";

fn build_http_client() -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("tokenoverflow-so-tag-sync/0.1"),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip"));

    reqwest::Client::builder()
        .default_headers(headers)
        .gzip(true)
        .build()
        .expect("failed to build HTTP client")
}

impl StackOverflowClient {
    pub fn new(api_key: Option<String>, access_token: Option<String>) -> Self {
        Self {
            client: build_http_client(),
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key,
            access_token,
        }
    }

    /// Create a client pointing at a custom base URL (for testing).
    pub fn with_base_url(
        base_url: String,
        api_key: Option<String>,
        access_token: Option<String>,
    ) -> Self {
        Self {
            client: build_http_client(),
            base_url,
            api_key,
            access_token,
        }
    }

    /// Append API key to URL query params. Access token is sent via
    /// Authorization header to avoid leaking it in logs.
    fn append_auth(&self, url: &mut String) {
        if let Some(ref key) = self.api_key {
            url.push_str(&format!("&key={}", key));
        }
    }

    pub async fn fetch_all_tags(&self) -> Result<Vec<StackOverflowTag>> {
        let mut all_tags = Vec::new();
        let mut page = 1;

        loop {
            let response = self.fetch_tags_page(page).await?;

            all_tags.extend(response.items.into_iter().map(|t| StackOverflowTag {
                name: t.name,
                count: t.count,
            }));

            if let Some(remaining) = response.quota_remaining {
                info!(
                    "Fetched page {} of tags ({} total, quota remaining: {})",
                    page,
                    all_tags.len(),
                    remaining
                );
            } else {
                info!("Fetched page {} of tags ({} total)", page, all_tags.len());
            }

            if !response.has_more {
                break;
            }

            let delay_ms = response.backoff.unwrap_or(0) * 1000;
            tokio::time::sleep(Duration::from_millis(delay_ms.max(500))).await;

            page += 1;
        }

        Ok(all_tags)
    }

    async fn fetch_tags_page(&self, page: u32) -> Result<ApiResponse<ApiTag>> {
        let mut url = format!(
            "{}/tags?order=desc&sort=popular&site=stackoverflow&pagesize=100&page={}",
            self.base_url, page
        );
        self.append_auth(&mut url);
        self.fetch_with_retry(&url).await
    }

    pub async fn fetch_tags_since(&self, min_timestamp: i64) -> Result<Vec<StackOverflowTag>> {
        let mut all_tags = Vec::new();
        let mut page = 1;

        loop {
            let mut url = format!(
                "{}/tags?order=desc&sort=activity&site=stackoverflow&pagesize=100&page={}&min={}",
                self.base_url, page, min_timestamp
            );
            self.append_auth(&mut url);

            let response: ApiResponse<ApiTag> = self.fetch_with_retry(&url).await?;

            all_tags.extend(response.items.into_iter().map(|t| StackOverflowTag {
                name: t.name,
                count: t.count,
            }));

            info!(
                "Fetched page {} of incremental tags ({} total)",
                page,
                all_tags.len()
            );

            if !response.has_more {
                break;
            }

            let delay_ms = response.backoff.unwrap_or(0) * 1000;
            tokio::time::sleep(Duration::from_millis(delay_ms.max(500))).await;

            page += 1;
        }

        Ok(all_tags)
    }

    pub async fn fetch_all_synonyms(&self) -> Result<Vec<StackOverflowSynonym>> {
        let mut all_synonyms = Vec::new();
        let mut page = 1;

        loop {
            let response = self.fetch_synonyms_page(page).await?;

            all_synonyms.extend(response.items.into_iter().map(|s| StackOverflowSynonym {
                from: s.from_tag,
                to: s.to_tag,
            }));

            info!(
                "Fetched page {} of synonyms ({} total)",
                page,
                all_synonyms.len()
            );

            if !response.has_more {
                break;
            }

            let delay_ms = response.backoff.unwrap_or(0) * 1000;
            tokio::time::sleep(Duration::from_millis(delay_ms.max(500))).await;

            page += 1;
        }

        Ok(all_synonyms)
    }

    async fn fetch_synonyms_page(&self, page: u32) -> Result<ApiResponse<ApiSynonym>> {
        let mut url = format!(
            "{}/tags/synonyms?order=desc&sort=creation&site=stackoverflow&pagesize=100&page={}",
            self.base_url, page
        );
        self.append_auth(&mut url);
        self.fetch_with_retry(&url).await
    }

    pub async fn fetch_synonyms_since(
        &self,
        min_timestamp: i64,
    ) -> Result<Vec<StackOverflowSynonym>> {
        let mut all_synonyms = Vec::new();
        let mut page = 1;

        loop {
            let mut url = format!(
                "{}/tags/synonyms?order=desc&sort=creation&site=stackoverflow&pagesize=100&page={}&min={}",
                self.base_url, page, min_timestamp
            );
            self.append_auth(&mut url);

            let response: ApiResponse<ApiSynonym> = self.fetch_with_retry(&url).await?;

            all_synonyms.extend(response.items.into_iter().map(|s| StackOverflowSynonym {
                from: s.from_tag,
                to: s.to_tag,
            }));

            info!(
                "Fetched page {} of incremental synonyms ({} total)",
                page,
                all_synonyms.len()
            );

            if !response.has_more {
                break;
            }

            let delay_ms = response.backoff.unwrap_or(0) * 1000;
            tokio::time::sleep(Duration::from_millis(delay_ms.max(500))).await;

            page += 1;
        }

        Ok(all_synonyms)
    }

    async fn fetch_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<ApiResponse<T>> {
        let mut attempts = 0;
        let max_retries = 3;

        loop {
            attempts += 1;
            let mut request = self.client.get(url);
            if let Some(ref token) = self.access_token {
                request = request.bearer_auth(token);
            }
            let response = request.send().await.context("HTTP request failed")?;

            let status = response.status();

            if status.is_success() {
                return response
                    .json::<ApiResponse<T>>()
                    .await
                    .context("Failed to parse Stack Overflow API response");
            }

            let error_body = response.text().await.unwrap_or_default();
            let error_detail = serde_json::from_str::<ApiErrorResponse>(&error_body)
                .map(|e| {
                    format!(
                        "{}: {}",
                        e.error_name.unwrap_or_default(),
                        e.error_message.unwrap_or_default()
                    )
                })
                .unwrap_or_else(|_| error_body.chars().take(200).collect());

            if (status.as_u16() == 429 || status.is_server_error()) && attempts < max_retries {
                let backoff = Duration::from_secs(2u64.pow(attempts));
                warn!(
                    "Stack Overflow API returned {} ({}), retrying in {:?} (attempt {}/{})",
                    status, error_detail, backoff, attempts, max_retries
                );
                tokio::time::sleep(backoff).await;
                continue;
            }

            bail!(
                "Stack Overflow API request failed: {} {} (attempt {}/{})\n  URL: {}\n  Error: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or(""),
                attempts,
                max_retries,
                url,
                error_detail
            );
        }
    }
}
