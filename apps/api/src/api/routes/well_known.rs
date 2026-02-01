use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::api::state::AppState;

#[derive(Serialize)]
pub struct ProtectedResourceMetadata {
    pub resource: String,
    pub authorization_servers: Vec<String>,
    pub bearer_methods_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
}

/// GET /.well-known/oauth-protected-resource
///
/// Returns the Protected Resource Metadata document (RFC 9728).
/// Tells MCP clients where to find the authorization server.
///
/// The `authorization_servers` field points to our own API (not directly to
/// AuthKit) so that the MCP client discovers our OAuth proxy endpoints.
/// The proxy is needed to work around a Claude Code bug where the MCP
/// client omits the `scope` parameter from authorization requests.
/// See: https://github.com/anthropics/claude-code/issues/4540
///
/// When the bug is fixed, change `authorization_servers` to point directly
/// to AuthKit and remove the proxy endpoints.
pub async fn oauth_protected_resource(
    State(state): State<AppState>,
) -> Json<ProtectedResourceMetadata> {
    Json(ProtectedResourceMetadata {
        resource: state.api_base_url.clone(),
        authorization_servers: vec![state.api_base_url.clone()],
        bearer_methods_supported: vec!["header".to_string()],
        scopes_supported: ["openid", "profile", "offline_access"]
            .map(String::from)
            .to_vec(),
    })
}

#[derive(Serialize)]
pub struct AuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
    pub jwks_uri: String,
    pub scopes_supported: Vec<String>,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
}

/// GET /.well-known/oauth-authorization-server
///
/// Returns an OAuth Authorization Server Metadata document (RFC 8414) that
/// mirrors AuthKit's metadata but with our proxy URLs substituted in for
/// the authorization, token, and registration endpoints.
///
/// This endpoint exists because the OAuth proxy is needed to work around
/// a Claude Code bug where it omits the `scope` parameter from
/// authorization requests.
/// See: https://github.com/anthropics/claude-code/issues/4540
///
/// Known limitation (RFC 8414 S3.3): The `issuer` field is AuthKit's URL
/// while `authorization_servers` in the PRM points to our API. A strict
/// RFC 8414 client would reject this metadata because the issuer does not
/// match the URL it was fetched from. Claude Code tolerates this; other
/// MCP clients (Cursor, Codex CLI, Copilot) have not been tested. When
/// the Claude Code bug is fixed, remove the proxy and point
/// `authorization_servers` directly to AuthKit to eliminate this mismatch.
///
/// The `jwks_uri` still points to AuthKit because tokens are issued by
/// AuthKit, not by us.
pub async fn oauth_authorization_server(
    State(state): State<AppState>,
) -> Json<AuthorizationServerMetadata> {
    let authkit = &state.auth_config.authkit_url;
    let api = &state.api_base_url;

    Json(AuthorizationServerMetadata {
        issuer: authkit.clone(),
        authorization_endpoint: format!("{}/oauth2/authorize", api),
        token_endpoint: format!("{}/oauth2/token", api),
        registration_endpoint: format!("{}/oauth2/register", api),
        jwks_uri: format!("{}/oauth2/jwks", authkit),
        scopes_supported: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
            "offline_access".to_string(),
        ],
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string()],
        token_endpoint_auth_methods_supported: vec![
            "none".to_string(),
            "client_secret_post".to_string(),
            "client_secret_basic".to_string(),
        ],
    })
}
