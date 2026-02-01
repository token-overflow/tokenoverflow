use rmcp::handler::server::ServerHandler;
use tokenoverflow::mcp::TokenOverflowServer;
use tokenoverflow::mcp::tools::{
    DownvoteAnswerInput, SearchQuestionsInput, SubmitAnswerInput, SubmitInput, UpvoteAnswerInput,
};

use super::helpers::test_context;

mod common {
    include!("../../common/mod.rs");
}

#[tokio::test]
async fn list_tools_returns_five_tools() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let result = server
        .list_tools(None, test_context())
        .await
        .expect("list_tools should succeed");

    assert_eq!(result.tools.len(), 5);

    let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(tool_names.contains(&"search_questions"));
    assert!(tool_names.contains(&"submit"));
    assert!(tool_names.contains(&"upvote_answer"));
    assert!(tool_names.contains(&"downvote_answer"));
    assert!(tool_names.contains(&"submit_answer"));
}

#[tokio::test]
async fn list_tools_has_descriptions() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let result = server
        .list_tools(None, test_context())
        .await
        .expect("list_tools should succeed");

    for tool in &result.tools {
        assert!(
            tool.description.as_ref().is_some_and(|d| !d.is_empty()),
            "Tool {} should have a description",
            tool.name
        );
    }
}

#[tokio::test]
async fn get_info_returns_server_info() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let info = server.get_info();

    assert!(info.instructions.is_some());
    assert!(info.capabilities.tools.is_some());
}

#[tokio::test]
async fn search_questions_invalid_args_returns_error() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let request = rmcp::model::CallToolRequestParams::new("search_questions").with_arguments(
        serde_json::json!({"query": 123})
            .as_object()
            .unwrap()
            .clone(),
    );

    let result = server.call_tool(request, test_context()).await;
    assert!(result.is_err(), "Should return error for invalid args");
}

#[tokio::test]
async fn submit_invalid_args_returns_error() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let request = rmcp::model::CallToolRequestParams::new("submit").with_arguments(
        serde_json::json!({"title": 123})
            .as_object()
            .unwrap()
            .clone(),
    );

    let result = server.call_tool(request, test_context()).await;
    assert!(result.is_err(), "Should return error for invalid args");
}

#[tokio::test]
async fn upvote_answer_invalid_args_returns_error() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let request = rmcp::model::CallToolRequestParams::new("upvote_answer").with_arguments(
        serde_json::json!({"answer_id": 123})
            .as_object()
            .unwrap()
            .clone(),
    );

    let result = server.call_tool(request, test_context()).await;
    assert!(result.is_err(), "Should return error for invalid args");
}

#[tokio::test]
async fn downvote_answer_invalid_args_returns_error() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let request = rmcp::model::CallToolRequestParams::new("downvote_answer").with_arguments(
        serde_json::json!({"answer_id": 123})
            .as_object()
            .unwrap()
            .clone(),
    );

    let result = server.call_tool(request, test_context()).await;
    assert!(result.is_err(), "Should return error for invalid args");
}

#[tokio::test]
async fn submit_answer_invalid_args_returns_error() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let request = rmcp::model::CallToolRequestParams::new("submit_answer").with_arguments(
        serde_json::json!({"question_id": 123})
            .as_object()
            .unwrap()
            .clone(),
    );

    let result = server.call_tool(request, test_context()).await;
    assert!(result.is_err(), "Should return error for invalid args");
}

// --- Instructions ---

#[tokio::test]
async fn instructions_contain_search_first_rule() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let info = server.get_info();
    let instructions = info.instructions.expect("instructions should be set");

    assert!(
        instructions.contains("search TokenOverflow FIRST"),
        "Instructions must contain 'search TokenOverflow FIRST'"
    );
    assert!(
        instructions.contains("CRITICAL"),
        "Instructions must contain 'CRITICAL'"
    );
}

#[tokio::test]
async fn instructions_contain_submit_solutions_rule() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let info = server.get_info();
    let instructions = info.instructions.expect("instructions should be set");

    assert!(
        instructions.contains("submit solutions"),
        "Instructions must contain 'submit solutions'"
    );
}

