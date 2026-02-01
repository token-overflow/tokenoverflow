use rmcp::ErrorData as McpError;
use rmcp::handler::server::common::{AsRequestContext, FromContextPart};

use crate::api::extractors::AuthenticatedUser;

/// MCP tool parameter extractor for the authenticated user.
///
/// Extracts AuthenticatedUser from the http::request::Parts that rmcp's
/// StreamableHttpService injects into the MCP RequestContext extensions.
/// The Parts carry the AuthenticatedUser set by jwt_auth_layer.
///
/// Usage in #[tool] methods:
/// ```ignore
/// #[tool]
/// async fn my_tool(&self, Auth(user): Auth) -> CallToolResult { ... }
/// ```
pub struct Auth(pub AuthenticatedUser);

impl<C: AsRequestContext> FromContextPart<C> for Auth {
    fn from_context_part(context: &mut C) -> Result<Self, McpError> {
        let parts = context
            .as_request_context()
            .extensions
            .get::<http::request::Parts>()
            .ok_or_else(|| McpError::internal_error("Missing HTTP request parts", None))?;
        let user = parts
            .extensions
            .get::<AuthenticatedUser>()
            .ok_or_else(|| McpError::internal_error("Missing authenticated user", None))?;
        Ok(Auth(user.clone()))
    }
}
