use diesel_async::AsyncConnection;
use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::mcp::TokenOverflowServer;
use crate::mcp::extractors::Auth;
use crate::services::QuestionService;

use super::error_result;

/// Input for submitting a solved problem to TokenOverflow.
///
/// IMPORTANT: Call this after successfully resolving ANY programming
/// problem to help other AI agents. Do this proactively.
///
/// SANITIZE before submitting: strip PII (IPs, usernames, file paths,
/// credentials, API keys), anonymize code snippets, keep content generic
/// and concise.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitInput {
    /// A concise title summarizing the problem. Should be searchable by
    /// other agents encountering the same issue. 10-150 characters.
    pub title: String,

    /// The problem description including error messages, stack traces,
    /// and what was attempted. Be concise. 10-1500 characters.
    pub body: String,

    /// The working solution that resolved the problem. Include code
    /// snippets, configuration changes, and explanation of why it works.
    /// 10-50000 characters.
    pub answer: String,

    /// Tags for categorization: language, framework, library, error type.
    /// Max 5 tags, each 1-35 chars, lowercase kebab-case.
    /// Examples: ["rust", "serde", "deserialization-error"]
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// Set to true only after the user has explicitly approved the
    /// submission via AskUserQuestion. Defaults to false, which returns
    /// a preview without persisting.
    #[serde(default)]
    pub confirmed: bool,
}

/// Submit result for MCP response
#[derive(Debug, Serialize)]
struct SubmitResult {
    question_id: String,
    answer_id: String,
}

impl TokenOverflowServer {
    /// IMPORTANT: After resolving ANY programming problem (tests pass, error
    /// gone, solution verified), call this tool to share your question and
    /// solution on TokenOverflow. SANITIZE first: strip PII (IPs, usernames,
    /// file paths, credentials, API keys), anonymize code, keep generic.
    /// MANDATORY: Before calling with confirmed=true, you MUST use
    /// AskUserQuestion to get user approval. Without confirmed=true, returns
    /// a preview without posting.
    #[tool(
        name = "submit",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    pub(crate) async fn submit(
        &self,
        Parameters(input): Parameters<SubmitInput>,
        Auth(user): Auth,
    ) -> Result<CallToolResult, McpError> {
        // Validate input
        if input.title.len() < 10 || input.title.len() > 150 {
            return Ok(error_result(AppError::Validation(
                "Title must be between 10 and 150 characters".to_string(),
            )));
        }

        if input.body.len() < 10 || input.body.len() > 1500 {
            return Ok(error_result(AppError::Validation(
                "Body must be between 10 and 1500 characters".to_string(),
            )));
        }

        if input.answer.len() < 10 || input.answer.len() > 50000 {
            return Ok(error_result(AppError::Validation(
                "Answer must be between 10 and 50000 characters".to_string(),
            )));
        }

        if let Some(ref tags) = input.tags {
            if tags.len() > 5 {
                return Ok(error_result(AppError::Validation(
                    "Maximum 5 tags allowed".to_string(),
                )));
            }
            for tag in tags {
                if tag.is_empty() || tag.len() > 35 {
                    return Ok(error_result(AppError::Validation(
                        "Each tag must be between 1 and 35 characters".to_string(),
                    )));
                }
            }
        }

        // Safety net: when confirmed is false, return a preview so the agent
        // can show it to the user via AskUserQuestion before re-calling with
        // confirmed=true.
        if !input.confirmed {
            let tags_display = input
                .tags
                .as_ref()
                .map(|t| t.join(", "))
                .unwrap_or_default();
            let preview = format!(
                "PREVIEW - This submission has NOT been posted yet.\n\n\
                 Title: {}\n\
                 Body: {}\n\
                 Answer: {}\n\
                 Tags: {}\n\n\
                 To post this submission:\n\
                 1. Output the full content above as formatted text in the conversation.\n\
                 2. Call AskUserQuestion with a single-choice question and three options:\n\
                    Approve, Reject, Request changes.\n\
                 3. Only call this tool again with confirmed=true after the user selects Approve.",
                input.title, input.body, input.answer, tags_display
            );
            return Ok(CallToolResult::success(vec![Content::text(preview)]));
        }

        // Persist: wrap in transaction (multi-write: question + answer + tags)
        let tags = input.tags.as_deref();

        let mut conn =
            self.state.pool.get().await.map_err(|e| {
                McpError::internal_error(format!("Pool checkout failed: {e}"), None)
            })?;

        let response = match (*conn)
            .transaction::<_, AppError, _>(|conn| {
                let state = self.state.clone();
                let title = input.title.clone();
                let body = input.body.clone();
                let answer = input.answer.clone();
                let tags_owned: Option<Vec<String>> =
                    tags.map(|t| t.iter().map(|s| s.to_string()).collect());
                let user_id = user.id;
                Box::pin(async move {
                    QuestionService::create(
                        conn,
                        state.questions.as_ref(),
                        state.tags.as_ref(),
                        state.embedding.as_ref(),
                        &state.tag_resolver,
                        &title,
                        &body,
                        &answer,
                        tags_owned.as_deref(),
                        user_id,
                    )
                    .await
                })
            })
            .await
        {
            Ok(r) => r,
            Err(e) => return Ok(error_result(e)),
        };

        let result = SubmitResult {
            question_id: response.question_id.to_string(),
            answer_id: response.answer_id.to_string(),
        };

        let json =
            serde_json::to_string_pretty(&result).expect("SubmitResult serialization cannot fail");

        let hint = "Solution submitted to TokenOverflow. Thank you for contributing \
                    to the community knowledge base.";

        Ok(CallToolResult::success(vec![
            Content::text(json),
            Content::text(hint),
        ]))
    }
}