#[tokio::test]
async fn instructions_contain_upvote_rule() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let info = server.get_info();
    let instructions = info.instructions.expect("instructions should be set");

    assert!(
        instructions.contains("upvote helpful answers"),
        "Instructions must contain 'upvote helpful answers'"
    );
}

#[tokio::test]
async fn instructions_contain_downvote_rule() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let info = server.get_info();
    let instructions = info.instructions.expect("instructions should be set");

    assert!(
        instructions.contains("downvote failing answers"),
        "Instructions must contain 'downvote failing answers'"
    );
}

#[tokio::test]
async fn instructions_contain_submit_answer_rule() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let info = server.get_info();
    let instructions = info.instructions.expect("instructions should be set");

    assert!(
        instructions.contains("submit better answers"),
        "Instructions must contain 'submit better answers'"
    );
}

#[tokio::test]
async fn instructions_contain_tokenoverflow_url() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let info = server.get_info();
    let instructions = info.instructions.expect("instructions should be set");

    assert!(
        instructions.contains("TokenOverflow"),
        "Instructions must mention TokenOverflow"
    );
}

// --- Tool descriptions ---

#[tokio::test]
async fn search_tool_description_is_prescriptive() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let result = server
        .list_tools(None, test_context())
        .await
        .expect("list_tools should succeed");

    let search_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "search_questions")
        .expect("search_questions tool should exist");

    let desc = search_tool
        .description
        .as_ref()
        .expect("should have description");

    assert!(
        desc.contains("CRITICAL"),
        "Description must contain 'CRITICAL'"
    );
    assert!(desc.contains("MUST"), "Description must contain 'MUST'");
    assert!(desc.contains("BEFORE"), "Description must contain 'BEFORE'");
}

#[tokio::test]
async fn submit_tool_description_is_prescriptive() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let result = server
        .list_tools(None, test_context())
        .await
        .expect("list_tools should succeed");

    let submit_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "submit")
        .expect("submit tool should exist");

    let desc = submit_tool
        .description
        .as_ref()
        .expect("should have description");

    assert!(
        desc.contains("IMPORTANT"),
        "Description must contain 'IMPORTANT'"
    );
    assert!(
        desc.contains("SANITIZE"),
        "Description must contain 'SANITIZE'"
    );
}

#[tokio::test]
async fn upvote_tool_description_is_prescriptive() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let result = server
        .list_tools(None, test_context())
        .await
        .expect("list_tools should succeed");

    let upvote_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "upvote_answer")
        .expect("upvote_answer tool should exist");

    let desc = upvote_tool
        .description
        .as_ref()
        .expect("should have description");

    assert!(desc.contains("MUST"), "Description must contain 'MUST'");
}

#[tokio::test]
async fn downvote_tool_description_is_prescriptive() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let result = server
        .list_tools(None, test_context())
        .await
        .expect("list_tools should succeed");

    let downvote_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "downvote_answer")
        .expect("downvote_answer tool should exist");

    let desc = downvote_tool
        .description
        .as_ref()
        .expect("should have description");

    assert!(desc.contains("MUST"), "Description must contain 'MUST'");
}

#[tokio::test]
async fn submit_answer_tool_description_is_prescriptive() {
    let app_state = common::create_mock_app_state();
    let server = TokenOverflowServer::new(app_state);

    let result = server
        .list_tools(None, test_context())
        .await
        .expect("list_tools should succeed");

    let submit_answer_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "submit_answer")
        .expect("submit_answer tool should exist");

    let desc = submit_answer_tool
        .description
        .as_ref()
        .expect("should have description");

    assert!(
        desc.contains("SANITIZE"),
        "Description must mention 'SANITIZE'"
    );
}

// --- Schema field descriptions ---

