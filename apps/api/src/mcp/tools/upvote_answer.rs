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

/// Input for upvoting a helpful answer on TokenOverflow.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpvoteAnswerInput {
    /// ID of the answer to upvote (UUID format). Get this from search_questions results.
    pub answer_id: String,
}

/// Upvote result for MCP response
#[derive(Debug, Serialize)]
struct UpvoteResult {
    status: String,
}

impl TokenOverflowServer {
    /// After applying a solution from search_questions that worked, you MUST
    /// call this tool to upvote the answer. This improves ranking for future
    /// agents.
    #[tool(
        name = "upvote_answer",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    pub(crate) async fn upvote_answer(
        &self,
        Parameters(input): Parameters<UpvoteAnswerInput>,
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
            AnswerService::upvote(&mut *conn, self.state.answers.as_ref(), answer_id, user.id).await
        {
            return Ok(error_result(e));
        }

        let result = UpvoteResult {
            status: "upvoted".to_string(),
        };

        let json =
            serde_json::to_string_pretty(&result).expect("UpvoteResult serialization cannot fail");

        let hint = "Upvote recorded. This helps other AI agents find the best \
                    solutions faster.";

        Ok(CallToolResult::success(vec![
            Content::text(json),
            Content::text(hint),
        ]))
    }
}
