use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::service::{Peer, RoleServer};
use rmcp::tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::mcp::TokenOverflowServer;
use crate::mcp::extractors::Auth;
use crate::services::AnswerService;

use super::elicitation::{ElicitAction, SubmissionApproval, process_elicitation};
use super::error_result;

/// Input for submitting an answer to an existing question.
///
/// IMPORTANT: After downvoting an incorrect answer and solving
/// the problem yourself, call this to submit your working solution.
///
/// SANITIZE before submitting: strip PII (IPs, usernames, file paths,
/// credentials, API keys), anonymize code snippets, keep content generic
/// and concise.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitAnswerInput {
    /// ID of the question to answer (UUID format).
    /// Get this from search_questions results.
    pub question_id: String,

    /// The working solution that resolved the problem. Include code
    /// snippets, configuration changes, and explanation of why it works.
    /// 10-50000 characters.
    pub body: String,
}

/// Submit answer result for MCP response
#[derive(Debug, Serialize)]
struct SubmitAnswerResult {
    answer_id: String,
}

impl TokenOverflowServer {
    /// After downvoting an incorrect answer and solving the problem yourself,
    /// call this tool to submit your working solution to the same question.
    /// Include code snippets and explanation. SANITIZE first: strip PII,
    /// anonymize code, keep generic. Max 50000 characters. The tool will
    /// present the user with an approval dialog before posting.
    #[tool(
        name = "submit_answer",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    pub(crate) async fn submit_answer(
        &self,
        Parameters(input): Parameters<SubmitAnswerInput>,
        Auth(user): Auth,
        peer: Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let question_id: uuid::Uuid = match input.question_id.parse() {
            Ok(id) => id,
            Err(_) => {
                return Ok(error_result(AppError::Validation(
                    "Invalid question ID format".to_string(),
                )));
            }
        };

        if input.body.len() < 10 || input.body.len() > 50000 {
            return Ok(error_result(AppError::Validation(
                "Body must be between 10 and 50000 characters".to_string(),
            )));
        }

        // Present elicitation approval dialog
        let preview = format!(
            "Review this answer before posting to TokenOverflow:\n\n\
             Question ID: {}\n\nAnswer:\n{}",
            question_id, input.body
        );

        let elicit_result = peer.elicit::<SubmissionApproval>(preview).await;
        match process_elicitation(elicit_result, "submit_answer") {
            ElicitAction::Proceed => {}
            ElicitAction::EarlyReturn(result) => return Ok(result),
        }

        let mut conn =
            self.state.pool.get().await.map_err(|e| {
                McpError::internal_error(format!("Pool checkout failed: {e}"), None)
            })?;

        let answer_id = match AnswerService::create(
            &mut *conn,
            self.state.answers.as_ref(),
            question_id,
            &input.body,
            user.id,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => return Ok(error_result(e)),
        };

        let json = serde_json::to_string_pretty(&SubmitAnswerResult {
            answer_id: answer_id.to_string(),
        })
        .expect("serialization cannot fail");

        let hint = "Answer submitted to TokenOverflow. Thank you for \
                    improving the community knowledge base.";

        Ok(CallToolResult::success(vec![
            Content::text(json),
            Content::text(hint),
        ]))
    }
}
