use rmcp::model::CallToolRequestParams;
use serde_json::json;

use super::super::helpers::{
    create_mcp_client, create_mcp_voter_client, extract_hint, extract_text, peer,
};

#[tokio::test]
async fn downvote_succeeds_after_submit() {
    let submitter = create_mcp_client().await;
    let voter = create_mcp_voter_client().await;

    // Submit a question to get an answer ID
    let submit_result = peer(&submitter)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP E2E test for downvote flow",
                    "body": "This is the body of the question for downvote testing.",
                    "answer": "This is the answer that will be downvoted in the test.",
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
    let answer_id = submit_parsed["answer_id"].as_str().unwrap();

    // Downvote as a different user
    let downvote_result = peer(&voter)
        .call_tool(
            CallToolRequestParams::new("downvote_answer").with_arguments(
                json!({
                    "answer_id": answer_id
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("downvote should succeed");

    assert!(!downvote_result.is_error.unwrap_or(false));
    let text = extract_text(&downvote_result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert_eq!(parsed["status"], "downvoted");
}

#[tokio::test]
async fn downvote_fails_for_invalid_id() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("downvote_answer").with_arguments(
                json!({
                    "answer_id": "not-a-valid-id"
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
async fn downvote_response_includes_hint() {
    let submitter = create_mcp_client().await;
    let voter = create_mcp_voter_client().await;

    // Submit a question to get an answer ID
    let submit_result = peer(&submitter)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP E2E test for downvote hint",
                    "body": "This is the body of the question for downvote hint test.",
                    "answer": "This is the answer for testing the downvote hint.",
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
    let answer_id = submit_parsed["answer_id"].as_str().unwrap();

    // Downvote as a different user and check hint
    let downvote_result = peer(&voter)
        .call_tool(
            CallToolRequestParams::new("downvote_answer").with_arguments(
                json!({
                    "answer_id": answer_id
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("downvote should succeed");

    assert_eq!(
        downvote_result.content.len(),
        2,
        "Downvote response should have 2 content items (data + hint)"
    );
    let hint = extract_hint(&downvote_result);
    assert!(
        hint.contains("submit_answer"),
        "Downvote hint should mention submit_answer"
    );
}
