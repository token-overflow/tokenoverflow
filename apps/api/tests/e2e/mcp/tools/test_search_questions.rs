use rmcp::model::CallToolRequestParams;
use serde_json::json;

use super::super::helpers::{create_mcp_client, extract_hint, extract_text, peer};

#[tokio::test]
async fn search_returns_valid_json_array() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("search_questions").with_arguments(
                json!({
                    "query": "How do I handle async errors in Rust code?"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("search should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(text).expect("Should parse as JSON");
    // Verify each result has the expected structure
    for item in &parsed {
        assert!(item["id"].is_string());
        assert!(item["title"].is_string());
    }
}

#[tokio::test]
async fn search_returns_results_after_submit() {
    let client = create_mcp_client().await;

    // Submit a question first
    let submit_result = peer(&client)
        .call_tool(CallToolRequestParams::new("submit").with_arguments(
            json!({
                "title": "How to implement traits in Rust generics?",
                "body": "I need help understanding how to use traits with generic types in Rust.",
                "answer": "You can use trait bounds with the where clause or inline bounds like fn foo<T: MyTrait>(x: T).",
                "tags": ["rust", "generics"]
            })
            .as_object()
            .unwrap()
            .clone(),
        ))
        .await
        .expect("submit should succeed");

    assert!(!submit_result.is_error.unwrap_or(false));

    // Search for it
    let search_result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("search_questions").with_arguments(
                json!({
                    "query": "How do I implement traits with generics in Rust?"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("search should succeed");

    let text = extract_text(&search_result);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(!parsed.is_empty(), "Should return matching results");
}

#[tokio::test]
async fn search_response_includes_hint() {
    let client = create_mcp_client().await;

    let result = peer(&client)
        .call_tool(
            CallToolRequestParams::new("search_questions").with_arguments(
                json!({
                    "query": "How do I handle async errors in Rust code?"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("search should succeed");

    assert_eq!(
        result.content.len(),
        2,
        "Search response should have 2 content items (data + hint)"
    );
    let hint = extract_hint(&result);
    // The hint should contain actionable guidance regardless of results
    assert!(
        hint.contains("submit") || hint.contains("upvote_answer"),
        "Hint should guide the agent to submit or upvote"
    );
}
