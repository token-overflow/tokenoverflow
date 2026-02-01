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
async fn search_questions_validates_query_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("search_questions").with_arguments(
        json!({
            "query": "short"
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
        "Should return error for short query"
    );
}

#[tokio::test]
async fn search_questions_validates_max_query_length() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let long_query = "a".repeat(10001);
    let request = CallToolRequestParams::new("search_questions").with_arguments(
        json!({
            "query": long_query
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
        "Should return error for query exceeding max length"
    );
}

#[tokio::test]
async fn search_questions_returns_error_when_repo_fails() {
    let db = IntegrationTestDb::new().await;
    let state = common::create_failing_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(state);

    let request = CallToolRequestParams::new("search_questions").with_arguments(
        json!({
            "query": "How do I handle errors in async Rust code?"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("should not be protocol error");
    assert_eq!(result.is_error, Some(true), "Should propagate repo failure");
}

#[tokio::test]
async fn submit_returns_error_when_repo_fails() {
    let db = IntegrationTestDb::new().await;
    let state = common::create_failing_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question triggering repo failure",
            "body": "This body is long enough to pass validation checks.",
            "answer": "This answer is long enough to pass validation checks."
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("should not be protocol error");
    assert_eq!(result.is_error, Some(true), "Should propagate repo failure");
}

#[tokio::test]
async fn upvote_answer_fails_for_invalid_id() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("upvote_answer").with_arguments(
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
        .expect("should not be protocol error");
    assert_eq!(
        result.is_error,
        Some(true),
        "Should return error for invalid ID"
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
async fn unknown_tool_returns_error() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("unknown_tool");

    let result = server.call_tool(request, test_context()).await;
    assert!(
        result.is_err(),
        "Should return protocol error for unknown tool"
    );
}

#[tokio::test]
async fn search_questions_returns_empty_when_no_questions() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("search_questions").with_arguments(
        json!({
            "query": "How do I handle errors in async Rust code?"
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
    let parsed: Vec<serde_json::Value> = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed.is_empty());
}

#[tokio::test]
async fn search_questions_returns_results_when_questions_exist() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "How to handle errors in async Rust?",
            "body": "I'm struggling with error handling in async functions. What's the best approach?",
            "answer": "Use the ? operator with Result types. You can also use anyhow for application code."
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    server
        .call_tool(submit_request, test_context())
        .await
        .expect("submit should succeed");

    let search_request = CallToolRequestParams::new("search_questions").with_arguments(
        json!({
            "query": "How do I handle errors in async Rust code?"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(search_request, test_context())
        .await
        .expect("search should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(!parsed.is_empty(), "Should return matching questions");
}

#[tokio::test]
async fn search_hint_when_no_results() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("search_questions").with_arguments(
        json!({
            "query": "How do I handle errors in async Rust code?"
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
        hint.contains("submit"),
        "No-results hint should mention submit"
    );
}
