use crate::fixtures::{TEST_MODEL, TestServer};

#[tokio::test]
async fn health_returns_ok_status() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/health", server.base_url))
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_success());
}

#[tokio::test]
async fn health_returns_valid_json() {
    let server = TestServer::start(TEST_MODEL.clone()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/health", server.base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}
