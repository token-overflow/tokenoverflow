use embedding_service::model::OUTPUT_DIMENSION;

use crate::fixtures::{TEST_MODEL, TestServer};

#[tokio::test]
async fn single_input_returns_embedding_with_correct_dimension() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/v1/embeddings", server.base_url))
        .json(&serde_json::json!({
            "input": "test query",
            "model": "voyage-code-3"
        }))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["object"], "list");
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["data"][0]["embedding"].as_array().unwrap().len(),
        OUTPUT_DIMENSION
    );
}

#[tokio::test]
async fn multiple_inputs_returns_multiple_embeddings() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/v1/embeddings", server.base_url))
        .json(&serde_json::json!({
            "input": ["first text", "second text", "third text"],
            "model": "voyage-code-3"
        }))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);

    for (i, item) in data.iter().enumerate() {
        assert_eq!(item["index"], i);
        assert_eq!(
            item["embedding"].as_array().unwrap().len(),
            OUTPUT_DIMENSION
        );
    }
}

#[tokio::test]
async fn response_includes_usage_stats() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/v1/embeddings", server.base_url))
        .json(&serde_json::json!({
            "input": "hello world",
            "model": "test"
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["model"], "voyage-code-3");
    assert!(body["usage"]["total_tokens"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn invalid_json_returns_client_error() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/v1/embeddings", server.base_url))
        .header("content-type", "application/json")
        .body("not valid json")
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn missing_input_field_returns_client_error() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/v1/embeddings", server.base_url))
        .json(&serde_json::json!({"model": "test"}))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/nonexistent", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn concurrent_requests_all_succeed() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let mut set = tokio::task::JoinSet::new();

    for i in 0..5 {
        let client = client.clone();
        let url = format!("{}/v1/embeddings", server.base_url);
        set.spawn(async move {
            client
                .post(&url)
                .json(&serde_json::json!({
                    "input": format!("concurrent text {}", i),
                    "model": "test"
                }))
                .send()
                .await
        });
    }

    while let Some(result) = set.join_next().await {
        let resp = result.unwrap().unwrap();
        assert!(resp.status().is_success());
    }
}
