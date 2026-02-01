# Design: MCP JWT Auth Enforcement and E2E Testing

## Architecture Overview

### Problem

The MCP endpoint (`POST /mcp`) has no JWT authentication enforcement. The
README documents that Axum middleware handles JWT for `/mcp`, but the code only
applies `jwt_auth_layer` to `/v1/*` routes. On production, the API Gateway
intentionally makes `/mcp` a public route (the MCP Streamable HTTP protocol
requires unauthenticated access for the initial handshake), but Axum was
supposed to be the second line of defense and it was never wired up.

This means anyone can call the MCP tools (`search_questions`, `submit`,
`upvote_answer`) without any token at all, both locally and in production.

In practice, Claude Code still authenticates correctly because `oauth.clientId`
is hardcoded in `.mcp.json`, which triggers the OAuth flow immediately without
needing a 401 response. But the server never validates the resulting token on
the `/mcp` route.

### Solution

Apply the existing `jwt_auth_layer` middleware to the `/mcp` route using an
Axum sub-router. Add a `WWW-Authenticate` header to 401 responses so MCP
clients that rely on RFC 6750 discovery can still trigger their OAuth flow.
Update the MCP e2e tests to send Bearer tokens and add new tests that verify
the auth enforcement.

### Before / After

```text
Before:
  POST /mcp (no token)     --> MCP handler (success)
  POST /mcp (garbage token) --> MCP handler (success)
  POST /mcp (valid token)   --> MCP handler (success)

After:
  POST /mcp (no token)      --> 401 + WWW-Authenticate header
  POST /mcp (garbage token)  --> 401
  POST /mcp (valid token)    --> MCP handler (success)
```

### Component Diagram

```text
                    Axum Router
                        |
        +---------------+----------------+
        |               |                |
   public routes   protected routes   mcp sub-router
   /health         /v1/*              /mcp
   /.well-known/*  (jwt_auth_layer)   (jwt_auth_layer)
   /oauth2/*
        |               |                |
        v               v                v
    handlers        handlers      StreamableHttpService
                                  (nest_service)
```

No new Docker services, no external dependencies. The change is entirely within
the Axum router wiring in `server.rs` and a small addition to the 401 response
in `middleware.rs`.

## Interfaces

### Changed HTTP Behavior on `POST /mcp`

**Unauthenticated request (no Bearer token):**

```text
POST /mcp HTTP/1.1
Content-Type: application/json

{"jsonrpc":"2.0","method":"initialize",...}
```

Response (new behavior):

```text
HTTP/1.1 401 Unauthorized
WWW-Authenticate: Bearer resource_metadata="http://localhost:8080/.well-known/oauth-protected-resource"
Content-Type: application/json

{"error":"Unauthorized"}
```

The `resource_metadata` URL uses the configured `api.base_url` so it resolves
correctly in all environments (local, development, production).

**Authenticated request (valid Bearer token):**

```text
POST /mcp HTTP/1.1
Content-Type: application/json
Authorization: Bearer <valid-jwt>

{"jsonrpc":"2.0","method":"initialize",...}
```

Response (unchanged):

```text
HTTP/1.1 200 OK
Content-Type: text/event-stream
...
```

### Internal Interface: `jwt_auth_layer` Reuse

The existing `jwt_auth_layer` in `middleware.rs` is reused without modification
to its signature. The only change is adding a `WWW-Authenticate` header to the
401 response when the request path is `/mcp`. This is needed because the MCP
Streamable HTTP spec (and the rmcp client library) expects a `WWW-Authenticate`
header to trigger OAuth discovery.

### Test Interface: Authenticated MCP Client

The `create_mcp_client()` helper in `tests/e2e/mcp/helpers.rs` will use the
rmcp `StreamableHttpClientTransportConfig.auth_header` field to attach a test
JWT to all MCP requests:

```rust
let transport = StreamableHttpClientTransport::from_config(
    StreamableHttpClientTransportConfig::with_uri(&*config.mcp.base_url)
        .auth_header(generate_test_jwt("system", 3600)),
);
```

This uses the same `generate_test_jwt` function already used by `TestClient`
in the REST API e2e tests.

## Logic

### 1. MCP Sub-Router with JWT Middleware (`server.rs`)

