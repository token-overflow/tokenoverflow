use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use serde_json::json;
use tokenoverflow::mcp::TokenOverflowServer;

use super::super::helpers::{extract_hint, extract_text, test_context};

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

/// Helper: submit a question and return its question_id.
async fn submit_question(server: &TokenOverflowServer) -> String {
    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for submit_answer testing",
            "body": "This is the body for testing submit_answer functionality.",
            "answer": "This is the initial answer to the question.",
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
    parsed["question_id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn submit_answer_succeeds_for_existing_question() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let question_id = submit_question(&server).await;

    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": question_id,
            "body": "This is a better answer that actually solves the problem.",
            "confirmed": true
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("submit_answer should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["answer_id"].is_string());
}

#[tokio::test]
async fn submit_answer_fails_for_invalid_question_id() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": "not-a-valid-id",
            "body": "This is a valid answer body with enough characters."
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
        "Should return isError for invalid question ID"
    );
}

#[tokio::test]
async fn submit_answer_fails_for_nonexistent_question() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": "00000000-0000-0000-0000-000000000099",
            "body": "This is a valid answer body with enough characters.",
            "confirmed": true
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
        "Should return isError for nonexistent question"
    );
}

#[tokio::test]
async fn submit_answer_validates_body_too_short() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": "00000000-0000-0000-0000-000000000001",
            "body": "Short"
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
        "Should return isError for body too short"
    );
}

#[tokio::test]
async fn submit_answer_validates_body_too_long() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let long_body = "x".repeat(50001);
    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": "00000000-0000-0000-0000-000000000001",
            "body": long_body
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
        "Should return isError for body too long"
    );
}

#[tokio::test]
async fn submit_answer_hint_on_success() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let question_id = submit_question(&server).await;

    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": question_id,
            "body": "This is the working solution that solves the problem.",
            "confirmed": true
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("submit_answer should succeed");

    assert_eq!(
        result.content.len(),
        2,
        "Response should have 2 content items"
    );
    let hint = extract_hint(&result);
    assert!(
        hint.contains("community knowledge base"),
        "Hint should mention community knowledge base"
    );
}

#[tokio::test]
async fn submit_answer_returns_error_when_repo_fails() {
    let db = IntegrationTestDb::new().await;
    let state = common::create_failing_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(state);

    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": "00000000-0000-0000-0000-000000000001",
            "body": "This is a valid answer body with enough characters.",
            "confirmed": true
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
