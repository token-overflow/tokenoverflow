// Unit tests for the elicitation module.
//
// The elicitation module is currently unused in production (see module-level
// doc comment in elicitation.rs) but is preserved for future ECS Fargate
// migration. These tests exercise the pure functions to maintain coverage
// and catch regressions if the code is re-enabled.

use rmcp::service::ElicitationError;
use tokenoverflow::mcp::tools::elicitation::{
    ElicitAction, SubmissionApproval, SubmissionDecision, process_elicitation,
};

#[test]
fn approve_proceeds() {
    let result = Ok(Some(SubmissionApproval {
        decision: SubmissionDecision::Approve,
    }));
    match process_elicitation(result, "submit") {
        ElicitAction::Proceed => {}
        ElicitAction::EarlyReturn(_) => panic!("Approve should proceed, not return early"),
    }
}

#[test]
fn reject_discards() {
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
fn fix_returns_retry_msg_with_tool_name() {
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
fn none_discards() {
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
fn capability_not_supported_falls_back() {
    let result: Result<Option<SubmissionApproval>, ElicitationError> =
        Err(ElicitationError::CapabilityNotSupported);
    match process_elicitation(result, "submit") {
        ElicitAction::Proceed => {}
        ElicitAction::EarlyReturn(_) => {
            panic!("CapabilityNotSupported should proceed, not return early")
        }
    }
}

#[test]
fn cancelled_discards() {
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
fn declined_discards() {
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
fn fix_mentions_correct_tool_name() {
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