Move the `nest_service("/mcp", mcp_service)` call into its own sub-router and
apply `jwt_auth_layer` via `route_layer`:

```rust
let mcp_router = Router::new()
    .nest_service("/mcp", mcp_service)
    .route_layer(axum::middleware::from_fn_with_state(
        app_state.clone(),
        middleware::jwt_auth_layer,
    ));

let app = routes::configure(app_state.clone())
    .merge(mcp_router)
    .with_state(app_state)
    .layer(/* existing global middleware unchanged */);
```

This works because `route_layer` wraps all routes in the sub-router (including
those added via `nest_service`) with the provided middleware. The global
middleware stack (trace ID, timeout, body limit, security headers, CORS)
remains unchanged and still applies to all routes including `/mcp`.

### 2. `WWW-Authenticate` Header on 401 (`middleware.rs`)

When `jwt_auth_layer` rejects a request with a missing Bearer token on the
`/mcp` path, the 401 response must include a `WWW-Authenticate` header per
RFC 6750. The rmcp client library checks for this header to produce an
`AuthRequired` error that triggers OAuth discovery.

The middleware needs access to the request path and the `api.base_url` config
to construct the header value. The `AppState` already contains `base_url`, so
the middleware can read both from the request it already has access to.

```rust
// In jwt_auth_layer, when token is missing:
let is_mcp = req.uri().path().starts_with("/mcp");
if is_mcp {
    let base_url = &state.base_url;
    let www_auth = format!(
        "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource\"",
        base_url.trim_end_matches('/')
    );
    return (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, www_auth)],
        Json(ErrorResponse { error: "Unauthorized".to_string() }),
    ).into_response();
}
// Otherwise, fall through to existing AppError::Unauthorized behavior
```

The `WWW-Authenticate` header is only added for `/mcp` requests. REST API
clients (`/v1/*`) continue to receive the existing plain 401 JSON response.

### 3. Authenticated MCP Test Client (`tests/e2e/mcp/helpers.rs`)

Replace `StreamableHttpClientTransport::from_uri(...)` with
`StreamableHttpClientTransport::from_config(...)` using a config that includes
`auth_header`. The `generate_test_jwt` function (already in `tests/common/`)
produces a JWT signed with the test private key that matches the local JWKS.

## Edge Cases & Constraints

### SSE Reconnection with Auth

The rmcp client transport automatically reconnects SSE streams on failure. The
`auth_header` field is passed to every reconnection attempt (visible in the
`StreamableHttpClientReconnect` struct in the rmcp source). No special handling
is needed.

### DELETE /mcp (Session Cleanup)

The rmcp client sends `DELETE /mcp` with the session ID when closing a
connection. The `jwt_auth_layer` must allow this through with a valid token.
Since the middleware checks for a Bearer token regardless of HTTP method, this
works automatically. The `nest_service` routes all methods (`ANY /mcp`) through
the same service.

### Authenticated User Propagation into MCP Tools

The rmcp library automatically injects `http::request::Parts` (including Axum
request extensions) into the MCP `RequestContext.extensions`. Since
`jwt_auth_layer` inserts `AuthenticatedUser` into the HTTP request extensions,
the MCP `call_tool` handler extracts it via:

```rust
context.extensions.get::<http::request::Parts>()
    .and_then(|parts| parts.extensions.get::<AuthenticatedUser>())
```

The `submit` and `upvote_answer` tools receive the real `user_id` from the JWT
instead of the previous `SYSTEM_USER_ID` hardcode. The `search_questions` tool
does not require a user identity.

### Production Deployment

The change deploys with the next Lambda update. No infrastructure changes are
needed since the API Gateway already allows `/mcp` through as a public route.
The Axum middleware now provides the JWT enforcement that was always documented
but never implemented.

### Cloud E2E Tests

The cloud environments (development, production) use real WorkOS JWTs. The
`generate_test_jwt` function produces tokens signed with the test private key,
which only matches the local JWKS (`file://tests/assets/auth/test_jwks.json`).
Cloud environments use the WorkOS JWKS endpoint. The MCP e2e tests only run
against the local stack (`TOKENOVERFLOW_ENV=local`), so there is no conflict.

### Path Matching for `WWW-Authenticate`

