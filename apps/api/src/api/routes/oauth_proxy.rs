use std::sync::LazyLock;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use http::StatusCode;

use crate::api::state::AppState;

/// Shared HTTP client for OAuth proxy requests.
///
/// Reuses connections to AuthKit across requests. Configured with timeouts
/// to prevent hanging if AuthKit is slow or unreachable.
static PROXY_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build OAuth proxy HTTP client")
});

/// GET /oauth2/authorize
///
/// Scope injection proxy for the OAuth authorization endpoint.
/// Adds `scope=openid profile offline_access` if not already present, then redirects (302)
/// to AuthKit's real authorization endpoint.
///
/// This works around a Claude Code bug where the MCP client omits the `scope`
/// parameter from authorization requests. Without scopes, AuthKit returns
/// `invalid_scope` because it requires at least `openid`.
///
/// See: https://github.com/anthropics/claude-code/issues/4540
pub async fn authorize(
    State(state): State<AppState>,
    Query(mut params): Query<Vec<(String, String)>>,
) -> impl IntoResponse {
    // Inject scope if missing or empty. Works around a Claude Code bug
    // where it sends scope="" or omits it entirely.
    let has_valid_scope = params
        .iter()
        .any(|(k, v)| k == "scope" && !v.trim().is_empty() && v.trim() != "\"\"");
    if !has_valid_scope {
        params.retain(|(k, _)| k != "scope");
        params.push((
            "scope".to_string(),
            "openid profile offline_access".to_string(),
        ));
    }

    let query_string = serde_urlencoded::to_string(&params).unwrap_or_default();
    let redirect_url = format!(
        "{}/oauth2/authorize?{}",
        state.auth_config.authkit_url, query_string
    );

    Redirect::to(&redirect_url)
}

/// POST /oauth2/token
///
/// Token exchange proxy. Forwards the form-encoded request body to AuthKit's
/// token endpoint and returns the response as-is (status, headers, body).
///
/// Handles both initial authorization code exchange and refresh token requests.
///
/// Exists to work around a Claude Code bug where the MCP client omits the
/// `scope` parameter from authorization requests.
/// See: https://github.com/anthropics/claude-code/issues/4540
///
/// SSRF note: The destination URL (`authkit_token_url`) is constructed from
/// `state.auth_config.authkit_url`, a server-side config value loaded from
/// TOML, not from user input. The request body is user-provided but the
/// destination is always the configured AuthKit endpoint. The global 100 KB
/// body size limit (server.rs) applies before this handler runs.
pub async fn token(State(state): State<AppState>, body: String) -> Response {
    let authkit_token_url = format!("{}/oauth2/token", state.auth_config.authkit_url);

    let result = PROXY_CLIENT
        .post(&authkit_token_url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await;

    match result {
        Ok(resp) => forward_response(resp).await,
        Err(e) => {
            tracing::error!("Failed to reach AuthKit token endpoint: {}", e);
            (StatusCode::BAD_GATEWAY, "Authorization server unreachable").into_response()
        }
    }
}

/// POST /oauth2/register
///
/// Dynamic Client Registration proxy. Forwards JSON request body to AuthKit's
/// registration endpoint and returns the response as-is.
///
/// This is a fallback in case the MCP client attempts DCR. With a
/// pre-configured `oauth.clientId` in `.mcp.json`, this should not be called,
/// but proxying it keeps the metadata document complete.
///
/// Exists to work around a Claude Code bug where the MCP client omits the
/// `scope` parameter from authorization requests.
/// See: https://github.com/anthropics/claude-code/issues/4540
///
/// SSRF note: The destination URL (`authkit_register_url`) is constructed
/// from `state.auth_config.authkit_url`, a server-side config value loaded
/// from TOML, not from user input. The request body is user-provided but the
/// destination is always the configured AuthKit endpoint. The global 100 KB
/// body size limit (server.rs) applies before this handler runs.
pub async fn register(State(state): State<AppState>, body: String) -> Response {
    let authkit_register_url = format!("{}/oauth2/register", state.auth_config.authkit_url);

    let result = PROXY_CLIENT
        .post(&authkit_register_url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await;

    match result {
        Ok(resp) => forward_response(resp).await,
        Err(e) => {
            tracing::error!("Failed to reach AuthKit register endpoint: {}", e);
            (StatusCode::BAD_GATEWAY, "Authorization server unreachable").into_response()
        }
    }
}

/// Convert a reqwest response into an axum response, preserving status code,
/// content-type header, and body.
async fn forward_response(resp: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();

    let body_bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to read AuthKit response body: {}", e);
            return (StatusCode::BAD_GATEWAY, "Failed to read upstream response").into_response();
        }
    };

    let mut builder = Response::builder().status(status);
    if let Some(ct) = content_type {
        builder = builder.header(http::header::CONTENT_TYPE, ct);
    }
    builder
        .body(Body::from(body_bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
