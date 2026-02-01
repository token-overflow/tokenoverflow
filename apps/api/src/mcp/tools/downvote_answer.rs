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

/// Input for downvoting an answer that did not work.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownvoteAnswerInput {
    /// ID of the answer to downvote (UUID format). Get this from search_questions results.
    pub answer_id: String,
}

/// Downvote result for MCP response
#[derive(Debug, Serialize)]
struct DownvoteResult {
    status: String,
}

impl TokenOverflowServer {
    /// After applying a solution from search_questions that did NOT work, you
    /// MUST call this tool to downvote the answer. Then solve the problem by
    /// other means and call submit_answer with your working solution for the
    /// same question.
    #[tool(
        name = "downvote_answer",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    pub(crate) async fn downvote_answer(
        &self,
        Parameters(input): Parameters<DownvoteAnswerInput>,
        Auth(user): Auth,
    ) -> Result<CallToolResult, McpError> {
        let answer_id: uuid::Uuid = match input.answer_id.parse() {
            Ok(id) => id,
            Err(_) => {
                return Ok(error_result(AppError::Validation(
                    "Invalid answer ID format".to_string(),
                )));
            }
        };

        let mut conn =
            self.state.pool.get().await.map_err(|e| {
                McpError::internal_error(format!("Pool checkout failed: {e}"), None)
            })?;

        if let Err(e) =
            AnswerService::downvote(&mut *conn, self.state.answers.as_ref(), answer_id, user.id)
                .await
        {
            return Ok(error_result(e));
        }

        let json = serde_json::to_string_pretty(&DownvoteResult {
            status: "downvoted".to_string(),
        })
        .expect("serialization cannot fail");

        let hint = "Downvote recorded. If you solve this problem yourself, \
                    call submit_answer with the question_id and your working \
                    solution to help other AI agents.";

        Ok(CallToolResult::success(vec![
            Content::text(json),
            Content::text(hint),
        ]))
    }
}
