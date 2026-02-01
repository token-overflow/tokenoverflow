use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use serde_json::json;
use tokenoverflow::mcp::TokenOverflowServer;

use super::super::helpers::{extract_text, test_context};

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

// --- submit: confirmed flag tests ---

#[tokio::test]
async fn submit_preview_when_not_confirmed() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for preview mode",
            "body": "This tests that submit returns a preview when not confirmed.",
            "answer": "The answer should not be persisted without confirmation."
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("call_tool should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    assert!(text.contains("PREVIEW"), "Response should contain PREVIEW");
    assert!(
        text.contains("Test question for preview mode"),
        "Preview should contain the title"
    );
    assert!(
        text.contains("AskUserQuestion"),
        "Preview should instruct agent to use AskUserQuestion"
    );
    // Verify it is NOT a persistence result (no question_id JSON)
    assert!(
        !text.contains("question_id"),
        "Preview should not contain question_id"
    );
}

#[tokio::test]
async fn submit_persists_when_confirmed() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for confirmed submission",
            "body": "This tests that submit persists when confirmed is true.",
            "answer": "The answer is persisted with explicit confirmation.",
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

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["question_id"].is_string());
    assert!(parsed["answer_id"].is_string());
}

// --- submit_answer: confirmed flag tests ---

#[tokio::test]
async fn submit_answer_preview_when_not_confirmed() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    // Submit a question first (with confirmed=true) to get a question_id
    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for submit_answer preview",
            "body": "This question exists so submit_answer can target it.",
            "answer": "Initial answer to the question.",
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
    let question_id = parsed["question_id"].as_str().unwrap();

    // Call submit_answer without confirmed
    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": question_id,
            "body": "This is a better answer that should only be previewed."
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("call_tool should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    assert!(text.contains("PREVIEW"), "Response should contain PREVIEW");
    assert!(
        text.contains(question_id),
        "Preview should contain the question_id"
    );
    assert!(
        text.contains("AskUserQuestion"),
        "Preview should instruct agent to use AskUserQuestion"
    );
}

#[tokio::test]
async fn submit_answer_persists_when_confirmed() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    // Submit a question first
    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for confirmed submit_answer",
            "body": "This question exists so submit_answer can persist to it.",
            "answer": "Initial answer to the question.",
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
    let question_id = parsed["question_id"].as_str().unwrap();

    // Call submit_answer with confirmed=true
    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": question_id,
            "body": "This is a better answer submitted with explicit confirmation.",
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

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["answer_id"].is_string());
}

// --- Validation runs before preview ---

#[tokio::test]
async fn submit_validates_before_preview() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    // confirmed=false with invalid input (title too short)
    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Short",
            "body": "This is a valid body with enough characters for testing.",
            "answer": "This is a valid answer with enough characters for testing."
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("call_tool should succeed");

    // Should return validation error, not preview
    assert!(
        result.is_error.unwrap_or(false),
        "Should return validation error even when confirmed is false"
    );
}

#[tokio::test]
async fn submit_answer_validates_before_preview() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    // confirmed=false with invalid question_id
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

    // Should return validation error, not preview
    assert!(
        result.is_error.unwrap_or(false),
        "Should return validation error even when confirmed is false"
    );
}
