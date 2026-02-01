use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::service::ElicitationError;
use serde_json::json;
use tokenoverflow::mcp::TokenOverflowServer;
use tokenoverflow::mcp::tools::elicitation::{
    ElicitAction, SubmissionApproval, SubmissionDecision, process_elicitation,
};

use super::super::helpers::{extract_hint, extract_text, test_context};

use crate::test_db::IntegrationTestDb;

mod common {
    include!("../../../common/mod.rs");
}

// --- Direct tests for process_elicitation ---

#[test]
fn submit_elicit_approve_proceeds() {
    let result = Ok(Some(SubmissionApproval {
        decision: SubmissionDecision::Approve,
    }));
    match process_elicitation(result, "submit") {
        ElicitAction::Proceed => {} // expected
        ElicitAction::EarlyReturn(_) => panic!("Approve should proceed, not return early"),
    }
}

#[test]
fn submit_elicit_reject_discards() {
    let result = Ok(Some(SubmissionApproval {
        decision: SubmissionDecision::Reject,
    }));
    match process_elicitation(result, "submit") {
        ElicitAction::EarlyReturn(r) => {
            let text = &r.content[0].as_text().expect("should be text").text;
            assert!(text.contains("discarded"));
        }
        ElicitAction::Proceed => panic!("Reject should return early, not proceed"),
    }
}

#[test]
fn submit_elicit_fix_returns_retry_msg() {
    let result = Ok(Some(SubmissionApproval {
        decision: SubmissionDecision::FixAndResubmit,
    }));
    match process_elicitation(result, "submit") {
        ElicitAction::EarlyReturn(r) => {
            let text = &r.content[0].as_text().expect("should be text").text;
            assert!(text.contains("call submit again"));
        }
        ElicitAction::Proceed => panic!("FixAndResubmit should return early"),
    }
}

#[test]
fn submit_elicit_none_discards() {
    let result: Result<Option<SubmissionApproval>, ElicitationError> = Ok(None);
    match process_elicitation(result, "submit") {
        ElicitAction::EarlyReturn(r) => {
            let text = &r.content[0].as_text().expect("should be text").text;
            assert!(text.contains("discarded"));
        }
        ElicitAction::Proceed => panic!("None should return early"),
    }
}

#[test]
fn submit_elicit_not_supported_falls_back() {
    let result: Result<Option<SubmissionApproval>, ElicitationError> =
        Err(ElicitationError::CapabilityNotSupported);
    match process_elicitation(result, "submit") {
        ElicitAction::Proceed => {} // expected: fall back to direct submission
        ElicitAction::EarlyReturn(_) => {
            panic!("CapabilityNotSupported should proceed, not return early")
        }
    }
}

#[test]
fn submit_elicit_cancelled_discards() {
    let result: Result<Option<SubmissionApproval>, ElicitationError> =
        Err(ElicitationError::UserCancelled);
    match process_elicitation(result, "submit") {
        ElicitAction::EarlyReturn(r) => {
            let text = &r.content[0].as_text().expect("should be text").text;
            assert!(text.contains("discarded"));
        }
        ElicitAction::Proceed => panic!("UserCancelled should return early"),
    }
}

#[test]
fn submit_elicit_declined_discards() {
    let result: Result<Option<SubmissionApproval>, ElicitationError> =
        Err(ElicitationError::UserDeclined);
    match process_elicitation(result, "submit") {
        ElicitAction::EarlyReturn(r) => {
            let text = &r.content[0].as_text().expect("should be text").text;
            assert!(text.contains("discarded"));
        }
        ElicitAction::Proceed => panic!("UserDeclined should return early"),
    }
}

#[test]
fn submit_answer_elicit_fix_mentions_correct_tool() {
    let result = Ok(Some(SubmissionApproval {
        decision: SubmissionDecision::FixAndResubmit,
    }));
    match process_elicitation(result, "submit_answer") {
        ElicitAction::EarlyReturn(r) => {
            let text = &r.content[0].as_text().expect("should be text").text;
            assert!(
                text.contains("call submit_answer again"),
                "Retry message should reference submit_answer"
            );
        }
        ElicitAction::Proceed => panic!("FixAndResubmit should return early"),
    }
}

// --- Integration tests via call_tool (CapabilityNotSupported fallback) ---

#[tokio::test]
async fn submit_persists_when_elicitation_not_supported() {
    let db = IntegrationTestDb::new().await;
    let app_state = common::create_mock_app_state_with_pool(db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    let request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for elicitation fallback",
            "body": "This tests that submit succeeds when elicitation is not supported.",
            "answer": "The answer is persisted directly without user approval."
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("call_tool should succeed");

    // Should persist successfully since CapabilityNotSupported falls through
    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["question_id"].is_string());
    assert!(parsed["answer_id"].is_string());
}

#[tokio::test]
async fn submit_answer_persists_when_elicitation_not_supported() {
    let db = IntegrationTestDb::new().await;
    let store = common::MockStore::new();
    let app_state = common::create_app_state_with_store_and_pool(&store, db.pool().clone());
    let server = TokenOverflowServer::new(app_state);

    // Submit a question first
    let submit_request = CallToolRequestParams::new("submit").with_arguments(
        json!({
            "title": "Test question for submit_answer elicitation fallback",
            "body": "This question exists so submit_answer can target it.",
            "answer": "Initial answer to the question."
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

    let request = CallToolRequestParams::new("submit_answer").with_arguments(
        json!({
            "question_id": question_id,
            "body": "This is a better answer submitted without elicitation support."
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server
        .call_tool(request, test_context())
        .await
        .expect("submit_answer should succeed");

    assert!(!result.is_error.unwrap_or(false));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Should parse as JSON");
    assert!(parsed["answer_id"].is_string());
    let hint = extract_hint(&result);
    assert!(hint.contains("community knowledge base"));
}
