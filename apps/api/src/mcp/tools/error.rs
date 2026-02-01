use rmcp::model::{CallToolResult, Content};

use crate::error::AppError;

/// Convert an AppError into a CallToolResult with isError: true.
/// Per MCP spec, tool execution errors (validation, not-found, DB) use
/// isError in the result, not protocol-level JSON-RPC errors.
pub(crate) fn error_result(err: AppError) -> CallToolResult {
    CallToolResult::error(vec![Content::text(err.to_string())])
}
