use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::tower::{
    StreamableHttpServerConfig, StreamableHttpService,
};
use tower::ServiceExt;

use tokenoverflow::api::server::mcp_allowed_hosts;
use tokenoverflow::mcp::TokenOverflowServer;

mod common {
    include!("../../common/mod.rs");
}

#[test]
fn mcp_allowed_hosts_includes_public_host_and_loopback() {
    let hosts = mcp_allowed_hosts("https://api.tokenoverflow.io");

    assert!(hosts.contains(&"api.tokenoverflow.io".to_string()));
    assert!(hosts.contains(&"localhost".to_string()));
    assert!(hosts.contains(&"127.0.0.1".to_string()));
    assert!(hosts.contains(&"::1".to_string()));
    assert!(!hosts.contains(&"evil.com".to_string()));
}

// Builds the /mcp service exactly as `server.rs` does so the only possible 403
// is the DNS-rebinding Host guard, then proves the allow-list admits the
// configured public host and rejects a foreign one.
fn mcp_app() -> Router {
    let state = common::create_mock_app_state();
    let mcp_config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(mcp_allowed_hosts("https://api.tokenoverflow.io"));
    let mcp_service = StreamableHttpService::new(
        move || Ok(TokenOverflowServer::new(state.clone())),
        Arc::new(NeverSessionManager::default()),
        mcp_config,
    );
    Router::new().nest_service("/mcp", mcp_service)
}

async fn post_mcp_with_host(host: &str) -> u16 {
    let request = http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", host)
        .body(axum::body::Body::empty())
        .unwrap();

    mcp_app().oneshot(request).await.unwrap().status().as_u16()
}

#[tokio::test]
async fn host_guard_admits_configured_public_host() {
    // Guard runs before method/Accept checks, so a non-403 status means the
    // host was admitted (the request fails later for unrelated reasons).
    let status = post_mcp_with_host("api.tokenoverflow.io").await;
    assert_ne!(status, 403);
}

#[tokio::test]
async fn host_guard_rejects_foreign_host() {
    let status = post_mcp_with_host("evil.com").await;
    assert_eq!(status, 403);
}
