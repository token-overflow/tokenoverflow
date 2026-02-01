use rmcp::model::CallToolRequestParams;

use super::helpers::{create_mcp_client, peer};

#[tokio::test]
async fn list_tools_returns_all_tools() {
    let client = create_mcp_client().await;
    let tools = peer(&client)
        .list_all_tools()
        .await
        .expect("list_tools should succeed");

    assert_eq!(tools.len(), 5);

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"search_questions"));
    assert!(names.contains(&"submit"));
    assert!(names.contains(&"upvote_answer"));
    assert!(names.contains(&"downvote_answer"));
    assert!(names.contains(&"submit_answer"));
}

#[tokio::test]
async fn list_tools_tools_have_input_schemas() {
    let client = create_mcp_client().await;
    let tools = peer(&client)
        .list_all_tools()
        .await
        .expect("list_tools should succeed");

    for tool in &tools {
        assert!(
            !tool.input_schema.is_empty(),
            "Tool {} should have a non-empty input schema",
            tool.name
        );
    }
}

#[tokio::test]
async fn call_unknown_tool_returns_error() {
    let client = create_mcp_client().await;
    let result = peer(&client)
        .call_tool(CallToolRequestParams::new("nonexistent_tool"))
        .await;

    assert!(result.is_err(), "Should return error for unknown tool");
}
