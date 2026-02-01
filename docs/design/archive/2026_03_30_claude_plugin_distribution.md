# Design: Claude Plugin Distribution

## Architecture Overview

This design covers building, testing, and distributing the TokenOverflow Claude
Code plugin. The plugin enables end-to-end automated integration: users install
it, authenticate via GitHub OAuth (MCP OAuth 2.1), and Claude automatically
searches TokenOverflow for solutions, applies them, and submits new solutions
back, all without manual prompting.

### Directory Placement Decision

The Claude plugin originally lived at `apps/claude/`. It has been moved to
`integrations/claude/` to separate integration code from application code.

### Refactor: `apps/claude/` to `integrations/claude/` (DONE)

The refactor is complete. The following was performed:

1. `mkdir -p integrations && git mv apps/claude integrations/claude` (8 files)
2. `git mv integrations/claude/hooks/settings.json integrations/claude/hooks/hooks.json`
3. Monorepo layout diagram in `README.md` updated
4. `marketplace.json` uses `integrations/claude/` as the source path

`apps/claude/` is not referenced anywhere outside of immutable historical design
docs.

### What Already Exists

The `integrations/claude/` directory contains a fully-built plugin with all
behavioral components:

```
integrations/claude/
+-- .claude-plugin/
|   +-- plugin.json           # Plugin manifest (name, description)
+-- .mcp.json                 # MCP server config (env-var-driven URL + auth header)
+-- instructions.md           # Behavioral rules injected into Claude's context
+-- skills/
|   +-- search-tokenoverflow/
|   |   +-- SKILL.md          # "Search first" skill
|   +-- submit-to-tokenoverflow/
|       +-- SKILL.md          # "Submit solution" skill
+-- agents/
|   +-- tokenoverflow-researcher.md  # Subagent for knowledge base lookup
+-- hooks/
|   +-- hooks.json            # PreToolUse, PostToolUse, Stop hooks
+-- tags.md                   # Canonical tag list for submissions
```

This plugin works today when loaded via
`claude --plugin-dir ./integrations/claude`. What is missing is the production
authentication fix (see "Production Auth Fixes" below) and the marketplace
distribution pipeline.

### Distribution Decision: Monorepo as Marketplace (Option B)

The monorepo itself serves as the Claude Code marketplace. A
`.claude-plugin/marketplace.json` at the repo root points to
`./integrations/claude` as the plugin source. No separate repository, no CI
sync, no duplication.

Users install with:

```
/plugin marketplace add token-overflow/tokenoverflow
/plugin install tokenoverflow@tokenoverflow-marketplace
```

### What This Design Adds

Three things are needed to complete the end-user story:

1. **Marketplace distribution**: Add `.claude-plugin/marketplace.json` at the
   monorepo root pointing to `./integrations/claude`.

