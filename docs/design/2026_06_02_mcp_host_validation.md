# Design: Mcp Host Validation

## Context & Problem

Claude Code cannot connect to the production MCP server. OAuth completes
successfully (the `/oauth2/token` proxy returns 200 and the client reports "Got
new credentials"), but the immediate `POST /mcp` reconnect returns `403` and the
client surfaces "rejected them on reconnect". The Lambda logs show the rmcp
Streamable HTTP transport rejecting the request: `WARN ... rejected request with
disallowed Host header (possible DNS rebinding attempt) host=api.tokenoverflow.io`.
The transport's DNS-rebinding guard ships an allow-list defaulting to loopback
only, and the server never widens it, so any non-loopback deployment rejects its
own public host (production is the live example). A second, non-blocking symptom
appears in the
same trace: `GET /.well-known/oauth-protected-resource/mcp` returns `404` because
only the root metadata path is registered; the client falls back to the root
document, so auth still succeeds, but the path-scoped probe mandated by RFC 9728
is unanswered.

## In Scope

- Admit each environment's public host through the MCP transport's
  DNS-rebinding `Host` allow-list while keeping loopback allowed.
- Serve the RFC 9728 path-scoped Protected Resource Metadata document at
  `/.well-known/oauth-protected-resource/mcp`.
- Tests proving the host guard admits the configured public host and rejects
  others, plus a test for the new metadata route.

## Out of Scope

- `Origin` header validation / CORS tightening (the transport's
  `allowed_origins` guard stays disabled; CORS stays `Any`).
- Stateful MCP sessions or any change to the MCP protocol surface.
- Changing the root metadata document, the `WWW-Authenticate` challenge, or the
  OAuth proxy behavior.
- Honoring RFC 8707 resource indicators in token audience validation.
- Any infrastructure (Terraform / API Gateway) change.

## Terminology

- **DNS-rebinding guard:** rmcp's inbound `Host`-header allow-list that defends
  a locally bound server from browser-driven DNS rebinding attacks.
- **Allowed host:** an entry in `StreamableHttpServerConfig.allowed_hosts`; a
  bare host (no port) matches the incoming `Host` regardless of port.
- **PRM:** Protected Resource Metadata, the RFC 9728 JSON document that tells an
  OAuth client which authorization server protects a resource.
- **Path-scoped well-known URL:** the RFC 9728 §3.1 form where
  `/.well-known/oauth-protected-resource` is inserted between the resource's host
  and its path, giving `/.well-known/oauth-protected-resource/mcp`.

## Key Decisions

### How should the MCP transport's `Host` allow-list be populated per environment?

#### ✅ Option 1: Derive the allowed host from `config.api.base_url` plus loopback

Build the allow-list at startup from the per-environment base URL we already
configure, always retaining loopback. The host is parsed out of the URL so the
value is never duplicated.

```rust
pub fn mcp_allowed_hosts(base_url: &str) -> Vec<String> {
    let mut hosts = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    if let Some(host) = base_url
        .parse::<http::Uri>()
        .ok()
        .and_then(|uri| uri.host().map(str::to_owned))
    {
        hosts.push(host);
    }
    hosts
}
```

Resulting allow-list: production adds `api.tokenoverflow.io`; local and unit_test
stay loopback-only (their base URLs are already loopback). Each environment
admits whatever host its own `config.api.base_url` resolves to, so a future
environment is covered automatically with no code change.

**Pros:**
- Single source of truth: the public host is defined once, in the same config
  the rest of the app already trusts for its origin.
- Fixes production with zero new config surface and no new env var; any future
  environment is covered automatically since the host comes from its own base_url.
- Loopback is retained, so local development and the existing test/e2e harness
  cannot regress.

**Cons:**
- Relies on `base_url` being a parseable absolute URL (it always is; it is also
  used to build OAuth proxy URLs).

**Rationale:** Lowest-surface fix that is correct in every environment and keeps
one source of truth. Selected.

#### ❌ Option 2: Add an explicit `mcp.allowed_hosts` config field

Introduce a new list field in `McpConfig` and set it per environment TOML.

```toml
[mcp]
base_url = "https://api.tokenoverflow.io/mcp"
allowed_hosts = ["api.tokenoverflow.io", "localhost", "127.0.0.1", "::1"]
```

**Pros:**
- Fully explicit; supports hosts unrelated to `base_url` if ever needed.

**Cons:**
- New config surface to maintain across every environment file.
- Duplicates the host already encoded in `base_url`; the two can silently drift.
- List overrides via `TOKENOVERFLOW__MCP__ALLOWED_HOSTS` env vars parse awkwardly
  through the `config` crate.

**Rationale:** Rejected. Adds maintenance burden and a drift risk for a value we
already hold. No current consumer needs a host outside the configured origin.

#### ❌ Option 3: Disable the guard with `disable_allowed_hosts()`

Clear the allow-list so any `Host` is accepted.

```rust
let mcp_config = StreamableHttpServerConfig::default()
    .with_stateful_mode(false)
    .with_json_response(true)
    .disable_allowed_hosts();
```

**Pros:**
- One line; never needs touching again.

**Cons:**
- Removes DNS-rebinding protection entirely; rmcp's own docs say this is "NOT
  recommended for public deployments."

**Rationale:** Rejected. Trades a security control for convenience when a precise
allow-list is just as easy.

### How should the path-scoped PRM handler produce the `.../mcp` resource value?

#### ✅ Option 1: Derive it in the handler from `api_base_url`

The handler builds the resource identifier as `format!("{}/mcp", api_base_url)`,
the same way the sibling `oauth_authorization_server` handler in this file already
builds its proxy URLs (`format!("{}/oauth2/token", api)`). No new state is added.

```rust
pub async fn oauth_protected_resource_mcp(
    State(state): State<AppState>,
) -> Json<ProtectedResourceMetadata> {
    let resource = format!("{}/mcp", state.api_base_url.trim_end_matches('/'));
    Json(protected_resource_metadata(resource, state.api_base_url.clone()))
}
```

**Pros:**
- Spec-correct (RFC 9728 §3.3): advertises `https://api.tokenoverflow.io/mcp`.
- Mirrors the established `format!`-from-`api_base_url` convention already in this
  exact file; adds no `AppState` field and no churn across `AppState::new` call
  sites.
- Keeps a single origin source (`api_base_url`) for both the host allow-list and
  the advertised resource, so the two cannot drift.

**Cons:**
- Hard-codes the `/mcp` mount path in the handler, which also appears in the
  router's `nest_service("/mcp", ...)`.

**Rationale:** Smallest change that is spec-correct and matches the file's own
convention. Token-audience validation is decoupled from the resource value
(WorkOS pins `aud` to a fixed client ID and ignores RFC 8707), so the advertised
resource is audience-safe. Selected.

#### ❌ Option 2: Add a `mcp_base_url` field to `AppState`

Thread `config.mcp.base_url` (already `https://api.tokenoverflow.io/mcp`) through
`AppState` and read it in the handler.

```rust
Json(protected_resource_metadata(
    state.mcp_base_url.clone(),
    state.api_base_url.clone(),
))
```

**Pros:**
- Reuses the canonical `config.mcp.base_url` value verbatim.

**Cons:**
- Adds a 13th positional `String` argument to `AppState::new`, directly adjacent
  to `api_base_url` (transposition hazard), and edits every call site including
  ~10 mock builders.
- `config.mcp.base_url` has no other `src/` consumer today, so this introduces a
  second origin source that can drift from `api_base_url`.

**Rationale:** Rejected. The churn and drift risk are not justified for a value
the handler derives in one line.

#### ❌ Option 3: Mirror the root document (`resource = api_base_url`)

Serve the path-scoped URL but leave `resource` pointing at the origin.

**Cons:**
- Violates RFC 9728 §3.3: a path-scoped document's `resource` must be the
  path-scoped identifier, not the origin. A strict client would reject the
  mismatch, defeating the point of registering the route.

**Rationale:** Rejected as non-compliant.

## Architecture Overview

```
Claude Code (MCP client)
        |
        v
API Gateway HTTP API  ──  GET /.well-known/{proxy+}  (greedy)
        |                 POST /mcp
        v
Axum Router (Lambda)
  ├─ public:  GET /.well-known/oauth-protected-resource        (root PRM)
  │           GET /.well-known/oauth-protected-resource/mcp     (NEW: path-scoped PRM)
  └─ mcp_router: nest_service("/mcp", StreamableHttpService)
                 route_layer = jwt_auth_layer
                        |
                        v
                 rmcp StreamableHttpService
                   └─ DNS-rebinding guard: allowed_hosts
                      [localhost,127.0.0.1,::1, <public host from base_url>]  (FIX)
```

Two independent edits: (1) the MCP transport config gains a derived host
allow-list so an authenticated `POST /mcp` clears the guard and is served (the
JWT route-layer runs ahead of the guard); (2) a new public GET route answers the
RFC 9728 path-scoped metadata probe. Neither edit changes request flow for any
other route.

## Third Party Dependencies

No new dependencies. The fix uses capabilities already present in the locked
`rmcp` version and the `http` crate already imported by `server.rs`.

|Capability|Chosen|Alternatives considered|Why|
|-|-|-|-|
|Host allow-list on MCP transport|`rmcp` `StreamableHttpServerConfig::with_allowed_hosts` (built-in)|Hand-rolled tower middleware in front of the service|rmcp already implements the guard and 403 semantics; re-implementing duplicates and risks divergence.|
|URL host extraction|`http::Uri` (already a dependency)|`url` crate|`http::Uri` is already imported and sufficient to read the host component.|

## Structure

```
apps/api/src/
  api/
    server.rs              # add pub fn mcp_allowed_hosts(base_url); call
                           #   .with_allowed_hosts(mcp_allowed_hosts(&config.api.base_url))
    routes/
      well_known.rs        # add oauth_protected_resource_mcp handler + private
                           #   protected_resource_metadata(resource, auth_server) builder
      configure.rs         # register GET /.well-known/oauth-protected-resource/mcp
apps/api/tests/
  unit/api/
    test_server.rs         # NEW: mcp_allowed_hosts contents + host-guard behavior
    routes/test_well_known.rs # add path-scoped PRM test
```

No `AppState` or mock-builder change: the handler derives the resource from the
existing `api_base_url`. `mcp_allowed_hosts` is `pub` so the integration test can
import it; `api::server` is a public module and `TokenOverflowServer::new` is
already public, so the host-guard test can build the service directly.

## Specs & Standards

- **RFC 9728 (OAuth 2.0 Protected Resource Metadata):**
    - §3 / §3.1 — path-scoped well-known URL construction: the
      `/.well-known/oauth-protected-resource` segment is inserted between the
      resource's host and its path component, giving
      `/.well-known/oauth-protected-resource/mcp`.
    - §3.3 — the `resource` field MUST be the resource identifier the document
      describes; the path-scoped document therefore advertises the `/mcp` URL.
- **MCP Streamable HTTP transport security:** servers SHOULD validate the `Host`
  header to prevent DNS rebinding; rmcp implements this as the `allowed_hosts`
  guard we are configuring (not disabling).
- **RFC 8707 (Resource Indicators):** explicitly NOT in effect; WorkOS pins token
  `aud` to a fixed client ID and ignores the `resource` parameter, which is why
  changing the advertised `resource` value is audience-safe.
- **RFC 6454 (Origin):** referenced only to note `Origin` validation stays out of
  scope.

## Interfaces

**New: `GET /.well-known/oauth-protected-resource/mcp`** (public, no auth)

Response `200 application/json`, shape identical to the existing root document
(`ProtectedResourceMetadata`), differing only in `resource`:

```json
{
  "resource": "https://api.tokenoverflow.io/mcp",
  "authorization_servers": ["https://api.tokenoverflow.io"],
  "bearer_methods_supported": ["header"],
  "scopes_supported": ["openid", "profile", "offline_access"]
}
```

- `resource` — RFC 9728 §3.3, the MCP endpoint URL (`format!("{}/mcp", api_base_url)`).
- `authorization_servers` — the OAuth proxy origin (`api_base_url`), unchanged
  from the root document.

**Changed contract: `POST /mcp`** (existing route, no signature change)

The transport now admits `Host: <public host>` in addition to loopback. Because
`jwt_auth_layer` is a route-layer that runs ahead of the transport, an
unauthenticated request still gets `401` at the auth layer and never reaches the
guard. An authenticated request that previously hit the guard and returned `403`
(host-rejected) now passes it and is served normally. No request body or header
contract changes for callers.

## Existing Code & Reuse

- `well_known.rs` already defines `ProtectedResourceMetadata` and the root
  `oauth_protected_resource` handler; the new handler reuses the struct and a
  shared private builder rather than duplicating field construction.
- `oauth_authorization_server` in the same file already derives URLs via
  `format!("{}/...", api_base_url)`; the new handler follows that convention to
  produce the `/mcp` resource, so no new state is needed.
- `configure.rs` already registers the root well-known route in the `public`
  router; the new route is a sibling line.
- `server.rs` already imports `http` and constructs `StreamableHttpServerConfig`;
  the only change is extracting `mcp_allowed_hosts` and adding
  `.with_allowed_hosts(...)`.
- The existing `tests/common` oneshot harness (`get_request`, `read_json`,
  `create_mock_app_state`, `fake_auth_layer`) and the `tests/e2e/mcp` reqwest
  pattern cover the new tests with no new utilities.

## Logic

Host allow-list, extracted as a `pub` function so it is unit-testable without
booting the server (body shown in Key Decision 1). `async_run` builds the config
inline:

```rust
let mcp_config = StreamableHttpServerConfig::default()
    .with_stateful_mode(false)
    .with_json_response(true)
    .with_allowed_hosts(mcp_allowed_hosts(&config.api.base_url));
```

`with_allowed_hosts` accepts any `IntoIterator<Item: Into<String>>`, so the
`Vec<String>` from `mcp_allowed_hosts` passes directly. The config is built via
the builder, never a struct literal, because `StreamableHttpServerConfig` is
`#[non_exhaustive]`.

Shared PRM builder so both handlers stay in sync; the path-scoped handler passes
the derived `/mcp` resource:

```rust
fn protected_resource_metadata(
    resource: String,
    auth_server: String,
) -> ProtectedResourceMetadata {
    ProtectedResourceMetadata {
        resource,
        authorization_servers: vec![auth_server],
        bearer_methods_supported: vec!["header".to_string()],
        scopes_supported: ["openid", "profile", "offline_access"]
            .map(String::from)
            .to_vec(),
    }
}
```

## Edge Cases & Constraints

- **Bare-host port independence:** rmcp matches a bare allowed host against any
  incoming port, so a `Host` with or without an explicit port (e.g. `:443` from
  the gateway) still matches; no port needs to be enumerated.
- **Case-insensitive match:** rmcp's `normalize_host` lowercases both sides, so
  `Host: API.TokenOverflow.io` is admitted; no extra handling needed.
- **`api.base_url` and `mcp.base_url` must share a host:** the allow-list derives
  from `api.base_url` while clients connect to the `mcp.base_url` host; all
  current envs share a host and a test asserts the public host is admitted, but
  pointing `mcp.base_url` at a different host would re-trigger the 403.
- **IPv6 loopback (`::1`):** retained explicitly; some local clients connect over
  IPv6.
- **Unparseable `base_url`:** if parsing yields no host, the list stays
  loopback-only (the public host is simply not added) rather than panicking; in
  practice `base_url` is always a valid absolute URL.
- **Trailing-dot FQDN (`api.tokenoverflow.io.`):** not normalized by rmcp and so
  not admitted; knowingly out of scope as no real client sends it.
- **Origin validation stays off:** leaving `allowed_origins` empty is acceptable
  because `/mcp` is a non-browser, Bearer-authenticated, POST-only JSON endpoint
  with no ambient credentials to abuse via CSRF, and the `Host` guard already
  blocks DNS rebinding.
- **Gateway greediness:** `GET /.well-known/{proxy+}` already forwards the
  path-scoped request to Lambda, so the new route is reachable without any
  Terraform change (per RFC 9728 §3.1 the suffix is part of the path).
- **Audience safety:** advertising `resource = .../mcp` does not affect token
  validation because `aud` is pinned to the WorkOS client ID (RFC 8707 ignored).

## Test Plan

- **Unit (host allow-list):** assert `mcp_allowed_hosts("https://api.tokenoverflow.io")`
  contains `api.tokenoverflow.io`, `localhost`, `127.0.0.1`, `::1`, and does NOT
  contain a foreign host (e.g. `evil.com`). Pure function; no server boot.
- **Integration (host-guard behavior):** build the `/mcp` rmcp service with
  `.with_allowed_hosts(mcp_allowed_hosts("https://api.tokenoverflow.io"))` over
  a mock `TokenOverflowServer`, then oneshot `POST /mcp`:
  `Host: api.tokenoverflow.io`
  must NOT return `403` (guard admits it) and `Host: evil.com` must return `403`
  (guard rejects it). Tested on the service in isolation (no auth layer), so the
  only possible `403` is the host guard.
- **Unit (path-scoped PRM):** oneshot `GET /.well-known/oauth-protected-resource/mcp`
  against a router with the new handler; assert `200`, `resource` equals
  `<api_base_url>/mcp`, and `authorization_servers[0]` equals the mock
  `api_base_url`. Mirrors the existing root-document tests.
- **Regression (existing):** the root PRM and authorization-server metadata tests
  are unchanged and must still pass; the e2e Docker harness runs with
  `TOKENOVERFLOW_ENV=local`, so the rmcp client sends `Host: localhost`, which
  remains allowed and cannot regress.
- **Post-deploy:** re-run `/mcp` in Claude Code; confirm tools load, `POST /mcp`
  returns `200` in the gateway logs, and the DNS-rebinding WARN no longer fires.
  (A no-token `curl` to `/mcp` returns `401` at `jwt_auth_layer` before the host
  guard, so it cannot distinguish the fix; the guard is only reachable after
  auth, which is why the automated test exercises the service directly.)
- Cross-reference Out of Scope: no `Origin`/CORS assertions and no RFC 8707
  audience tests are added.

## Documentation Changes

- Update the route table in `apps/api/src/CLAUDE.md` to list the new
  `GET /.well-known/oauth-protected-resource/mcp` public route.
- No `README.md` change; the fix is transparent to end users (the MCP endpoint
  URL and setup steps are unchanged).

## Development Environment Changes

None. No new environment variable, no `Brewfile` or setup-script change; local
and test environments already use loopback hosts that remain allowed.

## Tasks

```
Task 1 (host allow-list) ──┐
                           ├── independent, parallelizable
Task 2 (path-scoped PRM) ──┘
```

Both tasks are independent vertical slices and can be implemented and reviewed in
parallel; neither depends on the other.

|#|Task Name|Task Description|Success Criteria|Dependencies|
|-|-|-|-|-|
|1|MCP host allow-list|Extract `pub fn mcp_allowed_hosts(base_url)` in `server.rs`, derive the allow-list from `config.api.base_url` plus loopback, and call `.with_allowed_hosts(mcp_allowed_hosts(&config.api.base_url))`.|Unit test asserts `mcp_allowed_hosts` contains the public host + loopback and excludes a foreign host; integration test asserts `POST /mcp` with `Host: api.tokenoverflow.io` is not `403` and with `Host: evil.com` is `403`; `cargo build`, `clippy -D warnings`, and `cargo test` pass.|None|
|2|Path-scoped PRM route|Add the `oauth_protected_resource_mcp` handler (`resource = format!("{}/mcp", api_base_url)`) plus the shared `protected_resource_metadata` builder in `well_known.rs`, register the route in `configure.rs`, and update the `apps/api/src/CLAUDE.md` route table.|`GET /.well-known/oauth-protected-resource/mcp` returns 200 with `resource` = the `/mcp` URL and `authorization_servers[0]` = the origin; new unit test passes; existing well-known tests still pass.|None|

```
