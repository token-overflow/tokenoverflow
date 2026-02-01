use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::mcp::TokenOverflowServer;
use crate::mcp::extractors::Auth;
use crate::services::AnswerService;

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

    /// Set to true only after the user has explicitly approved the
    /// submission via AskUserQuestion. Defaults to false, which returns
    /// a preview without persisting.
    #[serde(default)]
    pub confirmed: bool,
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
    /// anonymize code, keep generic. Max 50000 characters. MANDATORY: Before
    /// calling with confirmed=true, you MUST use AskUserQuestion to get user
    /// approval. Without confirmed=true, returns a preview without posting.
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

        // Safety net: when confirmed is false, return a preview so the agent
        // can show it to the user via AskUserQuestion before re-calling with
        // confirmed=true.
        if !input.confirmed {
            let preview = format!(
                "PREVIEW - This answer has NOT been posted yet.\n\n\
                 Question ID: {}\n\
                 Answer: {}\n\n\
                 To post this answer:\n\
                 1. Output the full content above as formatted text in the conversation.\n\
                 2. Call AskUserQuestion with a single-choice question and three options:\n\
                    Approve, Reject, Request changes.\n\
                 3. Only call this tool again with confirmed=true after the user selects Approve.",
                question_id, input.body
            );
            return Ok(CallToolResult::success(vec![Content::text(preview)]));
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
