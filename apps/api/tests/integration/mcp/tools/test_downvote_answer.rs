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
async fn downvote_answer_succeeds_for_existing() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    // Submit a question to get an answer ID
    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for downvote testing",
            "body": "This is the body for testing downvote functionality.",
            "answer": "This is the answer that will be downvoted."
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

    // Downvote the answer as a different user
    let downvote_request = CallToolRequestParams::new("downvote_answer").with_arguments(
        json!({
            "answer_id": answer_id
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(downvote_request, test_voter_context())
        .await
        .expect("downvote should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert_eq!(parsed["status"], "downvoted");
}

#[tokio::test]
async fn downvote_answer_fails_for_invalid_id() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("downvote_answer").with_arguments(
        json!({
            "answer_id": "not-a-valid-id"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("call_tool should succeed");

    assert!(
        result.is_error.unwrap_or(false),
        "Should return isError for invalid ID"
    );
}

#[tokio::test]
async fn downvote_answer_fails_for_nonexistent() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("downvote_answer").with_arguments(
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
        .expect("call_tool should succeed");

    assert!(
        result.is_error.unwrap_or(false),
        "Should return isError for nonexistent answer"
    );
}

#[tokio::test]
async fn downvote_hint_on_success() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    // Submit a question to get an answer ID
    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for downvote hint testing",
            "body": "This is the body for testing downvote hint functionality.",
            "answer": "This is the answer for testing downvote hint."
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

    // Downvote and check hint as a different user
    let downvote_request = CallToolRequestParams::new("downvote_answer").with_arguments(
        json!({
            "answer_id": answer_id
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(downvote_request, test_voter_context())
        .await
        .expect("downvote should succeed");

    assert_eq!(
        result.content.len(),
        2,
        "Response should have 2 content items"
    );
    let hint = extract_hint(&result);
    assert!(
        hint.contains("submit_answer"),
        "Downvote hint should mention submit_answer"
    );
}

#[tokio::test]
async fn downvote_answer_returns_error_when_repo_fails() {
    let db = IntegrationTestDb::new().await;
    let state = common::create_failing_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(state);

    let request = CallToolRequestParams::new("downvote_answer").with_arguments(
        json!({
            "answer_id": "00000000-0000-0000-0000-000000000001"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("call_tool should succeed");

    assert!(
        result.is_error.unwrap_or(false),
        "Should return isError when repo fails"
    );
}

#[tokio::test]
async fn self_downvote_returns_error() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for self-downvote rejection",
            "body": "This is the body for testing self-downvote rejection.",
            "answer": "This is the answer the author will try to downvote."
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

    let downvote_request = CallToolRequestParams::new("downvote_answer").with_arguments(
        json!({
            "answer_id": answer_id
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(downvote_request, test_context())
        .await
        .expect("should not be protocol error");

    assert_eq!(
        result.is_error,
        Some(true),
        "Should return error when voting on own answer"
    );
}
