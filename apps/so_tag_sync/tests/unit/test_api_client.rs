use serde_json::json;
use so_tag_sync::api_client::StackOverflowClient;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(base_url: &str) -> StackOverflowClient {
    StackOverflowClient::with_base_url(base_url.to_string(), None, None)
}

fn client_with_auth(base_url: &str) -> StackOverflowClient {
    StackOverflowClient::with_base_url(
        base_url.to_string(),
        Some("test-key".to_string()),
        Some("test-token".to_string()),
    )
}

// ============================================================================
// fetch_all_tags
// ============================================================================

#[tokio::test]
async fn fetch_all_tags_single_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {"name": "rust", "count": 50000},
                {"name": "python", "count": 2000000}
            ],
            "has_more": false,
            "quota_remaining": 299
        })))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "rust");
    assert_eq!(tags[0].count, 50000);
    assert_eq!(tags[1].name, "python");
}

#[tokio::test]
async fn fetch_all_tags_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "rust", "count": 50000}],
            "has_more": true,
            "backoff": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "python", "count": 2000000}],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "rust");
    assert_eq!(tags[1].name, "python");
}

#[tokio::test]
async fn fetch_all_tags_no_quota_remaining() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "go", "count": 100}],
            "has_more": false
        })))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "go");
}

// ============================================================================
// fetch_tags_since
// ============================================================================

#[tokio::test]
async fn fetch_tags_since_single_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("min", "1700000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "svelte", "count": 5000}],
            "has_more": false
        })))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_tags_since(1700000000).await.unwrap();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "svelte");
}

#[tokio::test]
async fn fetch_tags_since_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("page", "1"))
        .and(query_param("min", "1700000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "a", "count": 1}],
            "has_more": true,
            "backoff": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("page", "2"))
        .and(query_param("min", "1700000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "b", "count": 2}],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_tags_since(1700000000).await.unwrap();

    assert_eq!(tags.len(), 2);
}

// ============================================================================
// fetch_all_synonyms
// ============================================================================

#[tokio::test]
async fn fetch_all_synonyms_single_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags/synonyms"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {"from_tag": "js", "to_tag": "javascript"},
                {"from_tag": "py", "to_tag": "python"}
            ],
            "has_more": false
        })))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let synonyms = client.fetch_all_synonyms().await.unwrap();

    assert_eq!(synonyms.len(), 2);
    assert_eq!(synonyms[0].from, "js");
    assert_eq!(synonyms[0].to, "javascript");
    assert_eq!(synonyms[1].from, "py");
    assert_eq!(synonyms[1].to, "python");
}

#[tokio::test]
async fn fetch_all_synonyms_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags/synonyms"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"from_tag": "js", "to_tag": "javascript"}],
            "has_more": true,
            "backoff": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tags/synonyms"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"from_tag": "ts", "to_tag": "typescript"}],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let synonyms = client.fetch_all_synonyms().await.unwrap();

    assert_eq!(synonyms.len(), 2);
}

// ============================================================================
// fetch_synonyms_since
// ============================================================================

#[tokio::test]
async fn fetch_synonyms_since_single_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags/synonyms"))
        .and(query_param("min", "1700000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"from_tag": "k8s", "to_tag": "kubernetes"}],
            "has_more": false
        })))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let synonyms = client.fetch_synonyms_since(1700000000).await.unwrap();

    assert_eq!(synonyms.len(), 1);
    assert_eq!(synonyms[0].from, "k8s");
}

#[tokio::test]
async fn fetch_synonyms_since_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags/synonyms"))
        .and(query_param("page", "1"))
        .and(query_param("min", "1700000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"from_tag": "k8s", "to_tag": "kubernetes"}],
            "has_more": true,
            "backoff": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tags/synonyms"))
        .and(query_param("page", "2"))
        .and(query_param("min", "1700000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"from_tag": "golang", "to_tag": "go"}],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let synonyms = client.fetch_synonyms_since(1700000000).await.unwrap();

    assert_eq!(synonyms.len(), 2);
}

// ============================================================================
// Auth parameters
// ============================================================================

#[tokio::test]
async fn api_key_appended_as_query_param() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_with_auth(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert!(tags.is_empty());
}

#[tokio::test]
async fn access_token_sent_as_bearer_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_with_auth(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert!(tags.is_empty());
}

// ============================================================================
// HTTP headers
// ============================================================================

#[tokio::test]
async fn sends_user_agent_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("User-Agent", "tokenoverflow-so-tag-sync/0.1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert!(tags.is_empty());
}

#[tokio::test]
async fn sends_accept_json_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("Accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert!(tags.is_empty());
}

#[tokio::test]
async fn sends_accept_encoding_gzip_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("Accept-Encoding", "gzip"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert!(tags.is_empty());
}

// ============================================================================
// Retry and error handling
// ============================================================================

#[tokio::test]
async fn retries_on_429() {
    let server = MockServer::start().await;

    // First call returns 429, second succeeds
    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error_id": 502,
            "error_name": "throttle_violation",
            "error_message": "too many requests"
        })))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "rust", "count": 1}],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert_eq!(tags.len(), 1);
}

#[tokio::test]
async fn retries_on_500() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"name": "go", "count": 2}],
            "has_more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert_eq!(tags.len(), 1);
}

#[tokio::test]
async fn fails_after_max_retries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error_id": 500,
            "error_name": "internal_error",
            "error_message": "something broke"
        })))
        .expect(3)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let result = client.fetch_all_tags().await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("500"), "Error should mention status code");
}

#[tokio::test]
async fn non_retryable_error_fails_immediately() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error_id": 400,
            "error_name": "bad_parameter",
            "error_message": "invalid sort"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let result = client.fetch_all_tags().await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("400"), "Error should mention status code");
}

#[tokio::test]
async fn error_with_non_json_body() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden by WAF"))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let result = client.fetch_all_tags().await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Forbidden by WAF"));
}

#[tokio::test]
async fn empty_items_returns_empty_vec() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "has_more": false
        })))
        .mount(&server)
        .await;

    let client = client(&server.uri());
    let tags = client.fetch_all_tags().await.unwrap();

    assert!(tags.is_empty());
}

// ============================================================================
// Constructor
// ============================================================================

#[test]
fn new_creates_default_client() {
    let client = StackOverflowClient::new(None, None);
    // Verify it doesn't panic — the base_url is set internally
    drop(client);
}

#[test]
fn with_base_url_sets_custom_url() {
    let client =
        StackOverflowClient::with_base_url("http://localhost:1234".to_string(), None, None);
    drop(client);
}