The `req.uri().path().starts_with("/mcp")` check in the middleware is
intentionally broad. The MCP service is mounted at exactly `/mcp` and handles
all sub-paths internally via the Streamable HTTP protocol. No other routes
start with `/mcp`.

## Test Plan

### Unit Tests (`tests/unit/api/test_auth_middleware.rs`)

Add tests for the MCP-specific 401 behavior:

| Test | Description |
|---|---|
| `mcp_returns_401_without_token` | `POST /mcp` with no Authorization header returns 401 |
| `mcp_401_includes_www_authenticate` | The 401 response includes `WWW-Authenticate: Bearer resource_metadata="..."` |
| `mcp_returns_401_with_invalid_token` | `POST /mcp` with an expired or wrong-issuer token returns 401 |
| `mcp_passes_with_valid_token` | `POST /mcp` with a valid test JWT passes through to the handler |

These use the existing `create_mock_app_state_with_users` helper and the
`generate_test_jwt` / `generate_expired_test_jwt` functions already in the test
common module.

### E2E Tests (`tests/e2e/mcp/`)

**Updated existing tests** (all tests in `test_server.rs` and `tools/`):

All existing MCP e2e tests continue to work unchanged. The only change is in
`helpers.rs` where `create_mcp_client()` now passes a test JWT via
`StreamableHttpClientTransportConfig.auth_header`.

**New auth-specific e2e tests** (`tests/e2e/mcp/test_auth.rs`):

| Test | Description |
|---|---|
| `unauthenticated_mcp_returns_401` | Raw `POST /mcp` (via reqwest, not rmcp) without a token returns 401 with `WWW-Authenticate` header |
| `mcp_rejects_expired_token` | Raw `POST /mcp` with an expired JWT returns 401 |
| `mcp_rejects_wrong_issuer` | Raw `POST /mcp` with a JWT signed by a different issuer returns 401 |
| `authenticated_mcp_initializes` | rmcp client with a valid test JWT successfully completes the MCP initialize handshake |

The raw HTTP tests use `reqwest` directly (like the existing `test_oauth_proxy.rs`
tests) because the rmcp client does not expose the HTTP response status. The
authenticated initialization test uses the rmcp client to verify the full
protocol works end-to-end with auth.

### Existing Test Impact

| Test file | Impact |
|---|---|
| `test_oauth_proxy.rs` | Remove the "Step 1 skip" comment (lines 26-30). The 401 behavior is now testable locally. Add a step that verifies `POST /mcp` returns 401. |
| `test_server.rs` | No changes (auth is handled transparently by the updated helper) |
| `tools/test_submit.rs` | No changes |
| `tools/test_search_questions.rs` | No changes |
| `tools/test_upvote_answer.rs` | No changes |

## Documentation Changes

### README.md

1. **Remove the local/production divergence note** in the "Why is `/mcp` a
   public Gateway route?" section. The paragraph about Axum `nest_service` not
   supporting middleware is no longer accurate. Replace with a note that Axum
   middleware enforces JWT auth on `/mcp` in all environments.

2. **Update the route configuration table**: The "Axum Auth" column for
   `ANY /mcp` already says "JWT (AuthKit)" which is now correct. No table
   change needed.

### Inline Code Comments

1. **Remove** the comment block at the bottom of `test_oauth_proxy.rs`
   (lines 126-130) about local/production divergence.

2. **Remove** the comment block in `helpers.rs` (lines 7-13) about MCP having
   no JWT auth middleware locally.

3. **Update** the comment in `submit.rs:83` and `upvote_answer.rs:33` to
   reference this design document as the reason `SYSTEM_USER_ID` is still used
   (auth is now enforced at the middleware level, but user propagation into the
   MCP handler is follow-up work).

## Development Environment Changes

No changes to the development environment. No new dependencies, Docker
services, environment variables, or setup steps are required.

The existing `docker compose up -d --build api` workflow and
`cargo test -p tokenoverflow --test e2e` command continue to work as before.
The only difference is that MCP e2e tests now send a Bearer token, which is
generated automatically by the test helper using the existing test key pair.

## Tasks

### Task 1: Add `WWW-Authenticate` to MCP 401 responses

**File:** `apps/api/src/api/middleware.rs`

