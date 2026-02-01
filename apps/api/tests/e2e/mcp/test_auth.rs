use http::StatusCode;

use super::helpers::{create_mcp_client, peer};

mod common {
    include!("../../common/mod.rs");
}

use common::test_jwt::{generate_expired_test_jwt, generate_test_jwt_custom};

// ============================================================================
// Unauthenticated / invalid token tests (raw HTTP via reqwest)
//
// These bypass the rmcp client to inspect HTTP-level details (status codes,
// headers) that the MCP protocol layer abstracts away.
// ============================================================================

fn mcp_base_url() -> String {
    let config = tokenoverflow::config::Config::load().expect("Failed to load config");
    config.mcp.base_url.clone()
}

#[tokio::test]
async fn unauthenticated_mcp_returns_401() {
    let client = reqwest::Client::new();
    let resp = client
        .post(mcp_base_url())
        .header("Content-Type", "application/json")
        .body(r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#)
        .send()
        .await
        .expect("POST /mcp failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED.as_u16());

    let www_auth = resp
        .headers()
        .get("www-authenticate")
        .expect("401 on /mcp must include WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(
        www_auth.starts_with("Bearer resource_metadata="),
        "WWW-Authenticate should start with 'Bearer resource_metadata=', got: {}",
        www_auth
    );
    assert!(
        www_auth.contains("/.well-known/oauth-protected-resource"),
        "WWW-Authenticate should contain discovery URL, got: {}",
        www_auth
    );
}

#[tokio::test]
async fn mcp_rejects_expired_token() {
    let client = reqwest::Client::new();
    let token = generate_expired_test_jwt("system");
    let resp = client
        .post(mcp_base_url())
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#)
        .send()
        .await
        .expect("POST /mcp failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED.as_u16());
}

#[tokio::test]
async fn mcp_rejects_wrong_issuer() {
    let client = reqwest::Client::new();
    let token = generate_test_jwt_custom("system", "wrong-issuer", "http://localhost:8080", 3600);
    let resp = client
        .post(mcp_base_url())
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#)
        .send()
        .await
        .expect("POST /mcp failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED.as_u16());
}

// ============================================================================
// Authenticated MCP client test (via rmcp)
// ============================================================================

#[tokio::test]
async fn authenticated_mcp_initializes() {
    let client = create_mcp_client().await;
    let tools = peer(&client)
        .list_all_tools()
        .await
        .expect("list_tools should succeed with valid auth");

    assert!(
        !tools.is_empty(),
        "Authenticated MCP client should see at least one tool"
    );
}
