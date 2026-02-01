use rmcp::model::CallToolRequestParams;
use serde_json::json;

use super::super::helpers::{create_mcp_client, extract_hint, extract_text, peer};

#[tokio::test]
async fn submit_answer_succeeds_after_submit() {
    let client = create_mcp_client().await;

    // Submit a question to get a question_id
    let submit_result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP E2E test for submit_answer flow",
                    "body": "This is the body of the question for submit_answer testing.",
                    "answer": "This is the initial answer to the question.",
                    "confirmed": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("submit should succeed");

    let submit_text = extract_text(&submit_result);
    let submit_parsed: serde_json::Value =
        serde_json::from_str(submit_text).expect("Should parse as JSON");
    let question_id = submit_parsed["question_id"].as_str().unwrap();

    // Submit a better answer
    let answer_result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit_answer").with_arguments(
                json!({
                    "question_id": question_id,
                    "body": "This is a better answer that actually solves the problem.",
                    "confirmed": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("submit_answer should succeed");

    assert!(!answer_result.is_error.unwrap_or(false));
    let text = extract_text(&answer_result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["answer_id"].is_string());

    // Verify the answer_id is a valid UUID
    let answer_id = parsed["answer_id"].as_str().unwrap();
    assert!(
        uuid::Uuid::parse_str(answer_id).is_ok(),
        "answer_id should be a valid UUID"
    );
}

#[tokio::test]
async fn submit_answer_fails_for_invalid_question() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit_answer").with_arguments(
                json!({
                    "question_id": "not-a-valid-id",
                    "body": "This is a valid answer body with enough characters."
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await;

    // With the new error handling, invalid IDs return isError: true
    // via CallToolResult, not a protocol error.
    // Protocol error is also acceptable.
    if let Ok(r) = result {
        assert!(r.is_error.unwrap_or(false), "Should return isError");
    }
}

#[tokio::test]
async fn submit_answer_response_includes_hint() {
    let client = create_mcp_client().await;

    // Submit a question to get a question_id
    let submit_result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP E2E test for submit_answer hint",
                    "body": "This is the body of the question for submit_answer hint test.",
                    "answer": "This is the answer for testing the submit_answer hint.",
                    "confirmed": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("submit should succeed");

    let submit_text = extract_text(&submit_result);
    let submit_parsed: serde_json::Value =
        serde_json::from_str(submit_text).expect("Should parse as JSON");
    let question_id = submit_parsed["question_id"].as_str().unwrap();

    // Submit an answer and check hint
    let answer_result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("submit_answer").with_arguments(
                json!({
                    "question_id": question_id,
                    "body": "This is the working solution for testing the hint.",
                    "confirmed": true
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("submit_answer should succeed");

    assert_eq!(
        answer_result.content.len(),
        2,
        "submit_answer response should have 2 content items (data + hint)"
    );
    let hint = extract_hint(&answer_result);
    assert!(
        hint.contains("community knowledge base"),
        "submit_answer hint should mention community knowledge base"
    );
}
