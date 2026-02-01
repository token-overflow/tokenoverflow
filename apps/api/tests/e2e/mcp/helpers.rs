use rmcp::ServiceExt;
use rmcp::model::CallToolResult;
use rmcp::service::{Peer, RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use tokenoverflow::config::Config;

mod common {
    include!("../../common/mod.rs");
}

/// Connect to the running MCP server via Streamable HTTP with a valid test JWT.
pub(super) async fn create_mcp_client() -> RunningService<RoleClient, ()> {
    create_mcp_client_as("system").await
}

/// Connect as the `test-voter` user (distinct identity for vote tests).
pub(super) async fn create_mcp_voter_client() -> RunningService<RoleClient, ()> {
    create_mcp_client_as("test-voter").await
}

async fn create_mcp_client_as(sub: &str) -> RunningService<RoleClient, ()> {
    let config = Config::load().expect("Failed to load config");
    let token = common::generate_test_jwt(sub, 3600);
    let transport_config =
        StreamableHttpClientTransportConfig::with_uri(&*config.mcp.base_url).auth_header(token);
    let transport = StreamableHttpClientTransport::from_config(transport_config);
    ().serve(transport)
        .await
        .expect("MCP client initialization failed")
}

/// Convenience wrapper to get the peer from a running service.
pub(super) fn peer(service: &RunningService<RoleClient, ()>) -> &Peer<RoleClient> {
    service.peer()
}

/// Extract text content from MCP response.
pub(super) fn extract_text(result: &CallToolResult) -> &str {
    &result.content[0]
        .as_text()
        .expect("Content should be text")
        .text
}

/// Extract the behavioral hint (second content item) from MCP response.
pub(super) fn extract_hint(result: &CallToolResult) -> &str {
    &result.content[1]
        .as_text()
        .expect("Hint should be text")
        .text
}