2. **Production auth fixes**: Fix the server-side configuration so that the
   MCP OAuth 2.1 flow completes against production (see "Production Auth
   Fixes" section below). The key simplification is shipping a pre-configured
   `oauth.clientId` in the plugin's `.mcp.json`, which lets Claude Code skip
   Dynamic Client Registration entirely and gives the API Gateway JWT
   authorizer a single predictable audience to validate.

3. **Local development workflow**: Document how developers test the plugin
   locally against the docker-compose stack before publishing.

### End-to-End Flow

```
1. INSTALL
   User runs:  /plugin marketplace add token-overflow/tokenoverflow
   User runs:  /plugin install tokenoverflow@tokenoverflow-marketplace

2. AUTHENTICATE (eager, on plugin load)
   Claude Code probes the MCP URL -> Gateway returns 401 (WWW-Authenticate: Bearer)
   -> MCP client sees no resource_metadata in header
   -> MCP client falls back to probing /.well-known/oauth-protected-resource
   -> discovers our API as the authorization server (proxy)
   -> MCP client fetches /.well-known/oauth-authorization-server from our API
   -> our API returns metadata with proxy URLs for authorize/token endpoints
   -> MCP client reads oauth.clientId from .mcp.json, skips DCR
   -> opens browser to our /oauth2/authorize proxy
   -> proxy injects scope=openid+profile and redirects (302) to AuthKit
   -> user logs in with GitHub via AuthKit (PKCE flow)
   -> AuthKit redirects back with auth code
   -> MCP client exchanges code at our /oauth2/token proxy
   -> proxy forwards to AuthKit, returns the token response
   -> token is cached and refreshed automatically by the MCP client
   -> plugin shows as enabled with green checkmark

3. USE (fully automatic, no user intervention)
   User: "Fix this compilation error: ..."
   Claude: calls search_questions (authenticated) -> finds solution -> applies it
           -> calls upvote_answer -> done.
   OR
   Claude: calls search_questions -> no results -> solves via other means
           -> calls submit (authenticated) -> solution stored for future agents.
```

### Production Auth Fixes (NEW)

Testing the MCP OAuth 2.1 flow against production
(`https://api.tokenoverflow.io/mcp`) revealed three issues that prevent
authentication from completing. The error observed by the MCP client is:

```
subject_types_supported: expected array, received undefined
id_token_signing_alg_values_supported: expected array, received undefined
```

The root cause is a chain of three problems in the production auth setup. Each
is described below with the evidence from the MCP spec and the MCP TypeScript
SDK source.

#### Issue 1: Wrong `authorization_servers` URL

**Symptom**: The `/.well-known/oauth-protected-resource` endpoint returns the
WorkOS OIDC issuer URL (`https://api.workos.com/user_management/client_01KKZ...`)
as the authorization server, instead of the AuthKit domain.

**Root cause**: The `well_known.rs` handler populates `authorization_servers`
from `state.auth_config.issuer`. In `production.toml`, the `issuer` field is
set to `https://api.workos.com/user_management/client_01KKZDZQ26HJSBXSWQRSWABFMX`.
This is the JWT issuer (the `iss` claim in tokens), not the AuthKit domain that
serves authorization server metadata.

**Why it fails**: The MCP client takes the first entry in `authorization_servers`
and tries to fetch its metadata. Per the MCP spec (RFC 8414 fallback order),
it tries:

1. `GET https://api.workos.com/.well-known/oauth-authorization-server/user_management/client_01KKZ...`
   Returns 404.
2. `GET https://api.workos.com/.well-known/openid-configuration/user_management/client_01KKZ...`
   Returns 404.
3. `GET https://api.workos.com/user_management/client_01KKZ.../.well-known/openid-configuration`
   Returns a partial metadata document (see Issue 2).

**Evidence**: The production response at step 3 is:

```json
{
  "issuer": "https://api.workos.com/user_management/client_01KKZDZQ26HJSBXSWQRSWABFMX",
  "authorization_endpoint": "https://api.workos.com/user_management/authorize",
  "token_endpoint": "https://api.workos.com/user_management/authenticate",
  "response_types_supported": ["code"],
  "jwks_uri": "https://api.workos.com/sso/jwks/client_01KKZDZQ26HJSBXSWQRSWABFMX"
}
```

This is missing the fields required by the OpenID Connect Discovery schema
(`subject_types_supported`, `id_token_signing_alg_values_supported`) and also
missing PKCE-related fields (`code_challenge_methods_supported`).

**Fix**: The `authorization_servers` in the Protected Resource Metadata must
point to our own API (`https://api.tokenoverflow.io`), not directly to AuthKit
or the OIDC issuer URL. Our API serves an OAuth authorization proxy that
mirrors AuthKit's metadata but with proxy URLs (see section 5e). This is
needed because Claude Code has a known bug where it omits the `scope`
parameter from authorization requests (see section 5e for details).

This requires a new config field because the `issuer` (JWT `iss` claim) and
the `authorization_servers` URL are different values in WorkOS.

#### Issue 2: MCP SDK OpenID validation requires missing fields

**Symptom**: The MCP client's Zod validation fails with "expected array, received
undefined" for `subject_types_supported` and `id_token_signing_alg_values_supported`.

**Root cause**: The MCP TypeScript SDK (`@modelcontextprotocol/sdk`) defines
two metadata schemas:

- `OAuthMetadataSchema` (for RFC 8414 `oauth-authorization-server` responses):
  requires `issuer`, `authorization_endpoint`, `token_endpoint`, and
  `response_types_supported`. Does NOT require `subject_types_supported` or
  `id_token_signing_alg_values_supported`.

- `OpenIdProviderDiscoveryMetadataSchema` (for `openid-configuration` responses):
  extends the OAuth schema and additionally requires `jwks_uri`,
  `subject_types_supported` (array), and `id_token_signing_alg_values_supported`
  (array).

Since the MCP client falls back to `openid-configuration` (after
`oauth-authorization-server` returns 404), it validates the response against the
stricter OpenID schema. The WorkOS user_management `openid-configuration`
endpoint omits the two required fields, causing Zod to reject it.

**Fix**: By pointing `authorization_servers` to our own API (Issue 1 fix), the
MCP client will find a working `oauth-authorization-server` endpoint on the
first attempt (served by our OAuth proxy). The proxy fetches AuthKit's metadata
and returns it with our proxy URLs. The `openid-configuration` fallback path
is never reached.

#### Issue 3: API Gateway 401 lacks `resource_metadata` in `WWW-Authenticate`

**Symptom**: The production 401 response returns `WWW-Authenticate: Bearer`
without the `resource_metadata` parameter. The MCP client cannot discover the
Protected Resource Metadata URL from the header.

**Root cause**: Two sub-issues:

1. **API Gateway intercepts the 401 before the Axum app runs.** The HTTP API
   Gateway JWT authorizer rejects requests with missing/invalid tokens and
   returns its own 401. This response only contains `WWW-Authenticate: Bearer`
   with no extra parameters. The Axum `error.rs` handler (which adds
   `resource_metadata`) never executes because the request never reaches Lambda.

2. **The Axum `error.rs` uses a relative URL.** Even when the Axum handler does
   execute, it sets `resource_metadata="/.well-known/oauth-protected-resource"`
   (relative). RFC 9728 Section 5.1 specifies that the `resource_metadata`
   value is a URL, and the MCP SDK's `SafeUrlSchema` validator requires a
   parseable absolute URL.

**Fix**: The API Gateway issue cannot be solved by changing the JWT authorizer
behavior (AWS does not support custom headers on Gateway-level 401 responses).
However, the MCP spec (draft, "Protected Resource Metadata Discovery
Requirements") states that servers MUST implement one of two discovery
mechanisms: either the `WWW-Authenticate` header OR the well-known URI. MCP
clients MUST support both and fall back to the well-known URI when the header
is absent. So the missing header is not a blocker if the well-known URI works.

The MCP client fallback behavior (when `resource_metadata` is absent from the
header) is to probe the well-known URI directly:

1. `GET /.well-known/oauth-protected-resource/mcp` (path-aware)
2. `GET /.well-known/oauth-protected-resource` (root)

Both routes match the `GET /.well-known/{proxy+}` API Gateway route, which has
no authorizer. The request reaches Lambda, and the Axum handler returns the
Protected Resource Metadata. This fallback path already works in production.

The Axum `error.rs` fix (relative to absolute URL) is still needed for
correctness, but is not blocking. The fix is to change the header value from
`"Bearer resource_metadata=\"/.well-known/oauth-protected-resource\""` to
`format!("Bearer resource_metadata=\"{}/{}\"", base_url, ".well-known/oauth-protected-resource")`
where `base_url` comes from the API config (e.g., `https://api.tokenoverflow.io`).

## Interfaces

This section defines the exact file contents for the new or modified files
that enable marketplace distribution and the production auth fix.

### 1. New file: `.claude-plugin/marketplace.json` (repo root)

This file lives at the monorepo root (not inside the plugin directory). It
registers the monorepo as a Claude Code marketplace and lists the plugin(s)
available for installation.

```json
{
  "name": "tokenoverflow-marketplace",
  "owner": {
    "name": "TokenOverflow",
    "email": "hello@tokenoverflow.io"
  },
  "metadata": {
    "description": "Stack Overflow for AI coding agents."
  },
  "plugins": [
    {
      "name": "tokenoverflow",
      "source": "./integrations/claude",
      "description": "Automatically search and contribute to the TokenOverflow knowledge base for AI coding agents.",
      "version": "0.0.1"
    }
  ]
}
```

Field notes:

- `name` - The marketplace identifier. Users reference this in install commands
  (`/plugin install tokenoverflow@tokenoverflow-marketplace`).
- `owner.name` / `owner.email` - Attribution shown in the plugin manager UI.
  `owner.name` is required; `email` is optional.
- `metadata.description` - Optional. Shown when browsing marketplaces.
- `plugins[].name` - Must match the `name` in the plugin's own `plugin.json`.
  This is the identifier users type in install commands.
- `plugins[].source` - Relative path from the repo root to the plugin
  directory. Must start with `./`. Resolved relative to the directory containing
  `.claude-plugin/`, which is the repo root.
- `plugins[].version` - Bumped to `0.1.0` to mark the first distributable
  release. This is where we manage the version going forward (not in
  `plugin.json`) because the plugin source is a relative path within the same
  repo.

### 2. Modified file: `integrations/claude/.claude-plugin/plugin.json` (DONE)

The plugin manifest. No `userConfig` and no `version` field.

```json
{
  "name": "tokenoverflow",
  "description": "Automatically search and contribute to the TokenOverflow knowledge base for AI coding agents.",
  "author": {
    "name": "TokenOverflow"
  },
  "homepage": "https://tokenoverflow.io",
  "repository": "https://github.com/token-overflow/tokenoverflow",
  "license": "MIT",
  "keywords": [
    "knowledge_base",
    "mcp",
    "agent_memory",
    "coding_solutions"
  ]
}
```

Field notes:

- No `userConfig` - The MCP URL uses an environment variable with a default
  value in `.mcp.json`. End users configure nothing; they just install and
  authenticate via MCP OAuth 2.1. This matches how all official Claude Code
  plugins (GitHub, Greptile, etc.) handle their MCP URLs.
- `version` removed - Per the Claude Code docs, avoid setting version in both
  `plugin.json` and `marketplace.json`. Since the plugin uses a relative-path
  source within the marketplace repo, we set it in `marketplace.json`.
- `repository` updated - Points to the actual monorepo
  (`token-overflow/tokenoverflow`), not the old placeholder URL.

### 3. Modified file: `integrations/claude/.mcp.json` (DONE)

The MCP URL is driven by an environment variable with a production default.
A separate environment variable provides the `Authorization` header for local
development. The `oauth.clientId` field provides a pre-configured client ID
so Claude Code skips Dynamic Client Registration.

```json
{
  "mcpServers": {
    "tokenoverflow": {
      "type": "http",
      "url": "${TOKENOVERFLOW_MCP_URL:-https://api.tokenoverflow.io/mcp}",
      "headers": {
        "Authorization": "${TOKENOVERFLOW_MCP_AUTH:-}"
      },
      "oauth": {
        "clientId": "client_01KN3MGDJEZSGSXWH8YKKDCB2T"
      }
    }
  }
}
```

Field notes:

- `url` uses `${TOKENOVERFLOW_MCP_URL:-...}` - The default is the production
  URL. Developers override this env var to point at local or dev environments.
  End users never touch this; the default just works.
- `type: "http"` - The MCP server uses HTTP Streamable transport.
- `headers.Authorization` uses `${TOKENOVERFLOW_MCP_AUTH:-}` - Empty by
  default (production uses MCP OAuth 2.1 for auth). For local development,
  developers set this to `Bearer <test-jwt>` from `dev_token.sh`. When the
  value is an empty string, the MCP client ignores the header and proceeds
  with its normal OAuth discovery flow.
- `oauth.clientId` - Pre-configured WorkOS client ID for the public PKCE app
  (`TokenOverflow MCP`). When present, Claude Code uses this client ID
  directly instead of performing Dynamic Client Registration (DCR). This is
  the public (no secret) OAuth application registered on WorkOS AuthKit
  specifically for Claude Code. The access tokens it produces still carry
  `aud: client_01KKZDZQ26HJSBXSWQRSWABFMX` (the environment-level client
  ID), which is what the API Gateway JWT authorizer validates.

#### Local development with env vars

Developers test the plugin locally by setting environment variables:

```bash
export TOKENOVERFLOW_MCP_URL=http://localhost:8080/mcp
export TOKENOVERFLOW_MCP_AUTH="Bearer $(./scripts/dev_token.sh)"
claude --plugin-dir ./integrations/claude
```

The env var approach replaces the previous root `.mcp.json` (which has been
deleted). This avoids having two MCP servers competing for the same tool
names and simplifies the developer workflow to a single MCP connection.

| Environment | TOKENOVERFLOW_MCP_URL | TOKENOVERFLOW_MCP_AUTH | Auth method |
|---|---|---|---|
| Production (end user) | not set (default) | not set (empty) | MCP OAuth 2.1 |
| Local dev | `http://localhost:8080/mcp` | `Bearer <test-jwt>` | Static JWT |
| Dev/staging cloud | `https://api.dev.tokenoverflow.io/mcp` | not set (empty) | MCP OAuth 2.1 |

### 4. Rename: `hooks/settings.json` to `hooks/hooks.json` (DONE)

The hooks file was renamed from `hooks/settings.json` to `hooks/hooks.json`.
The Claude Code plugin system expects hooks at `hooks/hooks.json` (the default
location per the plugin reference docs). The file contents are unchanged.

### 5. Server-side auth fixes

The production environment previously used the WorkOS `user_management`
endpoint as the JWT issuer for REST API clients, while MCP clients (Claude
Code) would authenticate through the WorkOS AuthKit domain. This caused JWT
validation mismatches because tokens issued by AuthKit carried a different
`iss` claim than what the API Gateway and Axum expected.

The fix is a single-issuer, single-client approach:

- **Single issuer**: Everything uses WorkOS AuthKit
  (`https://intimate-figure-17.authkit.app`) as the sole OAuth provider. REST
  API clients (Bruno, web app) and MCP clients (Claude Code) all authenticate
  through the same AuthKit instance and receive tokens with the same `iss`
  claim.
- **Single audience**: The plugin ships with a pre-configured `oauth.clientId`
  (`client_01KN3MGDJEZSGSXWH8YKKDCB2T`) in its `.mcp.json`. Claude Code sees
  this and skips DCR entirely. All JWTs carry
  `aud: client_01KKZDZQ26HJSBXSWQRSWABFMX` (the environment-level audience),
  giving the API Gateway JWT authorizer a single predictable audience to
  validate.
- **No DCR**: No Dynamic Client Registration, no abandoned client registrations
  on WorkOS, no dual-issuer validation in Axum.

#### 5a. Gateway JWT authorizer: switch to AuthKit issuer

**File**: `infra/terraform/live/prod/api_gateway/terragrunt.hcl`

Change `jwt_issuer` from the WorkOS `user_management` URL to the AuthKit
domain:

```hcl
jwt_issuer = "https://intimate-figure-17.authkit.app"
```

This ensures the API Gateway accepts tokens issued by AuthKit. Tokens from
the old `user_management` issuer are no longer accepted at the Gateway level.

#### 5b. Gateway routes: public routes for OAuth discovery and proxy

**File**: `infra/terraform/modules/api_gateway/routes.tf`

Because the plugin ships a pre-configured `client_id`, all JWTs have a
predictable audience. The Gateway JWT authorizer can now validate every
request, including `/mcp`. No public `/mcp` routes are needed.

The `$default` catch-all route (which has the JWT authorizer attached)
handles all API and MCP traffic. The routes without an authorizer are:

- `GET /health` - health check
- `GET /.well-known/{proxy+}` - OAuth discovery (protected resource metadata,
  authorization server metadata)
- `GET /oauth2/authorize` - OAuth authorization proxy (redirects to AuthKit)
- `POST /oauth2/token` - OAuth token proxy (forwards to AuthKit)
- `POST /oauth2/register` - OAuth DCR proxy (forwards to AuthKit)

The `/oauth2/*` routes must be public because they are part of the OAuth flow
itself. The MCP client calls these endpoints before it has a token.

MCP OAuth discovery still works because:

1. Gateway returns `401` with `WWW-Authenticate: Bearer` for unauthenticated
   `/mcp` requests.
2. The MCP client sees no `resource_metadata` in the header and falls back to
   probing `/.well-known/oauth-protected-resource` (a public route, no
   authorizer).
3. The Protected Resource Metadata returns our own API as the authorization
   server. The MCP client fetches `/.well-known/oauth-authorization-server`
   from our API, which returns metadata with proxy URLs.
4. The MCP client reads the `oauth.clientId` from `.mcp.json`, skips DCR,
   and opens the browser to our `/oauth2/authorize` proxy.
5. Our proxy injects the missing scopes and redirects (302) to AuthKit.
6. After login, all requests carry a valid JWT and pass the Gateway authorizer.

#### 5c. Axum config: switch issuer and JWKS to AuthKit

**File**: `apps/api/config/production.toml` and `apps/api/config/development.toml`

Change the `issuer` and `jwks_url` fields from the WorkOS `user_management`
endpoints to the AuthKit equivalents:

```toml
issuer = "https://intimate-figure-17.authkit.app"
jwks_url = "https://intimate-figure-17.authkit.app/oauth2/jwks"
```

The `local.toml` and `unit_test.toml` files are unchanged (they use test keys).

This ensures the Axum JWT validation middleware accepts tokens issued by
AuthKit and fetches the correct signing keys.

#### 5d. Route configuration summary

The following table shows how each route is handled at the Gateway and
application layers:

| Route | Gateway Auth | Axum Auth | Purpose |
|-------|-------------|-----------|---------|
| GET /health | None | None | Health check |
| GET /.well-known/{proxy+} | None | None | OAuth discovery |
| GET /oauth2/authorize | None | None | OAuth authorize proxy |
| POST /oauth2/token | None | None | OAuth token proxy |
| POST /oauth2/register | None | None | OAuth DCR proxy |
| $default (includes /mcp) | JWT (AuthKit) | JWT (AuthKit) | All API + MCP (defense-in-depth) |

All authenticated routes (including `/mcp`) go through the `$default` catch-all
with JWT validation at both the Gateway and Axum layers. The OAuth proxy
endpoints, health check, and discovery endpoints are publicly accessible
because they are part of the auth flow itself.

#### 5e. OAuth authorization proxy (scope injection workaround)

**Why this is needed**: Claude Code has a known bug
([anthropics/claude-code#4540](https://github.com/anthropics/claude-code/issues/4540))
where it omits the `scope` parameter from OAuth authorization requests. When
Claude Code sends an authorization request to WorkOS AuthKit without scopes,
AuthKit returns an `invalid_scope` error because it requires at least `openid`
to be present. This prevents the MCP OAuth 2.1 flow from completing.

This is a client-side bug in Claude Code, not an issue with our server or
WorkOS. The workaround (used by projects like
[hyprmcp/mcp-gateway](https://github.com/hyprmcp/mcp-gateway)) is to proxy
the OAuth endpoints through our own API, injecting the missing scopes before
forwarding to AuthKit.

**How the proxy works**: Our API becomes the "authorization server" from the
MCP client's perspective. Instead of the Protected Resource Metadata pointing
Claude Code directly at AuthKit, it points at our own API. We serve an
authorization server metadata document that mirrors AuthKit's but with our
proxy URLs substituted in. When the MCP client hits our authorize endpoint,
we add the missing scopes and redirect to AuthKit. When it hits our token
endpoint, we forward the request to AuthKit and return the response as-is.

The proxy is minimal: it does not store tokens, does not modify responses
(except adding scope to the authorize redirect), and does not inspect token
contents. It is purely a pass-through with scope injection.

**When this proxy can be removed**: Once Anthropic fixes the scope bug in
Claude Code, this proxy is no longer needed. At that point, revert
`authorization_servers` in the Protected Resource Metadata back to the AuthKit
URL directly, remove the proxy routes from the API and Gateway, and delete the
`oauth_proxy.rs` module.

**Proxy endpoints**:

1. **`GET /.well-known/oauth-authorization-server`** - Returns an authorization
   server metadata document (RFC 8414) with our proxy URLs. The response is
   built by taking AuthKit's actual metadata fields and replacing the
   `authorization_endpoint`, `token_endpoint`, and `registration_endpoint`
   with URLs pointing to our API:

   ```json
   {
     "issuer": "https://intimate-figure-17.authkit.app",
     "authorization_endpoint": "https://api.tokenoverflow.io/oauth2/authorize",
     "token_endpoint": "https://api.tokenoverflow.io/oauth2/token",
     "registration_endpoint": "https://api.tokenoverflow.io/oauth2/register",
     "scopes_supported": ["email", "offline_access", "openid", "profile"],
     "response_types_supported": ["code"],
     "response_modes_supported": ["query"],
     "grant_types_supported": ["authorization_code", "refresh_token"],
     "token_endpoint_auth_methods_supported": ["none", "client_secret_post", "client_secret_basic"],
     "code_challenge_methods_supported": ["S256"],
     "jwks_uri": "https://intimate-figure-17.authkit.app/oauth2/jwks",
     "introspection_endpoint": "https://intimate-figure-17.authkit.app/oauth2/introspection",
     "client_id_metadata_document_supported": true
   }
   ```

   The `issuer` and `jwks_uri` still point to AuthKit (tokens are issued by
   AuthKit, not by us). Only the interactive endpoints (authorize, token,
   register) are proxied through our API.

   This endpoint is served from the `well_known.rs` module alongside the
   existing `oauth-protected-resource` handler, since both are well-known
   metadata endpoints.

2. **`GET /oauth2/authorize`** - Receives the authorization request from the
   MCP client. Adds `scope=openid+profile` to the query parameters (or
   appends to existing scopes if any are present). Issues a `302 Found`
   redirect to AuthKit's real authorization endpoint with the modified query
   string. The MCP client's browser follows the redirect transparently.

3. **`POST /oauth2/token`** - Receives the token exchange request from the
   MCP client. Forwards the request body as-is to AuthKit's token endpoint.
   Returns AuthKit's response (status code, headers, body) unchanged. This
   handles both initial code exchange and refresh token requests.

4. **`POST /oauth2/register`** - Forwards Dynamic Client Registration
   requests to AuthKit's registration endpoint, in case the MCP client
   attempts DCR as a fallback (it should not with our `oauth.clientId`, but
   proxying it keeps the metadata document complete).

**What is NOT proxied**: The `jwks_uri` and `introspection_endpoint` point
directly to AuthKit. These are called by our own backend (for JWT validation)
and by other tooling, not by the MCP client during the auth flow. There is no
reason to proxy them.

**Impact on existing flows**: Bruno and the web app authenticate directly
against AuthKit (they use the confidential client and configure AuthKit URLs
in their own settings). They never touch our proxy endpoints. The proxy only
affects MCP clients that discover our API as their authorization server via
the Protected Resource Metadata.

**Implementation location**: The proxy handlers live in a new file
`apps/api/src/api/routes/oauth_proxy.rs`. The authorization server metadata
handler lives in the existing `well_known.rs` module. Routes are registered
in `configure.rs` as public (no auth middleware).

**Config requirements**: The proxy handlers need `api_base_url` (for building
proxy URLs in the metadata response) and `auth_config.authkit_url` (for
knowing where to forward requests). Both are already available on `AppState`.

#### 5f. Protected Resource Metadata: point to our API

**File**: `apps/api/src/api/routes/well_known.rs`

The `authorization_servers` field in the Protected Resource Metadata must now
point to our own API base URL (not directly to AuthKit). This is what makes
the MCP client discover our OAuth proxy instead of going to AuthKit directly.

Change:

```rust
authorization_servers: vec![state.auth_config.authkit_url.clone()],
```

To:

```rust
authorization_servers: vec![state.api_base_url.clone()],
```

This change means the MCP client will fetch
`GET https://api.tokenoverflow.io/.well-known/oauth-authorization-server`
instead of fetching it from AuthKit. Our proxy serves this metadata with
proxy URLs for authorize/token/register.

## Logic

This section describes what happens at each phase of the plugin lifecycle.
Since the plugin is static config files (no compiled code), the logic is
entirely about how Claude Code processes these files and how the MCP client
handles authentication.

### Plugin Installation

When the user runs `/plugin marketplace add token-overflow/tokenoverflow`,
Claude Code clones the GitHub repo and reads
`.claude-plugin/marketplace.json` at the repo root. This registers the
marketplace locally. The marketplace entry persists across sessions in the
user's Claude Code settings.

When the user runs `/plugin install tokenoverflow@tokenoverflow-marketplace`,
Claude Code resolves the `source` field (`./integrations/claude`), reads the
`plugin.json` inside that directory, and copies the plugin into the user's
local plugin store. The plugin is now available but not yet enabled.

### MCP OAuth 2.1 Handshake (Step by Step)

This flow is handled entirely by the Claude Code MCP client. The plugin
provides no auth code. Auth detection is eager: Claude Code probes the MCP URL
at plugin load time, not on first tool call.

1. Plugin loads. The MCP client sends a request to the resolved `url` from
   `.mcp.json` (default: `https://api.tokenoverflow.io/mcp`).
2. API Gateway has no Bearer token on the request, so it returns
   `401 Unauthorized` with header `WWW-Authenticate: Bearer` (no
   `resource_metadata` parameter, because Gateway returns its own 401).
3. The MCP client sees no `resource_metadata` in the header. Per the MCP spec,
   it falls back to probing the well-known URI. It constructs:
   `GET https://api.tokenoverflow.io/.well-known/oauth-protected-resource/mcp`
   (path-aware probe for the `/mcp` resource path).
4. If step 3 returns 404, the client tries the root:
   `GET https://api.tokenoverflow.io/.well-known/oauth-protected-resource`.
   This matches the `GET /.well-known/{proxy+}` Gateway route (no authorizer)
   and reaches the Axum handler.
5. The response contains `authorization_servers` pointing to our own API
   (`https://api.tokenoverflow.io`), not directly to AuthKit.
6. The MCP client fetches the authorization server metadata:
   `GET https://api.tokenoverflow.io/.well-known/oauth-authorization-server`.
   Our API returns a metadata document mirroring AuthKit's, but with proxy
   URLs: `authorization_endpoint` points to
   `https://api.tokenoverflow.io/oauth2/authorize`, `token_endpoint` to
   `https://api.tokenoverflow.io/oauth2/token`, etc.
7. The MCP client validates the metadata against `OAuthMetadataSchema` (Zod).
   All required fields are present. Validation passes.
8. The MCP client reads `oauth.clientId` from `.mcp.json`
   (`client_01KN3MGDJEZSGSXWH8YKKDCB2T`). Since a client ID is already
   provided, the client skips Dynamic Client Registration entirely.
9. The MCP client generates a PKCE `code_verifier` and `code_challenge`
   (S256), then opens the user's browser to our authorize proxy:
   `GET https://api.tokenoverflow.io/oauth2/authorize?response_type=code&client_id=client_01KN3MGDJEZSGSXWH8YKKDCB2T&code_challenge=...&redirect_uri=http://localhost:.../callback&resource=https://api.tokenoverflow.io`
   (note: Claude Code omits the `scope` parameter due to the bug).
10. Our authorize proxy adds `scope=openid+profile` to the query parameters
    and responds with `302 Found` redirecting to AuthKit's real authorization
    endpoint with the full query string including scopes.
11. WorkOS AuthKit presents the GitHub OAuth login page. The user authorizes.
12. WorkOS redirects to the MCP client's local loopback server with an
    authorization code.
13. The MCP client exchanges the code at our token proxy:
    `POST https://api.tokenoverflow.io/oauth2/token`. Our proxy forwards the
    request body to AuthKit's token endpoint and returns the response
    (access token + refresh token).
14. The MCP client caches both tokens. It attaches the access token as
    `Authorization: Bearer <token>` on all subsequent requests to the
    TokenOverflow MCP server.
15. When the access token expires, the MCP client uses the refresh token
    (via our token proxy) to obtain a new one without user interaction.

After step 14, the plugin shows as enabled (green checkmark). All MCP tool
calls are authenticated transparently from this point on.

### Hook Behavior During Normal Usage

The hooks in `hooks/hooks.json` fire automatically during Claude's operation:

- **PreToolUse (WebSearch|WebFetch)**: Before Claude uses web search, the hook
  injects a reminder to check TokenOverflow first via `search_questions`. This
  is an `echo` command that returns JSON with `additionalContext`. It runs in
  under 5 seconds (timeout: 5). If Claude has not yet called `search_questions`
  for the current problem, this nudges it to do so before falling back to web
  search.

- **PostToolUse (WebSearch|WebFetch)**: After a web search completes, the hook
  injects a reminder to submit the solution to TokenOverflow via the `submit`
  tool. Same mechanism: `echo` command returning JSON.

- **Stop**: Before Claude ends a session, a `prompt`-type hook runs. It uses
  `haiku` (a fast, cheap model) to review the conversation and check whether a
  solved problem was submitted to TokenOverflow. If not, it blocks the stop
  with a `decision: block` response, forcing Claude to call `submit` first.
  Timeout is 15 seconds to allow the model to review the conversation.

All hooks are stateless. They rely on Claude's context window to determine
whether TokenOverflow was already consulted or whether a submission is needed.

## Edge Cases & Constraints

### OAuth Flow Failures

- **User denies GitHub authorization**: WorkOS redirects back with an error
  parameter. The MCP client surfaces this as an authentication failure. Claude
  reports it cannot access TokenOverflow. The user can retry by triggering
  another tool call.
- **Browser does not open (headless/SSH)**: Claude Code cannot complete the
  OAuth flow in environments without a browser. The MCP connection stays
  unauthenticated and all tool calls fail with 401. This is a known limitation
  of browser-based OAuth for terminal tools. No workaround exists in the
  current design.
- **WorkOS AuthKit is down**: The OAuth metadata fetch or token exchange fails.
  Same result as above: tool calls fail, Claude reports the error. Transient
  failures resolve on retry.

### Token Expiry and Refresh

The MCP client handles token refresh automatically using the refresh token from
the initial OAuth flow. If the refresh token itself expires (long-lived, but
not permanent), the full browser OAuth flow is triggered again on the next tool
call. No plugin-side handling is needed.

### MCP Server Unavailable

If `api.tokenoverflow.io` is down, MCP tool calls return errors. Claude's
behavioral instructions say to proceed with web search if TokenOverflow returns
no results, but a server error is different from "no results." Claude will
report the error and fall back to other problem-solving methods. The hooks will
still fire, but the `submit` tool call will also fail. The Stop hook may block
repeatedly if it detects an unsaved solution but the server is unreachable. In
practice, the 15-second timeout on the Stop hook will eventually allow the
session to end if the model determines submission is impossible.

### User Has No GitHub Account

The WorkOS GitHub OAuth page requires a GitHub account. Users without one
cannot authenticate. This is by design: TokenOverflow uses GitHub as the sole
identity provider (per the authentication design doc). The plugin is
non-functional without authentication since all API endpoints require a JWT.

### Plugin Updates

When the upstream monorepo updates the plugin (new version in
`marketplace.json`), users get the update the next time Claude Code refreshes
marketplace data. The `source` is a relative path within the repo, so
`git pull` on the marketplace repo brings in new plugin files. Claude Code
detects the version bump and prompts the user to update.

### Offline Usage

The plugin requires network access for both MCP tool calls and the OAuth flow.
Offline usage is not supported. Claude will not be able to call any
TokenOverflow tools. The hooks will still fire (they are local `echo` commands
and a local model call), but the reminders to use TokenOverflow will be
unfulfillable.

### Conflicting Plugins

If another plugin registers an MCP server also named `tokenoverflow` in its
`.mcp.json`, a naming collision occurs. Claude Code's behavior for duplicate
MCP server names is undefined and may vary by version. Mitigation: the server
name `tokenoverflow` is specific enough that collisions are unlikely. If they
occur, the user must disable one of the conflicting plugins.

### API Gateway 401 vs Axum 401

The HTTP API Gateway JWT authorizer returns its own 401 with a plain
`WWW-Authenticate: Bearer` header (no `resource_metadata`). This happens before
the request reaches Lambda, so the Axum error handler cannot add the
`resource_metadata` parameter. The MCP client handles this by falling back to
the well-known URI probe, which bypasses the JWT authorizer (the
`/.well-known/{proxy+}` route has no authorizer). This is a valid path per the
MCP spec's "Protected Resource Metadata Discovery Requirements," which states
servers MUST implement one of the two discovery mechanisms (header or well-known
URI) and clients MUST support both.

### OAuth Proxy Failure Modes

- **AuthKit metadata fetch fails**: The `/.well-known/oauth-authorization-server`
  endpoint could fail if AuthKit is unreachable. Since we build the metadata
  response from config values (not by fetching AuthKit at request time), this
  is not a runtime concern. The metadata is effectively static.
- **Authorize redirect target is down**: The 302 redirect to AuthKit's
  authorize endpoint is followed by the user's browser, not our server. If
  AuthKit is down, the browser shows an error page. The user can retry later.
- **Token proxy cannot reach AuthKit**: If our server cannot reach AuthKit's
  token endpoint, the token proxy returns an error to the MCP client. The MCP
  client retries or re-initiates the flow on the next tool call.

## Test Plan

### Local Plugin Loading (Manual)

Test the plugin loads correctly via `--plugin-dir` against the local
docker-compose stack.

1. Start the local stack: `docker compose up -d`
2. Set env vars:

   ```bash
   export TOKENOVERFLOW_MCP_URL=http://localhost:8080/mcp
   export TOKENOVERFLOW_MCP_AUTH="Bearer $(./scripts/dev_token.sh)"
   ```

3. Load the plugin: `claude --plugin-dir ./integrations/claude`
4. Verify Claude sees the MCP server: check that `search_questions`, `submit`,
   and `upvote_answer` tools are listed.
5. Ask Claude to solve a programming problem. Confirm it calls
   `search_questions` before resorting to web search.
6. Confirm the PreToolUse hook fires when Claude attempts WebSearch (check
   Claude's context for the injected reminder).
7. Confirm the Stop hook fires at session end if a solution was found but not
   submitted.

### Auth Flow Testing (Manual, Against Production)

The OAuth flow must be tested against production (or a staging environment that
uses the real WorkOS AuthKit instance).

1. Unset the env vars (use defaults):

   ```bash
   unset TOKENOVERFLOW_MCP_URL
   unset TOKENOVERFLOW_MCP_AUTH
   ```

2. Load the plugin: `claude --plugin-dir ./integrations/claude`
3. Confirm the MCP client detects the 401 and discovers our API as the
   authorization server via the well-known URI fallback.
4. Confirm the browser opens to the GitHub OAuth page via our authorize proxy
   (which redirects to AuthKit).
5. Complete the authorization. Confirm the tool call succeeds after auth.
6. Kill Claude and restart. Confirm the cached token is reused (no browser
   popup on the next tool call).
7. Wait for token expiry (or manually invalidate the token). Confirm the
   refresh flow obtains a new token without opening the browser.

### Marketplace Install Flow (Manual)

Test the full install path from a clean state.

1. From a fresh Claude Code session (no prior plugin state), run:
   `/plugin marketplace add token-overflow/tokenoverflow`
2. Confirm the marketplace is listed in `/plugin marketplace list`.
3. Run: `/plugin install tokenoverflow@tokenoverflow-marketplace`
4. Confirm the plugin appears in `/plugin list` with the correct version
   (`0.1.0`).
5. Enable the plugin. Confirm the MCP server connects (triggers OAuth flow).

### Hook Behavior (Manual)

1. Load the plugin against the local stack (with env vars set).
2. Ask Claude a programming question. Confirm `search_questions` is called.
3. If no result is found, allow Claude to use web search. Confirm the
   PreToolUse hook message appears in Claude's context before the web search
   executes.
4. After Claude solves the problem via web search, confirm the PostToolUse hook
   message appears reminding Claude to submit.
5. Attempt to end the session without submitting. Confirm the Stop hook blocks
   and instructs Claude to call `submit`.

### Protected Resource Metadata Verification (Curl)

After deploying the auth fixes, verify the metadata chain:

```bash
# Step 1: Verify Protected Resource Metadata
curl -s https://api.tokenoverflow.io/.well-known/oauth-protected-resource | python3 -m json.tool
# Expected: authorization_servers contains our own API URL, NOT AuthKit or api.workos.com

# Step 2: Verify our authorization server metadata proxy
curl -s https://api.tokenoverflow.io/.well-known/oauth-authorization-server | python3 -m json.tool
# Expected: metadata with authorization_endpoint and token_endpoint pointing to
# our /oauth2/authorize and /oauth2/token proxy URLs, issuer still pointing to AuthKit

# Step 3: Verify the authorize proxy redirects with scopes
curl -sI "https://api.tokenoverflow.io/oauth2/authorize?response_type=code&client_id=test"
# Expected: HTTP 302 with Location header pointing to AuthKit's authorize endpoint
# with scope=openid+profile added to the query string

# Step 4: Verify the 401 response (all API routes behind JWT authorizer)
curl -sI https://api.tokenoverflow.io/mcp
# Expected: HTTP 401 with WWW-Authenticate: Bearer header
```

### OAuth Proxy Unit Tests

Add unit tests for each proxy endpoint in
`tests/unit/api/test_oauth_proxy.rs`:

- **Metadata endpoint**: Verify the response contains proxy URLs using the
  API base URL and AuthKit URLs for issuer/jwks_uri.
- **Authorize proxy**: Verify it returns 302 with the correct Location header
  (AuthKit authorize URL with `scope=openid+profile` injected). Test with
  and without existing scope parameter in the request.
- **Token proxy**: Verify it forwards the request to AuthKit's token endpoint
  and returns the response. (Use a mock HTTP client or wiremock.)

### JSON Validation (Unit-Level)

Add a lightweight check (shell script or CI step) that validates all JSON files
in `integrations/claude/` and `.claude-plugin/` are syntactically valid.

```bash
find integrations/claude .claude-plugin -name '*.json' -exec python3 -m json.tool {} \; > /dev/null
```

This can run as part of the existing pre-commit hooks.

### E2E Test Coverage

The existing Rust E2E tests (`cargo test -p tokenoverflow --test e2e`) test the
API endpoints that the plugin's MCP tools call (`search_questions`, `submit`,
`upvote_answer`). No additional Rust tests are needed for the plugin itself.
The plugin is static config; correctness means the files are valid and the
behavioral flow works end-to-end, which is covered by the manual tests above.

### Well-Known Endpoint Unit Test Update

The existing unit test for `oauth_protected_resource` in
`tests/unit/api/test_auth_middleware.rs` should be updated to verify the
`authorization_servers` value is the API base URL (since the proxy is now
served from our API, not AuthKit directly).

## Documentation Changes

### README.md

1. **Monorepo layout diagram**: Replace the `apps/claude/` entry with
   `integrations/claude/` and add the top-level `integrations/` directory. Add
   the `.claude-plugin/` directory to the tree.

2. **Claude Code section** (under "Agent Integration"): Add a subsection
   explaining the marketplace install flow for end users:

   ```
   /plugin marketplace add token-overflow/tokenoverflow
   /plugin install tokenoverflow@tokenoverflow-marketplace
   ```

   Keep the existing local dev instructions (`dev_token.sh`) but clarify they
   are for contributors, not end users. Update the local dev workflow to use
   env vars instead of the (now-deleted) root `.mcp.json`.

3. Remove the `apps/claude/` reference from the layout. The `apps/` directory
   should only list `api/`, `embedding_service/`, and `so_tag_sync/`.

4. **Authentication section**: Update the "Public client" flow to explain the
   OAuth proxy and why it exists (Claude Code scope bug workaround).

### No Other Documentation

No new standalone docs are needed. The plugin's own `instructions.md`, skill
files, and agent files are already complete and require no changes. The design
doc itself serves as the internal reference.

## Development Environment Changes

### No Changes to docker-compose

The local stack is unchanged. The plugin under `integrations/claude/` is static
files; it does not need a Docker service. Developers test the plugin by running
`claude --plugin-dir ./integrations/claude` against the already-running local
stack, with env vars set for the local URL and test JWT.

### No Changes to Scripts

The `dev_token.sh` script remains as-is. It generates test JWTs for local
development. Developers use it via
`export TOKENOVERFLOW_MCP_AUTH="Bearer $(./scripts/dev_token.sh)"`.

### Root `.mcp.json` Removed (DONE)

The root `.mcp.json` has been deleted. Local development now uses environment
variables (`TOKENOVERFLOW_MCP_URL` and `TOKENOVERFLOW_MCP_AUTH`) to override
the plugin's `.mcp.json` defaults. This avoids the dual-MCP-server confusion
that occurred when both the plugin's `tokenoverflow` server and the root's
`tokenoverflow-local` server registered tools with the same names.

### Pre-commit Hooks

Add a JSON syntax validation step for `integrations/claude/**/*.json` and
`.claude-plugin/**/*.json` to the existing pre-commit config. This catches
malformed JSON before it is committed.

## Tasks

### Completed Tasks

1. **Move `apps/claude/` to `integrations/claude/`** (DONE)
   - Ran: `mkdir -p integrations && git mv apps/claude integrations/claude`
   - Renamed `hooks/settings.json` to `hooks/hooks.json`
   - All 8 files present in the new location.

2. **Update `integrations/claude/.claude-plugin/plugin.json`** (DONE)
   - Removed the `version` field.
   - Removed `userConfig` (replaced with env var approach in `.mcp.json`).
   - Updated `repository` to `https://github.com/token-overflow/tokenoverflow`.

3. **Update `integrations/claude/.mcp.json`** (DONE)
   - URL uses `${TOKENOVERFLOW_MCP_URL:-https://api.tokenoverflow.io/mcp}`.
   - Headers include `Authorization: ${TOKENOVERFLOW_MCP_AUTH:-}` for local dev.

4. **Delete root `.mcp.json`** (DONE)
   - Removed. Local dev uses env vars instead.

### Remaining Tasks

1. **Create `.claude-plugin/marketplace.json` at repo root**
   - Create directory: `mkdir -p .claude-plugin`
   - Create the file with the exact contents from the Interfaces section.
   - Commit: "feat: add marketplace.json for claude plugin distribution"

2. **Add `oauth.clientId` to `integrations/claude/.mcp.json`**
   - Add the `oauth` block with `clientId: client_01KN3MGDJEZSGSXWH8YKKDCB2T`.
   - This lets Claude Code skip DCR and use the pre-registered public PKCE
     app.
   - Commit: "feat: add pre-configured oauth clientId to plugin .mcp.json"

3. **Single-issuer auth: switch Gateway and Axum to AuthKit**
   - Change `jwt_issuer` in `terragrunt.hcl` to
     `https://intimate-figure-17.authkit.app`.
   - Remove any explicit public `/mcp` routes in `routes.tf` (the `$default`
     catch-all with JWT authorizer now covers `/mcp`).
   - Change `issuer` and `jwks_url` in `production.toml` and
     `development.toml` to the AuthKit equivalents.
   - Commit: "fix: switch to single AuthKit issuer for JWT validation"

4. **Fix `well_known.rs`: point `authorization_servers` to our own API**
   - Change `authorization_servers: vec![state.auth_config.authkit_url.clone()]`
     to `authorization_servers: vec![state.api_base_url.clone()]`.
   - This makes the MCP client discover our OAuth proxy instead of going to
     AuthKit directly.
   - Update the unit test in `test_auth_middleware.rs` to verify the
     `authorization_servers` value matches the API base URL.
   - Commit: "fix: point authorization_servers to our API for OAuth proxy"

5. **Fix `error.rs` to use absolute URL in `WWW-Authenticate`**
   - Change the relative `/.well-known/oauth-protected-resource` to
     `https://api.tokenoverflow.io/.well-known/oauth-protected-resource`.
   - Update any related tests that assert on the header value.
   - Commit: "fix: use absolute URL in WWW-Authenticate resource_metadata"

6. **Implement OAuth authorization proxy**
   - Create `apps/api/src/api/routes/oauth_proxy.rs` with three handlers:
     - `authorize_proxy`: GET handler that adds `scope=openid+profile` to
       query params and redirects (302) to AuthKit's authorization endpoint.
     - `token_proxy`: POST handler that forwards the request body to AuthKit's
       token endpoint and returns the response as-is.
     - `register_proxy`: POST handler that forwards DCR requests to AuthKit's
       registration endpoint.
   - Add `oauth_authorization_server` handler to `well_known.rs` that serves
     the authorization server metadata with proxy URLs.
   - Register the new routes in `configure.rs` as public (no auth middleware).
   - Add the module to `routes/mod.rs`.
   - Commit: "feat: add OAuth authorization proxy for Claude Code scope bug
     workaround"

7. **Add Gateway routes for OAuth proxy endpoints**
   - Add public routes (no JWT authorizer) in `routes.tf`:
     - `GET /oauth2/authorize`
     - `POST /oauth2/token`
     - `POST /oauth2/register`
   - These routes use the same Lambda integration as all other routes.
   - Commit: "feat: add public Gateway routes for OAuth proxy endpoints"

8. **Add OAuth proxy unit tests**
   - Test the metadata endpoint returns correct proxy URLs.
   - Test the authorize proxy returns 302 with scopes injected.
   - Test the token proxy forwards to AuthKit (mock the HTTP client).
   - Commit: "test: add unit tests for OAuth proxy endpoints"

9. **Verify the auth chain end-to-end** (after deploying tasks 3-8)
   - Run the curl verification steps from the Test Plan.
   - Test the full MCP OAuth flow with Claude Code against production.
   - Confirm the `subject_types_supported` /
     `id_token_signing_alg_values_supported` error no longer occurs.
   - Confirm Claude Code uses the pre-configured `client_id` and does not
     attempt DCR.
   - Confirm the authorize proxy injects scopes and AuthKit accepts the
     request.

10. **Update README.md**
    - Update the monorepo layout diagram: replace `apps/claude/` with
      `integrations/claude/`, add `integrations/` directory, add
      `.claude-plugin/` directory.
    - Add marketplace install instructions under "Agent Integration > Claude
      Code".
    - Update local dev workflow to reference env vars instead of root
      `.mcp.json`.
    - Remove `apps/claude/` from the layout tree.
    - Update the Authentication section to explain the OAuth proxy.
    - Commit: "docs: update README for plugin distribution and OAuth proxy"

11. **Add JSON validation to pre-commit**
    - Add a hook that validates all JSON files under `integrations/` and
      `.claude-plugin/` are syntactically correct.
    - Commit: "chore: add JSON validation pre-commit hook for plugin files"

12. **Test: local plugin loading**
    - Start the local stack: `docker compose up -d`
    - Set env vars and run: `claude --plugin-dir ./integrations/claude`
    - Verify MCP tools are available and hooks fire correctly.
    - Verify `search_questions` works against the local API.

13. **Test: marketplace install flow**
    - From a clean Claude Code session, run the two-step install:
      `/plugin marketplace add token-overflow/tokenoverflow` then
      `/plugin install tokenoverflow@tokenoverflow-marketplace`.
    - Verify the plugin appears and the MCP server connects.

14. **Test: auth flow against production**
    - Use default env vars (production URL, no auth header).
    - Trigger OAuth flow and complete GitHub login.
    - Verify the JWT is cached and subsequent calls succeed without
      re-auth.
    - Verify the authorize proxy 302 redirect includes scopes.
