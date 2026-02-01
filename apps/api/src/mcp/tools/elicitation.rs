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
