//! Elicitation types for MCP submission approval.
//!
//! NOTE: This module is currently unused. MCP protocol-level elicitation
//! is incompatible with stateless Lambda deployment because:
//! 1. NeverSessionManager creates fresh servers with no peer_info
//! 2. OneshotTransport cannot handle bidirectional mid-call communication
//! 3. peer.elicit() always returns CapabilityNotSupported
//!
//! The approval flow is now handled by:
//! - Layer 1: Agent-side AskUserQuestion (instructions + hooks)
//! - Layer 2: Server-side confirmed flag on submit/submit_answer
//!
//! This code is preserved for future migration where stateful
//! sessions and SSE transport would enable real elicitation.

#![allow(dead_code)]

use rmcp::elicit_safe;
use rmcp::model::{CallToolResult, Content};
use rmcp::service::ElicitationError;
use schemars::JsonSchema;
use serde::Deserialize;

/// The user's decision on a submission before it is posted to TokenOverflow.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmissionApproval {
    /// Choose an action for this submission
    pub decision: SubmissionDecision,
}

/// The three choices presented to the user for every submission.
#[derive(Debug, Deserialize, JsonSchema)]
pub enum SubmissionDecision {
    /// Submit the content as-is to TokenOverflow
    Approve,
    /// Discard the submission and move on
    Reject,
    /// Edit the content before submitting (provide corrections in the feedback field)
    FixAndResubmit,
}

elicit_safe!(SubmissionApproval);

/// What the tool should do after processing the elicitation result.
pub enum ElicitAction {
    /// Continue to persist the submission.
    Proceed,
    /// Return an early result to the caller (discard or retry).
    EarlyReturn(CallToolResult),
}

/// Process an elicitation result into a tool action.
///
/// Shared by `submit` and `submit_answer` to avoid duplicating the
/// match logic for all SubmissionDecision and ElicitationError variants.
pub fn process_elicitation(
    result: Result<Option<SubmissionApproval>, ElicitationError>,
    retry_tool_name: &str,
) -> ElicitAction {
    match result {
        Ok(Some(approval)) => {
            match approval.decision {
                SubmissionDecision::Approve => ElicitAction::Proceed,
                SubmissionDecision::Reject => ElicitAction::EarlyReturn(CallToolResult::success(
                    vec![Content::text("Submission discarded by the user.")],
                )),
                SubmissionDecision::FixAndResubmit => {
                    let msg = format!(
                        "The user wants to edit the submission before posting. \
                     Ask the user what changes they want, apply the edits, \
                     and call {} again with the updated content.",
                        retry_tool_name
                    );
                    ElicitAction::EarlyReturn(CallToolResult::success(vec![Content::text(msg)]))
                }
            }
        }
        Ok(None) => ElicitAction::EarlyReturn(CallToolResult::success(vec![Content::text(
            "Submission discarded by the user.",
        )])),
        Err(ElicitationError::CapabilityNotSupported) => {
            // Client does not support elicitation, fall back to direct submission
            ElicitAction::Proceed
        }
        Err(ElicitationError::UserCancelled) | Err(ElicitationError::UserDeclined) => {
            ElicitAction::EarlyReturn(CallToolResult::success(vec![Content::text(
                "Submission discarded by the user.",
            )]))
        }
        Err(_) => {
            // Other elicitation errors: fall back to direct submission
            // to avoid blocking the workflow
            ElicitAction::Proceed
        }
    }
}
