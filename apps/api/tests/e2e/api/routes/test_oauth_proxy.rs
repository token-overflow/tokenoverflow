use http::StatusCode;
use serde_json::Value;

mod common {
    include!("../../../common/mod.rs");
}

// ============================================================================
// MCP Auth Discovery Chain
//
// Simulates the steps an MCP client (e.g., Claude Code) takes to discover
// and authenticate with our OAuth proxy. Tests the full chain without
// completing the browser login (which requires human interaction).
// ============================================================================

#[tokio::test]
async fn mcp_auth_discovery_chain() {
    let config = tokenoverflow::config::Config::load().expect("Failed to load config");
    let base_url = config.api.base_url.trim_end_matches('/');

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to build HTTP client");

    // Step 1: POST /mcp without token returns 401 with WWW-Authenticate
    let resp = client
        .post(format!("{}/mcp", base_url))
        .header("Content-Type", "application/json")
        .body(r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#)
        .send()
        .await
        .expect("POST /mcp failed");
    assert_eq!(
        resp.status().as_u16(),
        401,
        "Step 1: unauthenticated POST /mcp should return 401"
    );
    let www_auth = resp
        .headers()
        .get("www-authenticate")
        .expect("Step 1: 401 must include WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(
        www_auth.contains("/.well-known/oauth-protected-resource"),
        "Step 1: WWW-Authenticate should reference discovery URL, got: {}",
        www_auth
    );

    // Step 2: GET /.well-known/oauth-protected-resource -> authorization_servers
    let resp = client
        .get(format!(
            "{}/{}",
            base_url, ".well-known/oauth-protected-resource"
        ))
        .send()
        .await
        .expect("GET protected-resource failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = serde_json::from_str(&resp.text().await.unwrap())
        .expect("Protected resource response should be valid JSON");
    let auth_server = body["authorization_servers"][0]
        .as_str()
        .expect("authorization_servers[0] should be a string");

    // Step 3: GET /.well-known/oauth-authorization-server from the discovered server
    let resp = client
        .get(format!(
            "{}/.well-known/oauth-authorization-server",
            auth_server
        ))
        .send()
        .await
        .expect("GET authorization-server metadata failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let metadata: Value = serde_json::from_str(&resp.text().await.unwrap())
        .expect("Authorization server metadata should be valid JSON");

    let auth_endpoint = metadata["authorization_endpoint"]
        .as_str()
        .expect("authorization_endpoint should be a string");
    let token_endpoint = metadata["token_endpoint"]
        .as_str()
        .expect("token_endpoint should be a string");

    // Proxy URLs should point to our API, not directly to AuthKit
    assert!(
        auth_endpoint.starts_with(base_url),
        "authorization_endpoint should point to our API, got: {}",
        auth_endpoint
    );
    assert!(
        token_endpoint.starts_with(base_url),
        "token_endpoint should point to our API, got: {}",
        token_endpoint
    );

    // Step 4: GET /oauth2/authorize -> 302/303 to AuthKit with scope injected
    let resp = client
        .get(format!(
            "{}?client_id=test_client&response_type=code&code_challenge=abc&code_challenge_method=S256&redirect_uri=http://localhost:4001/callback",
            auth_endpoint
        ))
        .send()
        .await
        .expect("GET /oauth2/authorize failed");

    let status = resp.status().as_u16();
    assert!(
        status == 302 || status == 303,
        "Step 4: authorize should redirect, got: {}",
        status
    );

    let location = resp
        .headers()
        .get("location")
        .expect("redirect must have Location header")
        .to_str()
        .unwrap();

    // Should redirect to AuthKit (not back to ourselves)
    assert!(
        !location.starts_with(base_url),
        "authorize should redirect to AuthKit, not back to ourselves, got: {}",
        location
    );

    // Should have scope injected
    assert!(
        location.contains("scope=openid"),
        "authorize redirect should include scope, got: {}",
        location
    );

    // Original params should be preserved
    assert!(
        location.contains("client_id=test_client"),
        "authorize redirect should preserve client_id, got: {}",
        location
    );
}

// ============================================================================
// GET /.well-known/oauth-protected-resource
// ============================================================================

#[tokio::test]
async fn oauth_protected_resource_points_to_local_api() {
    let client = common::TestClient::from_config();

    let response = client.get("/.well-known/oauth-protected-resource").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    let servers = body["authorization_servers"]
        .as_array()
        .expect("authorization_servers should be an array");

    assert!(
        !servers.is_empty(),
        "authorization_servers should not be empty"
    );

    // The resource field should match the API base URL
    assert!(body["resource"].is_string(), "resource should be a string");
}

// ============================================================================
// GET /.well-known/oauth-authorization-server
// ============================================================================

#[tokio::test]
async fn oauth_authorization_server_has_proxy_urls() {
    let client = common::TestClient::from_config();

    let response = client.get("/.well-known/oauth-authorization-server").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();

    // All proxy endpoints should be present and be strings
    assert!(
        body["authorization_endpoint"].is_string(),
        "authorization_endpoint should be a string"
    );
    assert!(
        body["token_endpoint"].is_string(),
        "token_endpoint should be a string"
    );
    assert!(
        body["registration_endpoint"].is_string(),
        "registration_endpoint should be a string"
    );

    // Verify they contain the expected paths
    let auth_ep = body["authorization_endpoint"].as_str().unwrap();
    let token_ep = body["token_endpoint"].as_str().unwrap();
    let reg_ep = body["registration_endpoint"].as_str().unwrap();

    assert!(
        auth_ep.ends_with("/oauth2/authorize"),
        "authorization_endpoint should end with /oauth2/authorize, got: {}",
        auth_ep
    );
    assert!(
        token_ep.ends_with("/oauth2/token"),
        "token_endpoint should end with /oauth2/token, got: {}",
        token_ep
    );
    assert!(
        reg_ep.ends_with("/oauth2/register"),
        "registration_endpoint should end with /oauth2/register, got: {}",
        reg_ep
    );

    // Issuer and JWKS should also be present
    assert!(body["issuer"].is_string(), "issuer should be a string");
    assert!(body["jwks_uri"].is_string(), "jwks_uri should be a string");
}

// ============================================================================
// GET /oauth2/authorize - Redirect
// ============================================================================

#[tokio::test]
async fn authorize_redirects() {
    let config = tokenoverflow::config::Config::load().expect("Failed to load config");
    let base_url = config.api.base_url.trim_end_matches('/');

    // Build a client that does NOT follow redirects so we can inspect the 302
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to build HTTP client");

    let response = client
        .get(format!("{}/oauth2/authorize?client_id=test", base_url))
        .send()
        .await
        .expect("HTTP request failed");

    // Should be a redirect (302 or 303)
    let status = response.status().as_u16();
    assert!(
        status == 302 || status == 303,
        "Expected a redirect status, got: {}",
        status
    );

    let location = response
        .headers()
        .get("location")
        .expect("redirect must have a Location header")
        .to_str()
        .unwrap();

    // The Location should contain oauth2/authorize (pointing to AuthKit)
    assert!(
        location.contains("/oauth2/authorize"),
        "Location should contain /oauth2/authorize, got: {}",
        location
    );

    // Should have injected scope since we did not provide one
    assert!(
        location.contains("scope="),
        "Location should contain scope= parameter, got: {}",
        location
    );

    // Original client_id param should be preserved
    assert!(
        location.contains("client_id=test"),
        "Location should preserve client_id, got: {}",
        location
    );
}
