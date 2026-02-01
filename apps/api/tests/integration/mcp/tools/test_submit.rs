use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use serde_json::json;
use tokenoverflow::mcp::TokenOverflowServer;

use super::super::helpers::{extract_hint, extract_text, test_context};

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

#[tokio::test]
async fn submit_validates_title_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

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
        .expect("should not be protocol error");
    assert_eq!(
        result.is_error,
        Some(true),
        "Should return error for short title"
    );
}

#[tokio::test]
async fn submit_validates_body_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "This is a valid title with enough characters",
            "body": "Short",
            "answer": "This is a valid answer with enough characters for testing."
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
        "Should return error for short body"
    );
}

#[tokio::test]
async fn submit_validates_answer_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "This is a valid title with enough characters",
            "body": "This is a valid body with enough characters for testing.",
            "answer": "Short"
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
        "Should return error for short answer"
    );
}

#[tokio::test]
async fn submit_validates_too_many_tags() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question with too many tags",
            "body": "This is the body with enough characters for testing.",
            "answer": "This is the answer with enough characters for testing.",
            "tags": ["a", "b", "c", "d", "e", "f"]
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
        "Should return error for too many tags"
    );
}

#[tokio::test]
async fn submit_validates_empty_tag() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question with empty tag",
            "body": "This is the body with enough characters for testing.",
            "answer": "This is the answer with enough characters for testing.",
            "tags": ["rust", ""]
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
        "Should return error for empty tag"
    );
}

#[tokio::test]
async fn submit_creates_question_and_answer() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question title for MCP submit",
            "body": "This is the body of the test question with enough characters.",
            "answer": "This is the answer to the test question with enough characters.",
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

#[tokio::test]
async fn submit_with_tags_succeeds() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question with tags for MCP",
            "body": "This is the body with enough characters for testing tags.",
            "answer": "This is the answer with enough characters for testing tags.",
            "tags": ["rust", "async", "testing"],
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
}

#[tokio::test]
async fn submit_hint_on_success() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for hint testing",
            "body": "This is the body of the test question for hint testing.",
            "answer": "This is the answer to test the hint response.",
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

    assert_eq!(
        result.content.len(),
        2,
        "Response should have 2 content items"
    );
    let hint = extract_hint(&result);
    assert!(
        hint.contains("submitted to TokenOverflow"),
        "Submit hint should confirm submission"
    );
}
