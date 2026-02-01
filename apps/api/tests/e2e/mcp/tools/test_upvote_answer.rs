use rmcp::model::CallToolRequestParams;
use serde_json::json;

use super::super::helpers::{
    create_mcp_client, create_mcp_voter_client, extract_hint, extract_text, peer,
};

#[tokio::test]
async fn upvote_succeeds_after_submit() {
    let submitter = create_mcp_client().await;
    let voter = create_mcp_voter_client().await;

    // Submit a question to get an answer ID
    let submit_result = peer(&submitter)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP E2E test for upvote flow",
                    "body": "This is the body of the question for upvote testing.",
                    "answer": "This is the answer that will be upvoted in the test."
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

    // Upvote as a different user
    let upvote_result = peer(&voter)
        .call_tool(
            CallToolRequestParams::new("upvote_answer").with_arguments(
                json!({
                    "answer_id": answer_id
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("upvote should succeed");

    assert!(!upvote_result.is_error.unwrap_or(false));
    let text = extract_text(&upvote_result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert_eq!(parsed["status"], "upvoted");
}

#[tokio::test]
async fn upvote_fails_for_invalid_id() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("upvote_answer").with_arguments(
                json!({
                    "answer_id": "not-a-valid-id"
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
            "Should return isError for invalid ID"
        );
    }
}

#[tokio::test]
async fn upvote_fails_for_nonexistent_answer() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("upvote_answer").with_arguments(
                json!({
                    "answer_id": "00000000-0000-0000-0000-000000000099"
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
        "Should return isError for nonexistent answer"
    );
}

#[tokio::test]
async fn upvote_response_includes_hint() {
    let submitter = create_mcp_client().await;
    let voter = create_mcp_voter_client().await;

    // Submit a question to get an answer ID
    let submit_result = peer(&submitter)
        .call_tool(
            CallToolRequestParams::new("submit").with_arguments(
                json!({
                    "title": "MCP E2E test for upvote hint",
                    "body": "This is the body of the question for upvote hint test.",
                    "answer": "This is the answer for testing the upvote hint."
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

    // Upvote as a different user and check hint
    let upvote_result = peer(&voter)
        .call_tool(
            CallToolRequestParams::new("upvote_answer").with_arguments(
                json!({
                    "answer_id": answer_id
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("upvote should succeed");

    assert_eq!(
        upvote_result.content.len(),
        2,
        "Upvote response should have 2 content items (data + hint)"
    );
    let hint = extract_hint(&upvote_result);
    assert!(
        hint.contains("Upvote recorded"),
        "Upvote hint should confirm recording"
    );
}
