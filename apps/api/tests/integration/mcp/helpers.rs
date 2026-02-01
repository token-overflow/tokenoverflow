use rmcp::model::{Extensions, RequestId};
use rmcp::service::{
    RequestContext, RoleServer, RxJsonRpcMessage, TxJsonRpcMessage, serve_directly,
};
use tokenoverflow::api::extractors::AuthenticatedUser;

/// Dummy handler to bootstrap a Peer via serve_directly
struct DummyHandler;

impl rmcp::handler::server::ServerHandler for DummyHandler {}

/// Create test context for MCP requests.
///
/// Spawns a dummy service to obtain a `Peer<RoleServer>` since `Peer::new`
/// is crate-private in rmcp 0.13. Injects a fake `http::request::Parts` with
/// an `AuthenticatedUser` to mirror what the real server provides via
/// jwt_auth_layer + rmcp's automatic Parts injection.
pub(super) fn test_context() -> RequestContext<RoleServer> {
    let dummy_stream = futures::stream::pending::<RxJsonRpcMessage<RoleServer>>();
    let dummy_sink = futures::sink::drain::<TxJsonRpcMessage<RoleServer>>();
    let running = serve_directly(DummyHandler, (dummy_sink, dummy_stream), None);
    let peer = running.peer().clone();

    // Build http::request::Parts with AuthenticatedUser in its extensions,
    // matching what jwt_auth_layer injects in production.
    let (mut parts, _body) = http::Request::builder()
        .body(())
        .expect("empty request must build")
        .into_parts();
    parts.extensions.insert(AuthenticatedUser {
        id: tokenoverflow::constants::SYSTEM_USER_ID,
        workos_id: "system".to_string(),
    });

    let mut extensions = Extensions::new();
    extensions.insert(parts);

    let mut ctx = RequestContext::new(RequestId::Number(0), peer);
    ctx.extensions = extensions;
    ctx
}

/// Extract text content from MCP response
pub(super) fn extract_text(result: &rmcp::model::CallToolResult) -> &str {
    let content = &result.content[0];
    &content.as_text().expect("Content should be text").text
}

/// Create test context for MCP requests with an alternate voter identity.
///
/// Uses TEST_VOTER_ID so the caller is not the answer author and can vote
/// without triggering the self-vote guard.
pub(super) fn test_voter_context() -> RequestContext<RoleServer> {
    let dummy_stream = futures::stream::pending::<RxJsonRpcMessage<RoleServer>>();
    let dummy_sink = futures::sink::drain::<TxJsonRpcMessage<RoleServer>>();
    let running = serve_directly(DummyHandler, (dummy_sink, dummy_stream), None);
    let peer = running.peer().clone();

    let (mut parts, _body) = http::Request::builder()
        .body(())
        .expect("empty request must build")
        .into_parts();
    parts.extensions.insert(AuthenticatedUser {
        id: uuid::Uuid::from_bytes([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x02,
        ]),
        workos_id: "test-voter".to_string(),
    });

    let mut extensions = Extensions::new();
    extensions.insert(parts);

    let mut ctx = RequestContext::new(RequestId::Number(0), peer);
    ctx.extensions = extensions;
    ctx
}

/// Extract the behavioral hint (second content item) from MCP response
pub(super) fn extract_hint(result: &rmcp::model::CallToolResult) -> &str {
    let content = &result.content[1];
    &content.as_text().expect("Hint should be text").text
}