**Changes:**
- In `jwt_auth_layer`, when the Bearer token is missing and the request path
  starts with `/mcp`, return a 401 response with the `WWW-Authenticate` header
  containing the `resource_metadata` URL constructed from `state.base_url`.
- For non-MCP paths, keep the existing `AppError::Unauthorized` behavior.

**Success criteria:**
- The 401 response for `/mcp` includes
  `WWW-Authenticate: Bearer resource_metadata="<base_url>/.well-known/oauth-protected-resource"`
- The 401 response for `/v1/*` is unchanged (no `WWW-Authenticate` header).

### Task 2: Wire MCP sub-router with `jwt_auth_layer`

**File:** `apps/api/src/api/server.rs`

**Changes:**
- Create a sub-router containing `nest_service("/mcp", mcp_service)`.
- Apply `jwt_auth_layer` via `route_layer` on the sub-router.
- Merge the sub-router into the main app before applying global middleware.

**Success criteria:**
- `POST /mcp` without a token returns 401.
- `POST /mcp` with a valid token reaches the MCP handler.
- All existing routes (`/health`, `/v1/*`, `/.well-known/*`, `/oauth2/*`)
  behave identically to before.

### Task 3: Update MCP test helper to send Bearer token

**File:** `apps/api/tests/e2e/mcp/helpers.rs`

**Changes:**
- Replace `StreamableHttpClientTransport::from_uri(...)` with
  `StreamableHttpClientTransport::from_config(...)` using a config that
  includes `auth_header` set to `generate_test_jwt("system", 3600)`.
- Add the necessary import for `generate_test_jwt` and
  `StreamableHttpClientTransportConfig`.

**Success criteria:**
- All existing MCP e2e tests pass without modification.

### Task 4: Add MCP auth e2e tests

**File:** `apps/api/tests/e2e/mcp/test_auth.rs`

**Changes:**
- Add `test_auth` module to `tests/e2e/mcp/mod.rs`.
- Implement `unauthenticated_mcp_returns_401`: raw POST via reqwest, assert
  401 status and `WWW-Authenticate` header presence.
- Implement `mcp_rejects_expired_token`: POST with expired JWT, assert 401.
- Implement `mcp_rejects_wrong_issuer`: POST with wrong-issuer JWT, assert
  401.
- Implement `authenticated_mcp_initializes`: use rmcp client with valid JWT,
  assert successful `list_tools` call.

**Success criteria:**
- All four new tests pass.
- Tests use the existing `generate_test_jwt`, `generate_expired_test_jwt`,
  and `generate_test_jwt_custom` functions from `tests/common/test_jwt.rs`.

### Task 5: Add MCP auth unit tests

**File:** `apps/api/tests/unit/api/test_auth_middleware.rs`

**Changes:**
- Add `mcp_returns_401_without_token`: verify `/mcp` returns 401.
- Add `mcp_401_includes_www_authenticate`: verify the header value.
- Add `mcp_returns_401_with_invalid_token`: verify expired/bad tokens.
- Add `mcp_passes_with_valid_token`: verify a valid JWT passes through.

**Success criteria:**
- All four new unit tests pass.

### Task 6: Update `test_oauth_proxy.rs` discovery chain

**File:** `apps/api/tests/e2e/api/routes/test_oauth_proxy.rs`

**Changes:**
- In `mcp_auth_discovery_chain`, uncomment/add Step 1: verify that
  `POST /mcp` without a token returns 401.
- Remove the comment block at lines 126-130 about local/production divergence.

**Success criteria:**
- The discovery chain test now includes the 401 step.

### Task 7: Update documentation and comments

**Files:**
- `README.md`: Remove the local/production divergence paragraph in the
  "Why is `/mcp` a public Gateway route?" section.
- `apps/api/tests/e2e/mcp/helpers.rs`: Remove the comment about MCP having
  no JWT auth locally.
- `apps/api/src/mcp/server.rs`: Extract `AuthenticatedUser` from
  `RequestContext.extensions` in `call_tool` and pass `user_id` to tool impls.
- `apps/api/src/mcp/tools/submit.rs` and `upvote_answer.rs`: Replace
  `SYSTEM_USER_ID` with the `user_id` parameter from the authenticated user.
- `apps/api/tests/unit/mcp/helpers.rs`: Inject `AuthenticatedUser` inside
  `http::request::Parts` into the test context extensions.
