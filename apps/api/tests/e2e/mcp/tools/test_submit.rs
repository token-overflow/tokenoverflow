use rmcp::model::CallToolRequestParams;
use serde_json::json;

use super::super::helpers::{create_mcp_client, extract_hint, extract_text, peer};

#[tokio::test]
async fn submit_creates_question_and_answer() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP integration test question for submit",
                    "body": "This is the body of the integration test question.",
                    "answer": "This is the answer for the integration test question.",
                    "confirmed": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("submit should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["question_id"].is_string());
    assert!(parsed["answer_id"].is_string());
}

#[tokio::test]
async fn submit_rejects_short_title() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "Short",
                    "body": "This is a valid body with enough characters for testing.",
                    "answer": "This is a valid answer with enough characters for testing."
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await;

    // Protocol error is also acceptable.
    if let Ok(r) = result {
        assert!(
            r.is_error.unwrap_or(false),
            "Should return isError for short title"
        );
    }
}

#[tokio::test]
async fn submit_rejects_short_body() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "A valid title for the test question",
                    "body": "Too short",
                    "answer": "This is a valid answer with enough characters for testing."
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await;

    let result = result.expect("should not be protocol error");
    assert_eq!(
        result.is_error,
        Some(true),
        "Should return isError for short body"
    );
}

#[tokio::test]
async fn submit_rejects_short_answer() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "A valid title for the test question",
                    "body": "This is a valid body with enough characters for testing.",
                    "answer": "Too short"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await;

    let result = result.expect("should not be protocol error");
    assert_eq!(
        result.is_error,
        Some(true),
        "Should return isError for short answer"
    );
}

#[tokio::test]
async fn submit_with_tags_succeeds() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP integration test question with tags",
                    "body": "This is the body of the integration test question with tags.",
                    "answer": "This is the answer for the integration test question with tags.",
                    "tags": ["rust"],
                    "confirmed": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("submit with tags should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["question_id"].is_string());
    assert!(parsed["answer_id"].is_string());
}

#[tokio::test]
async fn submit_rejects_too_many_tags() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP integration test question for tag limit",
                    "body": "This is the body of the integration test question for tag limit.",
                    "answer": "This is the answer for the integration test question for tag limit.",
                    "tags": ["tag1", "tag2", "tag3", "tag4", "tag5", "tag6"]
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await;

    let result = result.expect("should not be protocol error");
    assert_eq!(
        result.is_error,
        Some(true),
        "Should return isError for too many tags"
    );
}

#[tokio::test]
async fn submit_response_includes_hint() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP integration test question for hint",
                    "body": "This is the body of the integration test question for hints.",
                    "answer": "This is the answer for the integration test question hints.",
                    "confirmed": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("submit should succeed");

    assert_eq!(
        result.content.len(),
        2,
        "Submit response should have 2 content items (data + hint)"
    );
    let hint = extract_hint(&result);
    assert!(
        hint.contains("submitted to TokenOverflow"),
        "Submit hint should confirm submission"
    );
}
