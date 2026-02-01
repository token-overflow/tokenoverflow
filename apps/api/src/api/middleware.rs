use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use diesel_async::AsyncConnection;
use http::{HeaderValue, StatusCode, header};
use tracing::Instrument;

use crate::api::extractors::AuthenticatedUser;
use crate::api::state::AppState;
use crate::error::AppError;

pub async fn trace_id(req: Request, next: Next) -> Response {
    let id = extract_trace_id(&req);
    let span = tracing::info_span!("request", trace_id = %id);

    async move {
        let mut response = next.run(req).await;
        if let Ok(val) = HeaderValue::from_str(&id) {
            response.headers_mut().insert("X-Trace-Id", val);
        }
        response
    }
    .instrument(span)
    .await
}

fn extract_trace_id(req: &Request) -> String {
    // In Lambda mode: extract requestContext.requestId from lambda_http extensions.
    // Handles both REST API (V1) and HTTP API (V2) payload formats.
    // In local mode (no Lambda context): generate a UUID.
    req.extensions()
        .get::<lambda_http::request::RequestContext>()
        .and_then(|ctx| match ctx {
            lambda_http::request::RequestContext::ApiGatewayV1(v1) => v1.request_id.clone(),
            lambda_http::request::RequestContext::ApiGatewayV2(v2) => v2.request_id.clone(),
            _ => None,
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

/// JWT authentication middleware layer.
///
/// Designed to be used with `axum::middleware::from_fn_with_state`.
/// Extracts the Bearer token from the Authorization header, validates it
/// against the configured JWKS, resolves the local user, and injects
/// `AuthenticatedUser` into request extensions.
pub async fn jwt_auth_layer(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let is_mcp = path.starts_with("/mcp");

    let token = match extract_bearer_token(&req) {
        Some(t) => t,
        None => {
            // MCP clients (e.g., rmcp) rely on RFC 6750 WWW-Authenticate to
            // trigger OAuth discovery. REST API clients get the plain 401.
            if is_mcp {
                return mcp_unauthorized_response(&state.api_base_url);
            }
            return AppError::Unauthorized("Missing Bearer token".to_string()).into_response();
        }
    };

    let claims = match state.auth.validate_jwt(&token).await {
        Ok(c) => c,
        Err(e) => {
            // MCP clients need WWW-Authenticate to re-initiate OAuth when
            // the token is expired or invalid (not just missing).
            if is_mcp {
                return mcp_unauthorized_response(&state.api_base_url);
            }
            return e.into_response();
        }
    };

    let mut conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => {
            return AppError::Internal(format!("Pool checkout failed: {}", e)).into_response();
        }
    };

    let workos_id = claims.sub;

    // Wrap resolve_user in a transaction so the find-or-create pattern
    // is atomic (guards against race conditions in multi-connection envs).
    let user = match (*conn)
        .transaction::<_, AppError, _>(|conn| {
            let wid = workos_id.clone();
            Box::pin(async move {
                state
                    .auth
                    .resolve_user(state.users.as_ref(), conn, &wid)
                    .await
            })
        })
        .await
    {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    // Drop the connection before passing control to the handler.
    // With max_size=1, the handler needs this connection back in the pool.
    drop(conn);

    req.extensions_mut().insert(AuthenticatedUser {
        id: user.id,
        workos_id,
    });

    next.run(req).await
}

/// Extract Bearer token from the Authorization header.
fn extract_bearer_token(req: &Request) -> Option<String> {
    let header = req.headers().get(http::header::AUTHORIZATION)?;
    let value = header.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    Some(token.to_string())
}

/// Build a 401 response with `WWW-Authenticate` header for MCP requests.
///
/// The `resource_metadata` URL points to the OAuth protected resource
/// metadata endpoint so that MCP clients can discover the authorization
/// server and initiate the OAuth flow (RFC 6750).
///
/// The `scope` parameter tells MCP clients which scopes to request
/// (RFC 6750 S3, MCP spec "Scope Selection Strategy"). The value
/// `openid profile offline_access` matches what the OAuth proxy injects
/// for the claude-code#4540 workaround. `offline_access` is required so
/// that AuthKit issues a refresh token, allowing the MCP client to
/// renew access tokens without re-authenticating via the browser.
fn mcp_unauthorized_response(api_base_url: &str) -> Response {
    let www_auth = format!(
        "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource\", \
         scope=\"openid profile offline_access\"",
        api_base_url.trim_end_matches('/')
    );

    #[derive(serde::Serialize)]
    struct ErrorBody {
        error: String,
    }

    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, www_auth)],
        axum::Json(ErrorBody {
            error: "Unauthorized".to_string(),
        }),
    )
        .into_response()
}

/// Build a 403 response with `WWW-Authenticate` header for MCP scope
/// step-up (RFC 6750 S3, MCP spec "Scope Step-Up").
///
/// Use this when the token is valid but lacks the scope required by a
/// specific tool. The client should request a new token with the
/// indicated scope.
pub fn mcp_forbidden_response(api_base_url: &str, required_scope: &str) -> Response {
    let www_auth = format!(
        "Bearer error=\"insufficient_scope\", \
         scope=\"{}\", \
         resource_metadata=\"{}/.well-known/oauth-protected-resource\"",
        required_scope,
        api_base_url.trim_end_matches('/')
    );

    #[derive(serde::Serialize)]
    struct ErrorBody {
        error: String,
    }

    (
        StatusCode::FORBIDDEN,
        [(header::WWW_AUTHENTICATE, www_auth)],
        axum::Json(ErrorBody {
            error: "Forbidden".to_string(),
        }),
    )
        .into_response()
}
