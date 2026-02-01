use http::StatusCode;
use serde_json::Value;

mod common {
    include!("../../../common/mod.rs");
}

// ============================================================================
// GET /health - Health Check
// ============================================================================

#[tokio::test]
async fn health_returns_ok_with_connected_database() {
    let client = common::TestClient::from_config();

    let response = client.get("/health").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    assert_eq!(body["status"], "ok");
    // Must not expose internal details like database status
    assert!(
        body.get("database").is_none(),
        "Response must not contain a 'database' field"
    );
}