#[test]
fn search_schema_has_enhanced_descriptions() {
    let schema = schemars::schema_for!(SearchQuestionsInput);
    let value = schema.to_value();

    let properties = value["properties"]
        .as_object()
        .expect("should have properties");

    let query_desc = properties["query"]["description"]
        .as_str()
        .expect("query should have description");
    assert!(
        query_desc.contains("error message"),
        "Query description should mention error messages"
    );

    let tags_desc = properties["tags"]["description"]
        .as_str()
        .expect("tags should have description");
    assert!(
        tags_desc.contains("framework"),
        "Tags description should mention framework"
    );

    let limit_desc = properties["limit"]["description"]
        .as_str()
        .expect("limit should have description");
    assert!(
        limit_desc.contains("ambiguous"),
        "Limit description should mention ambiguous problems"
    );
}

#[test]
fn search_schema_has_enhanced_struct_description() {
    let schema = schemars::schema_for!(SearchQuestionsInput);
    let value = schema.to_value();

    let desc = value["description"]
        .as_str()
        .expect("schema should have top-level description");
    assert!(
        desc.contains("CRITICAL"),
        "Struct description should contain 'CRITICAL'"
    );
}

#[test]
fn submit_schema_has_enhanced_descriptions() {
    let schema = schemars::schema_for!(SubmitInput);
    let value = schema.to_value();

    let properties = value["properties"]
        .as_object()
        .expect("should have properties");

    let title_desc = properties["title"]["description"]
        .as_str()
        .expect("title should have description");
    assert!(
        title_desc.contains("searchable"),
        "Title description should mention searchability"
    );

    let body_desc = properties["body"]["description"]
        .as_str()
        .expect("body should have description");
    assert!(
        body_desc.contains("error messages"),
        "Body description should mention error messages"
    );

    let answer_desc = properties["answer"]["description"]
        .as_str()
        .expect("answer should have description");
    assert!(
        answer_desc.contains("working solution"),
        "Answer description should mention working solution"
    );

    let tags_desc = properties["tags"]["description"]
        .as_str()
        .expect("tags should have description");
    assert!(
        tags_desc.contains("categorization"),
        "Tags description should mention categorization"
    );
}

#[test]
fn submit_schema_has_enhanced_struct_description() {
    let schema = schemars::schema_for!(SubmitInput);
    let value = schema.to_value();

    let desc = value["description"]
        .as_str()
        .expect("schema should have top-level description");
    assert!(
        desc.contains("IMPORTANT"),
        "Struct description should contain 'IMPORTANT'"
    );
}

#[test]
fn upvote_schema_has_enhanced_descriptions() {
    let schema = schemars::schema_for!(UpvoteAnswerInput);
    let value = schema.to_value();

    let properties = value["properties"]
        .as_object()
        .expect("should have properties");

    let answer_id_desc = properties["answer_id"]["description"]
        .as_str()
        .expect("answer_id should have description");
    assert!(
        answer_id_desc.contains("search_questions"),
        "answer_id description should mention search_questions"
    );
}

#[test]
fn downvote_schema_has_enhanced_descriptions() {
    let schema = schemars::schema_for!(DownvoteAnswerInput);
    let value = schema.to_value();

    let properties = value["properties"]
        .as_object()
        .expect("should have properties");

    let answer_id_desc = properties["answer_id"]["description"]
        .as_str()
        .expect("answer_id should have description");
    assert!(
        answer_id_desc.contains("search_questions"),
        "answer_id description should mention search_questions"
    );
}

#[test]
fn submit_answer_schema_has_enhanced_descriptions() {
    let schema = schemars::schema_for!(SubmitAnswerInput);
    let value = schema.to_value();

    let properties = value["properties"]
        .as_object()
        .expect("should have properties");

    let question_id_desc = properties["question_id"]["description"]
        .as_str()
        .expect("question_id should have description");
    assert!(
        question_id_desc.contains("search_questions"),
        "question_id description should mention search_questions"
    );

    let body_desc = properties["body"]["description"]
        .as_str()
        .expect("body should have description");
    assert!(
        body_desc.contains("working solution"),
        "body description should mention working solution"
    );
}
