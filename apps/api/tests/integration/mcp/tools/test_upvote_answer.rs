use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use serde_json::json;
use tokenoverflow::mcp::TokenOverflowServer;

use super::super::helpers::{extract_hint, extract_text, test_context, test_voter_context};

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

#[tokio::test]
async fn upvote_answer_succeeds_for_existing_answer() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for upvote testing",
            "body": "This is the body for testing upvote functionality.",
            "answer": "This is the answer that will be upvoted.",
            "confirmed": true
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let submit_result = server
        .call_tool(submit_request, test_context())
        .await
        .expect("submit should succeed");

    let text = extract_text(&submit_result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    let answer_id = parsed["answer_id"].as_str().unwrap();

    let upvote_request = CallToolRequestParams::new("upvote_answer").with_arguments(
        json!({
            "answer_id": answer_id
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(upvote_request, test_voter_context())
        .await
        .expect("upvote should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert_eq!(parsed["status"], "upvoted");
}

#[tokio::test]
async fn upvote_hint_on_success() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for upvote hint testing",
            "body": "This is the body for testing upvote hint functionality.",
            "answer": "This is the answer for testing upvote hint.",
            "confirmed": true
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let submit_result = server
        .call_tool(submit_request, test_context())
        .await
        .expect("submit should succeed");

    let text = extract_text(&submit_result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    let answer_id = parsed["answer_id"].as_str().unwrap();

    let upvote_request = CallToolRequestParams::new("upvote_answer").with_arguments(
        json!({
            "answer_id": answer_id
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(upvote_request, test_voter_context())
        .await
        .expect("upvote should succeed");

    assert_eq!(
        result.content.len(),
        2,
        "Response should have 2 content items"
    );
    let hint = extract_hint(&result);
    assert!(
        hint.contains("Upvote recorded"),
        "Upvote hint should confirm recording"
    );
}

#[tokio::test]
async fn upvote_answer_fails_for_nonexistent_answer() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("upvote_answer").with_arguments(
        json!({
            "answer_id": "00000000-0000-0000-0000-000000000099"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("should not be protocol error");
    assert_eq!(
        result.is_error,
        Some(true),
        "Should return error for nonexistent answer"
    );
}

#[tokio::test]
async fn upvote_answer_fails_for_invalid_uuid() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("upvote_answer").with_arguments(
        json!({
            "answer_id": "not-a-valid-uuid"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("should not be protocol error");
    assert_eq!(
        result.is_error,
        Some(true),
        "Should return error for invalid UUID string"
    );
}

#[tokio::test]
async fn self_upvote_returns_error() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for self-upvote rejection",
            "body": "This is the body for testing self-upvote rejection.",
            "answer": "This is the answer the author will try to upvote.",
            "confirmed": true
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let submit_result = server
        .call_tool(submit_request, test_context())
        .await
        .expect("submit should succeed");

    let text = extract_text(&submit_result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    let answer_id = parsed["answer_id"].as_str().unwrap();

    let upvote_request = CallToolRequestParams::new("upvote_answer").with_arguments(
        json!({
            "answer_id": answer_id
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(upvote_request, test_context())
        .await
        .expect("should not be protocol error");

    assert_eq!(
        result.is_error,
        Some(true),
        "Should return error when voting on own answer"
    );
}
