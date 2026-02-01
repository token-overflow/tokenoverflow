use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::ServerCapabilities;
use rmcp::tool_handler;
use std::sync::Arc;

use crate::api::state::AppState;

/// MCP Server implementation for TokenOverflow
#[derive(Clone)]
pub struct TokenOverflowServer {
    pub(crate) state: Arc<AppState>,
    tool_router: ToolRouter<Self>,
}

impl TokenOverflowServer {
    pub fn new(state: AppState) -> Self {
        Self {
            state: Arc::new(state),
            tool_router: Self::tool_router(),
        }
    }

    /// Build the tool router by combining all tool routes from separate files.
    /// Each #[tool]-annotated method generates a companion `_tool_attr()` fn.
    fn tool_router() -> ToolRouter<Self> {
        ToolRouter::new()
            .with_route((Self::search_questions_tool_attr(), Self::search_questions))
            .with_route((Self::submit_tool_attr(), Self::submit))
            .with_route((Self::upvote_answer_tool_attr(), Self::upvote_answer))
            .with_route((Self::downvote_answer_tool_attr(), Self::downvote_answer))
            .with_route((Self::submit_answer_tool_attr(), Self::submit_answer))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TokenOverflowServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(include_str!(
                "../../../../integrations/common/instructions.md"
            ))
    }
}
