# Design: Authentication

## Architecture Overview

### Goal

Implement user authentication for the TokenOverflow API using GitHub as the
primary identity provider (no email+password management). Users sign in with
GitHub, their GitHub username becomes their default display name (changeable
later). All access — web, MCP, and programmatic — is authenticated via JWT.
There are no user-facing API keys.

### Scope

This design covers:

- Selecting an authentication provider from the researched alternatives
- GitHub OAuth login flow for the web interface and MCP clients
- User profile creation (GitHub username as default display name)
- Integration with the existing Rust/Axum API on AWS Lambda
- Database schema changes for the new auth model
- Migration from REST API Gateway to HTTP API Gateway
- HTTP API built-in JWT Authorizer (defense-in-depth, pre-filters invalid tokens)
- Global throttling via HTTP API stage/route settings
- Testing strategy using test JWKS with `file://` protocol
- Bruno testing workflow for local and cloud environments
- Future-proofing for CLI/plugin auth (device flow)

This design does NOT cover:

- Frontend implementation (web signup UI, dashboard)
- Enterprise SSO / SAML federation
- Role-based access control (RBAC)
- Billing / payment processing
- Per-user rate limiting / paid tiers (see Out of Scope section)
- WAF (see Out of Scope section)

### User Requirements

| # | Requirement | Weight |
|---|-------------|--------|
| 1 | Low or no cost even at large number of users | Critical |
| 2 | Easy to maintain | Critical |
| 3 | Possible to be SOC2 compliant in the future | Important |
| 4 | Reliable: set-it and forget-it | Critical |

### Future User Stories (Informing Design, Out of Scope)

1. **Web signup**: User creates account through a web interface, gets redirected
   to GitHub, authorizes, and lands on a dashboard.
2. **CLI/Plugin auth (Device Flow)**: User installs TokenOverflow plugin through
   Claude marketplace, initiates auth in their terminal, Claude opens a browser,
   user logs in with GitHub, and their Claude Code session is authenticated. This
   requires OAuth 2.0 Device Authorization Grant (RFC 8628) support.

### Chosen Solution: WorkOS AuthKit

After evaluating 21 authentication solutions (see Alternatives Research appendix
below), **WorkOS AuthKit** was selected. Key reasons:

- 1M MAU free tier (cost is a non-issue for the foreseeable future)
- Native GitHub OAuth (no shims or wrappers)
- Purpose-built CLI Auth (RFC 8628 Device Authorization Grant)
- Fully managed SaaS (zero maintenance)
- SOC2 Type II, GDPR, CCPA, HIPAA compliant
- Spec-compliant OAuth 2.1 authorization server with MCP integration support

---

### MCP OAuth Ecosystem Compatibility

All major AI coding tools support OAuth/browser-based login for MCP servers.
This validates the "browser opens, login with GitHub, done" approach across the
entire ecosystem via MCP OAuth 2.1.

| Tool | OAuth Support | Transport | Maturity |
|------|:---:|---|---|
| **Claude Code** | Yes | HTTP MCP + browser redirect | Comprehensive config, some open bugs with silent auth failures |
| **OpenCode** | Yes | `opencode mcp auth` -> browser | Works, fails silently in headless/SSH |
| **Codex CLI** | Yes | `codex mcp login` -> browser | Solid since v0.0.389, auto token refresh |
| **Cursor** | Yes | Auto browser flow for MCP servers | Early adopter (June 2025), some PKCE issues |
| **Windsurf** | Yes | OAuth 2.1 + DCR for remote MCP | Supported across all transports |
| **Cline** | Yes | Browser -> `vscode://cline/callback` | Initial auth works, re-auth after token expiry is buggy |
| **Continue** | Yes | Browser redirect for SSE endpoints | Works but documentation still catching up |
| **GitHub Copilot** | Yes | OAuth 2.1 + DCR, WAM on Windows | Most mature -- Microsoft is directly involved in the MCP auth spec |

**Implication for TokenOverflow**: By implementing the MCP OAuth 2.1 spec with
WorkOS as the authorization server, any MCP-compatible AI coding tool can
authenticate to TokenOverflow out of the box. Agents beyond Claude Code (Codex
CLI, Cursor, Copilot, etc.) get free authentication support.

---

### High-Level System Diagram

**Cloud (behind API Gateway)**:

```text
  MCP Client (Claude Code, Cursor, etc.)
    |
    |-- Bearer JWT (from WorkOS GitHub OAuth)
    v
  HTTP API Gateway
    |-- Built-in JWT Authorizer (defense-in-depth)
    |     Rejects invalid tokens before Lambda runs
    |     Saves Lambda invocations/cost
    |
    |-- Stage/route-level throttling (global safety net)
    v
  Lambda (Rust/Axum API)
    |-- jwt_auth middleware
    |     Extracts Bearer token from Authorization header
    |     Validates JWT against WorkOS JWKS (~1-5ms, negligible)
    |     Same code path as local — no environment detection
    |-- Resolves local user by workos_id (JIT provisioning)
    |-- Business logic
    v
  PostgreSQL (via PgBouncer)
```

**Local development (no API Gateway)**:

```text
  MCP Client / Bruno / E2E tests
    |
    |-- Bearer JWT (signed with test private key)
    v
  Rust/Axum API (direct HTTP, port 8080)
    |-- jwt_auth middleware
    |     Extracts Bearer token from Authorization header
    |     Validates JWT against file:// JWKS (test public key)
    |     Same code path as cloud — no environment detection
    |-- Resolves local user by workos_id (JIT provisioning)
    |-- Business logic
    v
  PostgreSQL (via Docker Compose)
```

### Auth Model

All authentication uses **WorkOS JWTs via GitHub OAuth**. There are no
user-facing API keys. The MCP OAuth flow gives the client a JWT, and the server
validates that JWT on every request.

---

## HTTP API Gateway and JWT Authorizer

### Why HTTP API Instead of REST API

The current infrastructure uses REST API Gateway. This design migrates to HTTP
API Gateway for three reasons:

1. **Built-in JWT Authorizer**: HTTP API has a native JWT authorizer that
   validates tokens at the edge by fetching the provider's JWKS. Configure the
   issuer URL and audience, and API Gateway handles signature verification,
   claim validation, and key rotation automatically. Zero application code, no
   Lambda Authorizer to build or maintain.

2. **Cost**: HTTP API is $1.00 per million requests vs. REST API's $3.50 per
   million -- a 71% reduction.

3. **REST API Usage Plan limits are untenable**: REST API's Usage Plans require
   API Gateway keys, which have a hard limit of 10,000 per account per region.
   This limit cannot be increased. It makes per-user rate limiting via API
   Gateway impossible at consumer scale.

### JWT Authorizer Configuration (Defense-in-Depth)

The HTTP API JWT authorizer is configured entirely in Terraform. It acts as a
defense-in-depth layer that pre-filters invalid tokens before Lambda is invoked.
The Axum application always re-validates JWTs itself (~1-5ms, negligible) so
that the same code path runs in all environments. If API Gateway is ever
removed or changed, the application still works correctly.

**Configuration**:

| Setting | Value |
|---|---|
| Authorizer type | JWT |
| Identity source | `$request.header.Authorization` |
| Issuer | `https://<authkit_domain>` (WorkOS AuthKit OIDC issuer) |
| Audience | `https://api.tokenoverflow.io` |

API Gateway validates each request by:

1. Extracting the Bearer token from the Authorization header
2. Fetching the WorkOS JWKS (cached internally by API Gateway)
3. Verifying the JWT signature (RS256)
4. Validating `iss`, `aud`, and `exp` claims
5. Passing the validated claims to Lambda in the event payload

If validation fails, API Gateway returns 401 directly without invoking Lambda.

### Why the App Re-validates JWTs

API Gateway's JWT authorizer pre-filters invalid tokens, but the Axum
application always validates JWTs itself from the `Authorization` header. This
means:

- **Single code path**: The same `jwt_auth` middleware runs in cloud (Lambda),
  local dev, and tests. No environment detection or branching needed.
- **Defense-in-depth**: API Gateway catches invalid tokens before Lambda runs
  (saving invocations/cost). The app re-validates (~1-5ms, negligible) for
  correctness.
- **Portability**: If API Gateway is ever removed, reconfigured, or replaced,
  the app still authenticates correctly.

The application does **not** read from
`requestContext.authorizer.jwt.claims`. It always reads the `Authorization`
header directly.

**Note**: The current `trace_id` middleware in `middleware.rs` matches on
`RequestContext::ApiGatewayV1`. This must be updated to also handle
`RequestContext::ApiGatewayV2`, which is the event format used by HTTP API.
This is a trace_id concern, not an auth concern.

### Route Structure

HTTP API uses a simpler routing model than REST API. Routes are defined with
path patterns and methods, and authorizers are attached per-route.

```text
Route                                Authorizer
-----                                ----------
GET  /health                         None (public)
GET  /.well-known/{proxy+}           None (public, MCP metadata)
$default                             JWT (catches everything else)
```

Only `/health` and `/.well-known/*` are defined as explicit routes without an
authorizer. Everything else — `/v1/*`, `/mcp/*`, and any future endpoints —
falls through to the `$default` route which requires a valid JWT. This means
new endpoints are authenticated by default without any Terraform changes.

**Health endpoint**: Since `/health` is public, it must not leak internal
details. The response changes from `{"status":"ok","database":"connected"}` to
just `{"status":"ok"}`. No database status, no version numbers, no dependency
information on the public endpoint.

**Why no public read endpoints?** The future web frontend will use server-side
rendering (SSR) — the website server fetches from the API on the server side
(with a service-level JWT or via internal VPC access), renders HTML, and serves
it to the browser. Public visitors see HTML pages, never the API directly. This
means all API endpoints can stay behind JWT auth.

### Global Throttling

HTTP API provides stage-level and route-level throttling settings:

| Setting | Value | Purpose |
|---|---|---|
| Stage default rate | 500 req/sec | Sustained load from ~2,000 active users |
| Stage default burst | 1,000 | Handles signup spikes (e.g., HN front page) |

These are global limits (not per-user). They protect against accidental DDoS or
runaway clients. If a single user hits the global limit, all users are affected.
This is acceptable for MVP. Per-user rate limiting is deferred (see Out of Scope
section).

Route-level overrides can be added later for specific endpoints (e.g., lower
limits on write endpoints).

---

## Interfaces

### 1. MCP OAuth 2.1 Metadata Endpoints (New)

The TokenOverflow API (acting as an MCP resource server) must serve two metadata
endpoints to comply with the MCP authorization spec.

#### GET /.well-known/oauth-protected-resource

Returns the Protected Resource Metadata document (RFC 9728). This tells MCP
clients where to find the authorization server.

**Response:**

```json
{
    "resource": "https://api.tokenoverflow.io",
    "authorization_servers": [
        "https://intimate-figure-17.authkit.app"
    ],
    "bearer_methods_supported": ["header"],
    "scopes_supported": ["openid", "profile"]
}
```

The `resource` field is the canonical URI of the MCP server. MCP clients use
this as the `resource` parameter in authorization and token requests per
RFC 8707.

The `authorization_servers` array points to the WorkOS AuthKit instance. The
environment ID is provided by WorkOS during setup.

#### Out of Scope: GET /.well-known/oauth-authorization-server

This endpoint would proxy WorkOS authorization server metadata for MCP clients
that don't support Protected Resource Metadata (RFC 9728). All 8 MCP-compatible
tools we researched (Claude Code, Cursor, Codex CLI, Copilot, Windsurf, Cline,
Continue, OpenCode) support Protected Resource Metadata. Since we're launching
fresh with no legacy clients, this endpoint is unnecessary. If a future client
requires it, it can be added as a simple proxy to
`https://intimate-figure-17.authkit.app/.well-known/oauth-authorization-server`.

#### 401 Unauthorized Response Format

When API Gateway rejects a request due to a missing or invalid JWT, it returns
a 401 directly. For MCP-specific 401 responses (e.g., when an MCP client hits
the API without a token), the Axum application also returns:

```http
HTTP/1.1 401 Unauthorized
WWW-Authenticate: Bearer resource_metadata="https://api.tokenoverflow.io/.well-known/oauth-protected-resource"
```

This triggers the MCP client to discover the authorization server and initiate
the OAuth flow.

### 2. Authenticated User Extraction (New)

A single `jwt_auth` middleware extracts and validates the JWT on every request.
The same code path runs in all environments (cloud, local, test) — no
environment detection needed.

```rust
pub struct AuthenticatedUser {
    pub user_id: i64,       // Primary key from the users table
    pub workos_id: String,  // WorkOS user ID (from JWT sub claim)
}
```

The middleware:

1. Extracts the Bearer token from the `Authorization` header
2. Validates the JWT against the configured JWKS (WorkOS JWKS in production,
   `file://` test key locally)
3. Resolves the local user by `workos_id` (JIT provisioning if new)
4. Injects `AuthenticatedUser` into request extensions

### 3. Route Protection Strategy

```text
Route Group              Auth Method         Middleware
-----------              -----------         ----------
/health                  None                (public)
/.well-known/*           None                (public, metadata)
/mcp                     JWT (MCP OAuth)     jwt_auth
/v1/*                    JWT                 jwt_auth
```

The `jwt_auth` middleware always validates JWTs from the `Authorization` header.
In production, API Gateway pre-filters invalid tokens as defense-in-depth, but
the app does not rely on this — it validates every request itself.

### 5. User Provisioning (JIT)

No explicit "signup" endpoint exists. User provisioning happens automatically
on first authenticated request:

1. JWT is validated by the `jwt_auth` middleware (same code path everywhere)
2. `sub` claim (WorkOS user ID) extracted
3. Lookup user by `workos_id` in the database
4. If not found:
   a. Call WorkOS GET /user_management/users/{workos_id}
      (using TOKENOVERFLOW_WORKOS_API_KEY)
   b. Extract: github_id, github_username, avatar_url
   c. INSERT INTO users (workos_id, github_id, github_username,
      display_name, avatar_url)
      ON CONFLICT (workos_id) DO NOTHING
   d. If insert conflicted (concurrent first login), re-fetch the user
5. If found: return the existing user

This "just-in-time provisioning" pattern means:
- No separate signup flow to build
- User record is created on first login to TokenOverflow
- GitHub username is fetched from WorkOS and stored as `display_name`

### 6. Configuration Changes

New configuration section in TOML files:

```toml
[auth]
workos_client_id = "client_..."
workos_api_url = "https://api.workos.com"
jwks_url = "https://<authkit_domain>/oauth2/jwks"
jwks_cache_ttl_secs = 3600
issuer = "https://<authkit_domain>"
audience = "https://api.tokenoverflow.io"
```

Environment variables (secrets, not in TOML):

```bash
TOKENOVERFLOW_WORKOS_API_KEY=sk_live_...   # WorkOS API key
```

The `workos_client_id` is not a secret (it is public, included in JWKS URL and
metadata responses). Only the WorkOS secret key is a secret.

For local/test environments, the `jwks_url` uses the `file://` protocol to load
from a local file (see Testing Strategy section).

---

## Logic

### Database Migration

A new migration adds columns to the `users` table and removes the Cognito
dependency.

```sql
-- Migration: Replace Cognito with WorkOS

-- 1. Add new columns
ALTER TABLE api.users
    ADD COLUMN workos_id VARCHAR(255),
    ADD COLUMN github_id BIGINT,
    ADD COLUMN github_username VARCHAR(39),
    ADD COLUMN display_name VARCHAR(100),
    ADD COLUMN avatar_url TEXT;

-- 2. Backfill: system user gets a placeholder workos_id
UPDATE api.users SET workos_id = 'system' WHERE id = 1;

-- 3. Make workos_id NOT NULL after backfill
ALTER TABLE api.users ALTER COLUMN workos_id SET NOT NULL;

-- 4. Add unique constraints
ALTER TABLE api.users ADD CONSTRAINT users_workos_id_unique UNIQUE (workos_id);
ALTER TABLE api.users ADD CONSTRAINT users_github_id_unique UNIQUE (github_id);

-- 5. Drop the old cognito_sub column and its unique constraint
ALTER TABLE api.users DROP CONSTRAINT users_cognito_sub_key;
ALTER TABLE api.users DROP COLUMN cognito_sub;

-- 7. Index for GitHub username lookups
CREATE INDEX users_github_username_idx ON api.users (github_username);
```

**Column rationale:**

| Column | Type | Why |
|---|---|---|
| `workos_id` | VARCHAR(255) NOT NULL UNIQUE | WorkOS user identifier (`user_...`). Replaces `cognito_sub`. Used for JWT `sub` claim lookup. |
| `github_id` | BIGINT UNIQUE | GitHub's numeric user ID. Stable even if user renames their GitHub account. |
| `github_username` | VARCHAR(39) | GitHub username (max 39 chars per GitHub spec). Used as default display name. May become stale if user renames on GitHub. |
| `display_name` | VARCHAR(100) | User-facing display name. Defaults to `github_username` on first login but can be changed by the user. |
| `avatar_url` | TEXT | GitHub avatar URL. Used by future web frontend. |

### Diesel Schema Update

The Diesel schema for the `users` table changes from:

```rust
// Before
api.users (id) {
    id -> Int8,
    cognito_sub -> Varchar,
    created_at -> Timestamptz,
    updated_at -> Timestamptz,
}
```

To:

```rust
// After
api.users (id) {
    id -> Int8,
    workos_id -> Varchar,
    github_id -> Nullable<Int8>,
    github_username -> Nullable<Varchar>,
    display_name -> Nullable<Varchar>,
    avatar_url -> Nullable<Text>,
    created_at -> Timestamptz,
    updated_at -> Timestamptz,
}
```

### JWKS Verification Logic

The Axum application validates JWTs in all environments. In production, API
Gateway also validates as defense-in-depth, but the app always re-validates
from the `Authorization` header for a single code path.

```text
1. Extract Bearer token from Authorization header
2. Decode JWT header (without verification) to get `kid` (Key ID)
3. Look up `kid` in cached JWKS keyset
   - If cache miss or expired: load JWKS from configured URL
   - If `kid` not found after refresh: return 401
4. Verify JWT signature using the matching public key (RS256)
5. Validate claims:
   - `iss` must match configured issuer
   - `aud` must match configured audience
   - `exp` must be in the future
   - `sub` must be present (WorkOS user ID)
6. Return validated claims
```

**Crate choice**: `jsonwebtoken` (most popular Rust JWT library, 2.2K+ GitHub
stars, supports RS256 and JWKS). JWKS fetching uses `reqwest` (already a
dependency).

**`file://` protocol support**: When the `jwks_url` config starts with
`file://`, the JWKS is loaded from a local file instead of fetching over HTTP.
This enables test and local configurations to use a checked-in test JWKS without
running a mock HTTP server. The path is relative to the working directory.

### User Resolution Logic

After the JWT is validated and claims are extracted, the middleware resolves the
local user:

```text
1. Extract `sub` from JWT claims -> workos_id
2. SELECT * FROM users WHERE workos_id = $1
3. If found: inject user.id into request extensions
4. If not found:
   a. Call WorkOS GET /user_management/users/{workos_id}
      (using TOKENOVERFLOW_WORKOS_API_KEY)
   b. Extract: github_id, github_username, avatar_url
   c. INSERT INTO users (workos_id, github_id, github_username,
      display_name, avatar_url)
      VALUES ($1, $2, $3, $3, $4)
      ON CONFLICT (workos_id) DO NOTHING
   d. If insert was a no-op (conflict), re-fetch the user
   e. Inject new user.id into request extensions
```

The WorkOS API call in step 4a only happens once per user (first login). After
that, the user is resolved from the local database.

### Replacing SYSTEM_USER_ID

The current codebase uses `SYSTEM_USER_ID = 1` as a placeholder for all
operations. With auth implemented:

- Service functions (`QuestionService::create`, `AnswerService::create`, etc.)
  already accept a `submitted_by: i64` parameter
- The change is at the call site: instead of passing `SYSTEM_USER_ID`, pass
  the authenticated user ID from `AuthenticatedUser` in request extensions
- `SYSTEM_USER_ID` remains only for the seed data and for any background
  operations that are not user-initiated

### Config Struct Changes

```rust
// New section in config.rs
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub workos_client_id: String,
    pub workos_api_url: String,
    pub jwks_url: String,
    #[serde(default = "default_jwks_cache_ttl")]
    pub jwks_cache_ttl_secs: u64,
    pub issuer: String,
    pub audience: String,
    #[serde(skip_deserializing)]
    workos_api_key: Option<String>,
}

fn default_jwks_cache_ttl() -> u64 {
    3600
}

impl AuthConfig {
    pub fn workos_api_key(&self) -> Option<&str> {
        self.workos_api_key.as_deref()
    }
}
```

The `Config` struct gains a new `pub auth: AuthConfig` field. The secret
`workos_api_key` is loaded from `TOKENOVERFLOW_WORKOS_API_KEY` in the
same pattern as `database.password` and `embedding.api_key`.

### AppState Changes

```rust
pub struct AppState {
    pub pool: DbPool,
    pub embedding: Arc<dyn EmbeddingService>,
    pub questions: Arc<dyn QuestionRepository>,
    pub answers: Arc<dyn AnswerRepository>,
    pub search: Arc<dyn SearchRepository>,
    pub tags: Arc<dyn TagRepository>,
    pub tag_resolver: Arc<TagResolver>,
    // New:
    pub auth: Arc<AuthService>,
}
```

`AuthService` encapsulates JWKS loading, JWT validation, user resolution, and
WorkOS API calls (for JIT provisioning). It runs the same code path in all
environments.

### File Structure (New/Modified Files)

```text
apps/api/
  src/
    api/
      middleware.rs             # Add jwt_auth; update trace_id for v2 payload format
      extractors.rs             # NEW: AuthenticatedUser extractor
      routes/
        configure.rs            # Add middleware layers per route group
        well_known.rs           # NEW: /.well-known/* metadata endpoints
    services/
      auth.rs                   # NEW: AuthService (JWKS, JWT, user resolution)
      repository/
        interface/
          user.rs               # NEW: UserRepository trait
        postgres/
          user.rs               # NEW: PgUserRepository
    db/
      models/
        user.rs                 # NEW: User, NewUser structs
    config.rs                   # Add AuthConfig section
    error.rs                    # Add Unauthorized, Forbidden variants
  config/
    local.toml                  # Add [auth] section (file:// JWKS)
    unit_test.toml              # Add [auth] section (file:// JWKS)
    development.toml            # Add [auth] section
    production.toml             # Add [auth] section
  tests/
    assets/
      auth/
        test_jwks.json          # NEW: Static test JWKS (public key)
        test_private_key.pem    # NEW: Static test RSA private key
    common/
      test_jwt.rs               # NEW: Helper to generate test JWTs
  migrations/
    <timestamp>_auth/up.sql     # Schema migration
    <timestamp>_auth/down.sql   # Rollback migration

infra/terraform/
  modules/
    api_gateway/                # UPDATED: HTTP API Gateway v2 module
      api.tf
      authorizer.tf
      stage.tf
      routes.tf
      permissions.tf
      variables.tf
      outputs.tf
    dns/
      api_gateway.tf            # MODIFIED: Switch to apigatewayv2 resources

bruno/
  TokenOverflow/
    collections/api/
      opencollection.yml        # MODIFIED: Add collection-level Bearer auth
      environments/local.yml    # Add auth_token with pre-committed long-lived test JWT
      well_known.yml            # NEW: Test /.well-known endpoint

scripts/src/
  dev_token.sh                  # NEW: Generate test JWTs (--expiry, --sub)

.mcp.json                       # NEW: Committed with pre-generated test JWT for Claude Code
```

---

## Edge Cases and Constraints

### GitHub Username Changes

If a user renames their GitHub account, their `github_username` in
TokenOverflow becomes stale. The `github_id` (numeric, stable) is the
authoritative identifier. On subsequent logins, the middleware can check if the
GitHub username changed (by comparing the WorkOS profile) and update it.
However, this requires an additional WorkOS API call on every login, which is
not worth it for MVP. The staleness is cosmetic only and can be addressed by a
future "refresh profile" endpoint.

### JWKS Key Rotation

WorkOS rotates signing keys periodically. The `AuthService` JWKS cache
respects the configured TTL (`jwks_cache_ttl_secs`) and refreshes automatically.
If a `kid` is not found in the cached keyset, the service forces a JWKS refresh
before returning 401 — handling key rotation gracefully.

In local/test environments, the JWKS is loaded from a static `file://` path, so
rotation is not applicable.

API Gateway also caches and rotates JWKS independently as defense-in-depth.

### Token Expiry and MCP Sessions

WorkOS access tokens have a short expiry (typically 5-15 minutes). MCP clients
are responsible for refreshing tokens using the refresh token. The API server
only validates the access token and does not manage refresh logic.

If a token expires mid-session, API Gateway returns a 401 and the MCP client is
expected to re-authenticate per the MCP spec.

### Race Condition: Concurrent First Login

If a user's first request spawns multiple concurrent MCP tool calls, multiple
Lambda invocations may try to create the user simultaneously. The
`UNIQUE(workos_id)` constraint prevents duplicates. The insert uses
`ON CONFLICT (workos_id) DO NOTHING` and re-fetches the existing user.

### Lambda Cold Starts

On Lambda cold starts, the JWKS cache is empty. The first request triggers a
JWKS fetch from the configured URL (~50-200ms for WorkOS). Subsequent requests
use the cached keyset. API Gateway's defense-in-depth layer ensures that only
valid tokens reach Lambda, so the cold start JWKS fetch only happens for
legitimate requests.

The user resolution database query on cold start is the same cost as any other
database query -- no special concern.

### Existing Endpoints During Migration

During the transition period, existing endpoints that currently use
`SYSTEM_USER_ID` will need a migration strategy:

- Phase 1: Add auth middleware but allow unauthenticated requests to fall back
  to `SYSTEM_USER_ID` (backwards compatible)
- Phase 2: Require authentication on all write endpoints
- Phase 3: Remove `SYSTEM_USER_ID` fallback entirely

### WorkOS API Rate Limits

The WorkOS User Management API has rate limits. Since we only call it on first
login (to fetch the user profile), this is not a concern. Even at high signup
rates, WorkOS's limits are generous for user management operations.

### REST API to HTTP API Migration -- Downtime Considerations

The API Gateway migration involves replacing the REST API with an HTTP API.
This is a destructive change -- the REST API resources are deleted and replaced.

**Custom domain migration**: The DNS module currently uses
`aws_api_gateway_domain_name` and `aws_api_gateway_base_path_mapping` (REST API
resources). These must change to `aws_apigatewayv2_domain_name` and
`aws_apigatewayv2_api_mapping` (HTTP API resources).

**Migration approach**:

1. Deploy HTTP API alongside the existing REST API (new module, new resources)
2. Test the HTTP API using its default endpoint URL
3. Update the DNS module to point `api.tokenoverflow.io` to the HTTP API
4. Delete the old REST API module

This allows a brief cutover window rather than extended downtime. The DNS
change propagation through Cloudflare is near-instant since the domain is
proxied.

### HTTP API Payload Format Version

HTTP API uses payload format version 2.0 by default. The `lambda_http` crate
already supports this via `RequestContext::ApiGatewayV2`. The current trace_id
middleware matches only on `RequestContext::ApiGatewayV1` -- it must be updated
to handle both variants (or just `ApiGatewayV2` after the migration). Auth does
not read from the event context; it always validates from the `Authorization`
header directly.

The `AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH` environment variable is a REST API
concern. HTTP API does not prepend the stage name to paths by default, so this
variable can be removed after migration.

---

## Testing Strategy

### Test JWKS Approach

Instead of mocking HTTP endpoints or running a test OIDC server, the test
strategy uses a **static RSA key pair checked into the repository** and the
`file://` protocol in JWKS configuration.

**Key components**:

1. **Test private key**: `apps/api/tests/assets/auth/test_private_key.pem`
   - An RSA private key used to sign test JWTs
   - Checked into the repo (it is a test-only key, not a secret)
2. **Test JWKS**: `apps/api/tests/assets/auth/test_jwks.json`
   - Contains the corresponding RSA public key in JWKS format
   - Referenced by `local.toml` and `unit_test.toml` via `file://` protocol
3. **Test JWT generator**: `apps/api/tests/common/test_jwt.rs`
   - A Rust helper that loads the private key and generates JWTs with
     configurable claims (sub, exp, iss, aud, kid)
   - Used by unit tests, integration tests, and E2E tests

**Config for test environments**:

```toml
# config/unit_test.toml and config/local.toml additions
[auth]
workos_client_id = "client_test"
workos_api_url = "http://localhost:8080"
jwks_url = "file://tests/assets/auth/test_jwks.json"
jwks_cache_ttl_secs = 0
issuer = "tokenoverflow-test"
audience = "http://localhost:8080"
```

The `file://` path is relative to the working directory (the `apps/api`
directory when running tests or the local dev server). The JWKS loader detects
the `file://` prefix and reads from the filesystem instead of making an HTTP
request.

### No Auth Bypass Endpoints

There are no test-only code paths, feature flags, or bypass endpoints. The
same `jwt_auth` middleware runs in all environments. The only difference is the
JWKS source: `file://` for test/local, `https://` for production. The
validation always happens in the Axum middleware. In production, API Gateway
also validates as defense-in-depth.

### Unit Tests

| Component | Key Tests |
|---|---|
| JWT validation | Valid token accepted, expired token rejected, wrong issuer rejected, wrong audience rejected, missing `sub` rejected, unknown `kid` triggers JWKS refresh |
| JWKS loader | `file://` protocol loads from filesystem, cache TTL respected |
| User resolution | Existing user returned, new user created from WorkOS profile, concurrent first login handled (ON CONFLICT) |
| Auth middleware | Bearer token extracted, missing token returns 401 with correct WWW-Authenticate header, invalid token returns 401, valid token injects AuthenticatedUser |
| Well-known endpoints | Correct JSON structure, correct authorization_servers URL, correct resource URI |

Unit tests use the test JWKS (`file://tests/assets/auth/test_jwks.json`)
and mock WorkOS API responses. No network calls.

### Integration Tests

| Test | Dependencies | Success Criteria |
|---|---|---|
| JWT -> user creation | Real PostgreSQL (testcontainers) | JWT with unknown workos_id creates user row; second request returns same user |
| Migration | Real PostgreSQL | Migration applies cleanly, existing system user preserved, new columns present |

### E2E Tests

E2E tests generate JWTs on the fly using the test private key. They hit the
running API server (started via `docker compose`).

| Test | Success Criteria |
|---|---|
| GET /.well-known/oauth-protected-resource | Returns valid JSON with authorization_servers array |
| Unauthenticated request to protected endpoint | Returns 401 |
| Authenticated MCP request (with test JWT) | Returns MCP response (tool list or tool result) |
| POST /v1/questions with test JWT | Creates question, submitted_by matches the JWT sub user |
| POST /v1/questions without auth | Returns 401 |

**E2E test JWT generation pattern**:

```rust
// In tests/common/test_jwt.rs
use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};

pub fn generate_test_jwt(sub: &str, expires_in_secs: u64) -> String {
    let private_key = include_bytes!("../assets/auth/test_private_key.pem");
    let key = EncodingKey::from_rsa_pem(private_key).unwrap();

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-key-1".to_string());

    let claims = serde_json::json!({
        "sub": sub,
        "iss": "tokenoverflow-test",
        "aud": "http://localhost:8080",
        "exp": jsonwebtoken::get_current_timestamp() + expires_in_secs,
        "iat": jsonwebtoken::get_current_timestamp(),
    });

    encode(&header, &claims, &key).unwrap()
}
```

### Bruno Testing Workflow

Bruno is used for manual API testing. Collection-level Bearer auth
(`Bearer {{auth_token}}`) is set in `opencollection.yml`, so all requests
inherit the auth header automatically. Two environment workflows:

**Local environment** (zero setup — see
[Bruno: Zero-Setup Local Testing](#bruno-zero-setup-local-testing) for details):

The `local` environment ships with a pre-committed long-lived test JWT. Select
the `local` environment in Bruno and start making requests. No token generation
or manual configuration needed.

**Cloud environment** (real WorkOS OAuth):

1. Authenticate with WorkOS via browser (OAuth flow)
2. Copy the access token from the browser/callback
3. Set the token as a Bruno environment variable in `prod.yml` or `dev.yml`
4. Bruno requests include `Authorization: Bearer <real_token>` header
5. HTTP API Gateway JWT authorizer validates against real WorkOS JWKS

**Bruno environment files**:

```yaml
# bruno/TokenOverflow/collections/api/environments/local.yml
# Pre-committed with a 10-year test JWT — no manual setup needed
name: local
variables:
  - name: base_url
    value: http://localhost:8080
  - name: auth_token
    value: <pre-generated-long-lived-test-jwt>
```

```yaml
# bruno/TokenOverflow/collections/api/environments/prod.yml
name: prod
variables:
  - name: base_url
    value: https://api.tokenoverflow.io
  - name: auth_token
    value: <paste-real-workos-token>
```

---

## Security Considerations

### Test Key Pair Safety

The test RSA key pair is checked into the repository. This is safe because:

- It is only used in `unit_test` and `local` environments
- Production and development environments use real WorkOS JWKS URLs (`https://`)
- The `issuer` claim in test tokens (`tokenoverflow-test`) does not match the
  production issuer -- a test token cannot be used against production
- The `audience` claim in test tokens (`http://localhost:8080`) does not match
  production (`https://api.tokenoverflow.io`)
- API Gateway's JWT authorizer in production is configured with the real WorkOS
  issuer and audience, so test tokens are always rejected

### API Gateway JWT Authorizer Security (Defense-in-Depth)

The HTTP API JWT authorizer validates tokens at the edge before the Lambda is
invoked. This is a defense-in-depth layer:

- Invalid tokens never reach the Lambda function (saves invocations/cost)
- Provides an additional validation layer independent of application code
- API Gateway handles its own JWKS caching and rotation automatically

The application also validates JWTs itself for a single code path across all
environments.

### WorkOS API Key Scope

The `TOKENOVERFLOW_WORKOS_API_KEY` secret is only used for the WorkOS User
Management API (to fetch user profiles during JIT provisioning). It is never
exposed to clients. It is stored in SSM Parameter Store and injected as a Lambda
environment variable.

### CORS Update

The current CORS configuration allows `Any` origin. With auth, the CORS layer
should also allow the `Authorization` header:

```rust
.allow_headers([CONTENT_TYPE, AUTHORIZATION])
```

---

## Terraform Changes

### REST API to HTTP API Migration

The entire `api_gateway` module is replaced with a new `http_api` module. The
old module is deleted after the migration is complete.

#### New Module: `infra/terraform/modules/http_api/`

**`api.tf`**:

```hcl
resource "aws_apigatewayv2_api" "main" {
  name          = "main-api"
  protocol_type = "HTTP"

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
```

**`authorizer.tf`**:

```hcl
resource "aws_apigatewayv2_authorizer" "jwt" {
  api_id           = aws_apigatewayv2_api.main.id
  authorizer_type  = "JWT"
  identity_sources = ["$request.header.Authorization"]
  name             = "workos-jwt"

  jwt_configuration {
    issuer   = var.jwt_issuer
    audience = var.jwt_audience
  }
}
```

**`stage.tf`**:

```hcl
resource "aws_apigatewayv2_stage" "prod" {
  api_id      = aws_apigatewayv2_api.main.id
  name        = "$default"
  auto_deploy = true

  default_route_settings {
    throttling_rate_limit  = var.default_rate_limit
    throttling_burst_limit = var.default_burst_limit
  }

  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.api_gateway.arn
    format = jsonencode({
      requestId    = "$context.requestId"
      ip           = "$context.identity.sourceIp"
      method       = "$context.httpMethod"
      path         = "$context.path"
      status       = "$context.status"
      latency      = "$context.responseLatency"
      authStatus   = "$context.authorizer.status"
      authError    = "$context.authorizer.error"
    })
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_cloudwatch_log_group" "api_gateway" {
  name              = "/aws/apigateway/main-http-api"
  retention_in_days = var.log_retention_days

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
```

**`routes.tf`**:

```hcl
resource "aws_apigatewayv2_integration" "lambda" {
  api_id                 = aws_apigatewayv2_api.main.id
  integration_type       = "AWS_PROXY"
  integration_uri        = var.lambda_invoke_arn
  payload_format_version = "2.0"
}

# Public routes (no authorizer)
resource "aws_apigatewayv2_route" "health" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "GET /health"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

resource "aws_apigatewayv2_route" "well_known" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "GET /.well-known/{proxy+}"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

# Everything else requires JWT auth
resource "aws_apigatewayv2_route" "default" {
  api_id             = aws_apigatewayv2_api.main.id
  route_key          = "$default"
  target             = "integrations/${aws_apigatewayv2_integration.lambda.id}"
  authorization_type = "JWT"
  authorizer_id      = aws_apigatewayv2_authorizer.jwt.id
}
```

Only three routes are needed. The `$default` catch-all requires JWT auth and
handles all `/v1/*`, `/mcp/*`, and any future endpoints. New endpoints are
authenticated by default without Terraform changes.

**`permissions.tf`**:

```hcl
resource "aws_lambda_permission" "apigw" {
  statement_id  = "AllowHTTPAPIInvoke"
  action        = "lambda:InvokeFunction"
  function_name = var.lambda_function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.main.execution_arn}/*/*"
}
```

**`variables.tf`**:

```hcl
variable "env_name" {
  description = "Environment name"
  type        = string
}

variable "lambda_invoke_arn" {
  description = "Lambda invoke ARN"
  type        = string
}

variable "lambda_function_name" {
  description = "Lambda function name for invoke permission"
  type        = string
}

variable "jwt_issuer" {
  description = "JWT issuer URL (WorkOS)"
  type        = string
}

variable "jwt_audience" {
  description = "JWT audience (list of allowed audiences)"
  type        = list(string)
}

variable "default_rate_limit" {
  description = "Default stage-level rate limit (requests/second)"
  type        = number
  default     = 500
}

variable "default_burst_limit" {
  description = "Default stage-level burst limit"
  type        = number
  default     = 1000
}

variable "log_retention_days" {
  description = "CloudWatch log retention in days"
  type        = number
  default     = 14
}
```

**`outputs.tf`**:

```hcl
output "api_id" {
  description = "HTTP API ID"
  value       = aws_apigatewayv2_api.main.id
}

output "api_endpoint" {
  description = "HTTP API default endpoint"
  value       = aws_apigatewayv2_api.main.api_endpoint
}

output "execution_arn" {
  description = "HTTP API execution ARN"
  value       = aws_apigatewayv2_api.main.execution_arn
}

output "stage_name" {
  description = "Stage name"
  value       = aws_apigatewayv2_stage.prod.name
}
```

#### Modified Module: `infra/terraform/modules/dns/`

The DNS module's `api_gateway.tf` currently uses REST API resources. It needs
to switch to HTTP API (apigatewayv2) resources:

```hcl
# Before (REST API)
resource "aws_api_gateway_domain_name" "main" { ... }
resource "aws_api_gateway_base_path_mapping" "main" { ... }

# After (HTTP API)
resource "aws_apigatewayv2_domain_name" "main" {
  for_each    = local.api_gateway_domains
  domain_name = each.value.domain_name

  domain_name_configuration {
    certificate_arn = aws_acm_certificate_validation.main[each.key].certificate_arn
    endpoint_type   = "REGIONAL"
    security_policy = "TLS_1_2"
  }
}

resource "aws_apigatewayv2_api_mapping" "main" {
  for_each    = local.api_gateway_domains
  api_id      = each.value.backend.api_id
  domain_name = aws_apigatewayv2_domain_name.main[each.key].domain_name
  stage       = each.value.backend.stage_name
}
```

The `var.domains` type changes from `rest_api_id` / `stage_name` to `api_id` /
`stage_name`.

#### Modified Module: `infra/terraform/modules/lambda/`

**`function.tf`**: Add the WorkOS API key environment variable. Remove
`AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH` (not needed for HTTP API).

```hcl
environment {
  variables = {
    TOKENOVERFLOW_ENV                       = var.tokenoverflow_env
    TOKENOVERFLOW_DATABASE_PASSWORD         = data.aws_ssm_parameter.db_password.value
    TOKENOVERFLOW_EMBEDDING_API_KEY         = data.aws_ssm_parameter.embedding_key.value
    TOKENOVERFLOW_WORKOS_API_KEY       = data.aws_ssm_parameter.auth_workos_api_key.value
  }
}
```

**`variables.tf`**: Add the new SSM parameter variable:

```hcl
variable "auth_workos_api_key_ssm_name" {
  description = "SSM parameter name for the WorkOS API key"
  type        = string
}
```

#### Deleted Module: `infra/terraform/modules/api_gateway/`

The entire REST API module is removed after the HTTP API module is deployed and
the custom domain is pointed to it. Files deleted:

- `api.tf` (REST API, resources, methods, integrations)
- `usage_plans.tf` (Usage Plans)
- `stage.tf` (REST API deployment, stage, CloudWatch)
- `permissions.tf` (Lambda permission)
- `variables.tf` (including `usage_plans` variable)
- `outputs.tf` (REST API outputs)

#### Terragrunt Updates

**`live/prod/http_api/terragrunt.hcl`** (new, replaces `api_gateway`):

```hcl
inputs = {
  env_name             = "prod"
  lambda_invoke_arn    = dependency.lambda.outputs.invoke_arn
  lambda_function_name = dependency.lambda.outputs.function_name
  jwt_issuer           = "https://<authkit_domain>"
  jwt_audience         = ["https://api.tokenoverflow.io"]
  default_rate_limit   = 500
  default_burst_limit  = 1000
  log_retention_days   = 14
}
```

**`live/prod/dns/terragrunt.hcl`**: Update dependency from `api_gateway` to
`http_api`, update output references from `rest_api_id` to `api_id`.

**`live/prod/lambda/terragrunt.hcl`**: Add `auth_workos_api_key_ssm_name`
input.

#### WorkOS Configuration (Dashboard-Only)

WorkOS AuthKit setup is configured entirely through the WorkOS dashboard and is
not managed by Terraform. The following settings are configured manually as a
one-time setup:

- **GitHub OAuth provider**: Enable GitHub as a social login provider
- **Redirect URIs**: Set callback URLs for the application
  (`https://api.tokenoverflow.io/auth/callback`)
- **Client ID**: Generated by WorkOS, referenced in Terraform as `jwt_issuer`
  and in application config
- **JWT issuer and audience**: Configured in the AuthKit settings, must match
  the `jwt_issuer` and `jwt_audience` values in the HTTP API Gateway module

The community Terraform provider
[`osodevops/terraform-provider-workos`](https://registry.terraform.io/providers/osodevops/terraform-provider-workos)
was evaluated for managing WorkOS configuration as IaC. The provider passed a
security audit (legitimate UK consultancy, no vulnerabilities, proper
practices), but it manages resources (organizations, users, roles, permissions)
that are not relevant to TokenOverflow's current needs. The AuthKit settings
TokenOverflow depends on (OAuth providers, redirect URIs, JWT configuration)
are not exposed via the WorkOS API and therefore cannot be managed by any
Terraform provider.

If organization management or RBAC features enter scope in the future, this
provider should be re-evaluated.

---

## Monitoring and Observability

### Access Log Fields

The HTTP API access log format includes auth-specific fields:

| Field | Purpose |
|---|---|
| `$context.authorizer.status` | JWT authorizer validation result |
| `$context.authorizer.error` | JWT authorizer error message (if any) |
| `$context.status` | HTTP status code (401 = auth failure, 429 = throttled) |
| `$context.requestId` | Request ID for tracing |

### Key Metrics to Monitor

| What | How | Alert? |
|---|---|---|
| 401 rate | Access logs where status = 401 | Spike detection (possible brute force) |
| 429 rate | Access logs where status = 429 | Yes -- means global throttle is being hit |
| JWT authorizer errors | Access logs where authorizer.error is set | Yes -- could indicate JWKS fetch issues |
| JIT provisioning failures | Application logs (Lambda CloudWatch) | Yes -- users cannot sign up |

---

## Documentation Changes

### README.md

Add to the Architecture section:

- Authentication provider: WorkOS AuthKit (GitHub OAuth)
- MCP OAuth 2.1 support for AI coding tool authentication
- JWT-only auth (no user-facing API keys)
- HTTP API Gateway with built-in JWT authorizer

### Config Table

Update the API Configuration section to document the new `[auth]` config
section and the `TOKENOVERFLOW_WORKOS_API_KEY` environment variable.

---

## Development Environment Changes

### Local Development Auth Strategy

For local development, the API uses the **test JWKS** (loaded via `file://`
protocol) rather than connecting to WorkOS. This avoids requiring a WorkOS
account for local development.

The approach:

1. A static RSA key pair is checked into the repository (test-only, not a
   secret)
2. `config/local.toml` points `jwks_url` to
   `file://tests/assets/auth/test_jwks.json`
3. A helper script (`scripts/src/dev_token.sh`) generates test JWTs signed
   with the test private key
4. Developers use these test JWTs for local MCP testing

```toml
# config/local.toml additions
[auth]
workos_client_id = "client_test"
workos_api_url = "http://localhost:8080"
jwks_url = "file://tests/assets/auth/test_jwks.json"
jwks_cache_ttl_secs = 0
issuer = "tokenoverflow-test"
audience = "http://localhost:8080"
```

#### Bruno: Zero-Setup Local Testing

Bruno requires no manual auth setup for local development. The repository ships
with a **pre-committed, long-lived test JWT** in the `local` environment file
and collection-level Bearer auth — select the `local` environment and start
making requests.

**Collection-level auth** (`opencollection.yml`):

```yaml
auth:
  mode: bearer
  bearer:
    token: "{{auth_token}}"
```

All requests inherit this via `auth: inherit` (Bruno's default), so no
per-request auth configuration is needed.

**Pre-committed token** (`local.yml`):

```yaml
name: local
variables:
  - name: base_url
    value: http://localhost:8080
  - name: auth_token
    value: <pre-generated-long-lived-test-jwt>
```

The committed JWT is generated by `dev_token.sh` with a **10-year expiry** and
these claims:

| Claim | Value | Why |
|-------|-------|-----|
| `sub` | `system` | Matches the seeded system user |
| `iss` | `tokenoverflow-test` | Matches `local.toml` issuer config |
| `aud` | `http://localhost:8080` | Matches `local.toml` audience config |
| `exp` | 10 years from generation | Effectively permanent for dev use |
| `kid` | `test-key-1` | Matches the test JWKS key ID |

Committing this JWT is safe: it is signed with the test private key (already in
the repo), uses a test-only issuer and audience that production rejects, and
anyone with repo access can regenerate it. See
[`dev_token.sh` Reference](#dev_tokensh-reference) for regeneration.

#### Claude Code: Zero-Setup MCP Testing

The project-level `.mcp.json` is committed with a pre-generated test JWT,
identical to the Bruno approach. Clone the repo, start the local server, and
Claude Code connects immediately.

**`.mcp.json`** (committed):

```json
{
  "mcpServers": {
    "tokenoverflow-local": {
      "type": "streamable-http",
      "url": "http://localhost:8080",
      "headers": {
        "Authorization": "Bearer <pre-generated-long-lived-test-jwt>"
      }
    }
  }
}
```

The static `Authorization` header bypasses OAuth discovery, sending the test JWT
directly with every MCP request. No browser-based OAuth flow, no WorkOS account
needed.

The committed token uses the same claims and **10-year expiry** as the Bruno
token (see table above). Committing it is safe for the same reasons: test-only
key, test issuer/audience rejected by production, and anyone with repo access
can regenerate it.

**What this enables**:

- Claude Code can discover tools, read resources, and call endpoints on the
  local TokenOverflow server
- The same `jwt_auth` middleware validates the token (identical code path to
  production)
- JIT provisioning creates a local user on first request (using the mock WorkOS
  response configured in `local.toml`)

#### `dev_token.sh` Reference

The `scripts/src/dev_token.sh` helper generates test JWTs:

```bash
# Generate a token and print to stdout (default: 1h expiry)
./scripts/src/dev_token.sh

# Generate a long-lived token (for Bruno local.yml and .mcp.json)
./scripts/src/dev_token.sh --expiry 10y

# Custom claims
./scripts/src/dev_token.sh --sub user_custom_123 --expiry 24h
```

The script reads `apps/api/tests/assets/auth/test_private_key.pem` and
`apps/api/config/local.toml` (for issuer, audience, kid) to produce a valid
JWT. It requires `openssl` and `jq` (both available via Homebrew and in the
`Brewfile`).

**Regenerating committed tokens** (only needed if claims or the test key pair
change):

```bash
./scripts/src/dev_token.sh --expiry 10y
# Update auth_token in local.yml and the Bearer token in .mcp.json
```

### Environment Variables

```bash
# Local dev -- not needed (file:// JWKS, mock WorkOS for JIT provisioning)
# TOKENOVERFLOW_WORKOS_API_KEY is not set locally

# Production (stored in SSM Parameter Store)
TOKENOVERFLOW_WORKOS_API_KEY=sk_live_...
```

### Docker Compose

No new services required. The test JWKS is loaded from the filesystem.

---

## Out of Scope / Future Work

The following items are explicitly deferred to separate design documents:

### Per-User Rate Limiting / Paid Tiers

Per-user rate limiting (Free: 100 req/day, Pro: 10K req/day, Enterprise:
custom) is not part of this design. The current architecture uses global
throttling only.

When needed, per-user rate limiting will be implemented as DynamoDB-based
counters in the Rust application middleware (not API Gateway Usage Plans). This
approach scales without limits and supports flexible tier logic.

### WAF (Web Application Firewall)

AWS WAF can be attached to the HTTP API for IP-based rate limiting, geo-blocking,
SQL injection protection, etc. This is not needed for MVP but can be added as a
separate Terraform resource without code changes.

### Enterprise SSO / SAML

WorkOS supports SAML and SCIM for enterprise customers. This is a future
feature that can be enabled in the WorkOS dashboard without code changes to the
JWT validation logic (WorkOS issues the same JWT format regardless of the
upstream identity provider).

---

## Tasks

### Owner Setup Guide (Engineer: Walk the Owner Through These)

Several steps require the project owner to act in the WorkOS dashboard and AWS
console. These are not automatable. The engineer should guide the owner through
each step at the appropriate point during implementation. Steps are ordered by
when they're needed.

#### Step 1: Create WorkOS Account and Project

**When**: Before starting Task 6 (AuthService needs real config values for
`development.toml` and `production.toml`).

1. Go to [workos.com](https://workos.com) and sign up
2. Create a new project (e.g., "TokenOverflow")
3. Note down the following from the WorkOS dashboard:
   - **Client ID** (`client_...`) — found in the API Keys section
   - **API Key** (`sk_live_...`) — found in the API Keys section

**Owner provides to engineer**:

| Value | Where it goes |
|-------|---------------|
| Client ID | `production.toml` → `workos_client_id`, JWKS URL |
| API Key | AWS SSM Parameter Store (see Step 5) |

#### Step 2: Enable GitHub OAuth in WorkOS

**When**: Same as Step 1 (part of initial WorkOS setup).

1. In the WorkOS dashboard, go to **Authentication** → **OAuth providers** →
   **GitHub** → **Manage** and copy the **Redirect URI**
2. In GitHub, go to **Settings** → **Developer settings** → **OAuth Apps** →
   **New OAuth App**
3. Set the **Authorization callback URL** to the WorkOS Redirect URI from
   step 1
4. Register the app, copy the **Client ID**, and generate a **Client Secret**
5. Back in the WorkOS dashboard, select **Your app's credentials** and paste
   the GitHub Client ID and Client Secret

#### Step 3: Configure Redirect URIs

**When**: Before deploying to any cloud environment (Task 11+).

1. In the WorkOS dashboard, go to **Redirects**
2. Add the following redirect URIs:

| URI | Environment |
|-----|-------------|
| `https://api.tokenoverflow.io/auth/callback` | Production |
| `https://dev.api.tokenoverflow.io/auth/callback` | Development (if applicable) |

Do **not** add `http://localhost:*` — local development uses test JWTs and
never talks to WorkOS.

#### Step 4: Verify JWT Configuration in WorkOS

**When**: Before deploying Task 11 (Terraform HTTP API module needs the exact
issuer and JWKS URL).

1. In the WorkOS dashboard, go to **AuthKit** > **Custom Domains** section
2. Confirm the **AuthKit domain** (e.g., `<slug>.authkit.app`)
3. The **OIDC issuer** is `https://<authkit_domain>` and **JWKS URL** is `https://<authkit_domain>/oauth2/jwks`

The engineer will use these values in:
- `production.toml` → `jwks_url` and `issuer`
- Terraform → `jwt_issuer` input for the HTTP API Gateway JWT authorizer
- Well-known endpoint → `authorization_servers` array

#### Step 5: Store API Key in AWS SSM Parameter Store

**When**: Before deploying Task 13 (Lambda needs the API key at runtime).

1. Open the AWS console → **Systems Manager** → **Parameter Store**
2. Create a new parameter:
   - **Name**: `/tokenoverflow/prod/workos_api_key`
   - **Type**: `SecureString`
   - **Value**: the `sk_live_...` API key from Step 1
3. Confirm the parameter name matches what's in the Terragrunt config
   (`auth_workos_api_key_ssm_name` input)

The engineer cannot do this step — it requires the real API key which should
never be shared in plain text (Slack, email, etc.). The owner enters it
directly in the AWS console.

#### Step 6: Verify End-to-End in Production

**When**: After all Terraform tasks (11-14) are deployed and application code
is live.

1. Open a browser and navigate to
   `https://api.tokenoverflow.io/.well-known/oauth-protected-resource`
2. Confirm the response contains the correct `authorization_servers` URL
3. Test the full OAuth flow:
   - Use an MCP client (Claude Code, Cursor, etc.) pointed at
     `https://api.tokenoverflow.io`
   - The client should trigger a browser-based GitHub login via WorkOS
   - After login, the client should receive a JWT and make authenticated
     requests
4. Check CloudWatch logs to confirm:
   - JWT authorizer is passing valid tokens
   - JIT provisioning created a user record on first login

#### Quick Reference: What Goes Where

| WorkOS Value | Config Location | Who Enters It |
|-------------|-----------------|---------------|
| Client ID (`client_...`) | `production.toml`, JWKS URL | Engineer (in code, from owner) |
| API Key (`sk_live_...`) | AWS SSM Parameter Store | **Owner** (in AWS console) |
| JWKS URL | `production.toml` (derived from Client ID) | Engineer |
| Redirect URIs | WorkOS dashboard | **Owner** (in WorkOS dashboard) |
| GitHub OAuth | WorkOS dashboard | **Owner** (toggle in WorkOS dashboard) |

### Task 1: Database Migration

**Files**:

- `apps/api/migrations/<timestamp>_auth/up.sql`
- `apps/api/migrations/<timestamp>_auth/down.sql`

Add `workos_id`, `github_id`, `github_username`, `display_name`, `avatar_url`
columns. Remove `cognito_sub` and `email`. Backfill system user.

**Success criteria**: Migration applies cleanly on local Docker stack.
`diesel print-schema` generates updated schema. Existing system user preserved
with `workos_id = 'system'`.

### Task 2: Diesel Schema and User Model

**Files**:

- `apps/api/src/db/schema.rs` (regenerated by diesel CLI)
- `apps/api/src/db/models/user.rs` (new)
- `apps/api/src/db/models/mod.rs` (add `pub use user::*`)

Define `User` (Queryable) and `NewUser` (Insertable) structs matching the new
schema.

**Success criteria**: `cargo build` compiles. Unit tests for `User` struct
creation pass.

### Task 3: UserRepository

**Files**:

- `apps/api/src/services/repository/interface/user.rs` (new trait)
- `apps/api/src/services/repository/interface/mod.rs` (add re-export)
- `apps/api/src/services/repository/postgres/user.rs` (new impl)
- `apps/api/src/services/repository/postgres/mod.rs` (add re-export)

Trait methods: `find_by_workos_id`, `create`, `update_github_profile`.

**Success criteria**: Integration tests (testcontainers) pass for create, find,
and upsert flows.

### Task 4: Test JWKS and Test JWT Generator

**Files**:

- `apps/api/tests/assets/auth/test_jwks.json` (new -- static test public key
  in JWKS format)
- `apps/api/tests/assets/auth/test_private_key.pem` (new -- static test RSA
  private key)
- `apps/api/tests/common/test_jwt.rs` (new -- Rust helper to generate test
  JWTs)
- `apps/api/tests/common/mod.rs` (add re-export)

Generate an RSA key pair. Export the public key as JWKS JSON. Export the private
key as PEM. Write the test JWT generator function.

**Success criteria**: `generate_test_jwt("user_test123", 3600)` produces a JWT
that validates against `test_jwks.json` using the `jsonwebtoken` crate.

### Task 5: AuthConfig and Config Changes

**Files**:

- `apps/api/src/config.rs` (add `AuthConfig`)
- `apps/api/config/local.toml` (add `[auth]` section with `file://` JWKS)
- `apps/api/config/unit_test.toml` (add `[auth]` section with `file://` JWKS)
- `apps/api/config/development.toml` (add `[auth]` section)
- `apps/api/config/production.toml` (add `[auth]` section)

**Success criteria**: `Config::load()` succeeds with the new `[auth]` section.
Unit tests pass. `file://` JWKS URL is accepted.

### Task 6: AuthService (JWKS + JWT + User Resolution)

**Files**:

- `apps/api/src/services/auth.rs` (new)
- `apps/api/src/services/mod.rs` (add re-export)

Implements: JWKS loading (with `file://` protocol support), JWT signature
verification, claim validation, user resolution (lookup or create via WorkOS
API on first login). The same validation logic runs in all environments.

**Success criteria**: Unit tests with test JWKS pass for all JWT validation
scenarios (valid, expired, wrong issuer, wrong audience, unknown kid, `file://`
loading).

### Task 7: Auth Middleware and AuthenticatedUser Extractor

**Files**:

- `apps/api/src/api/middleware.rs` (add `jwt_auth`; update `trace_id` for v2
  payload format)
- `apps/api/src/api/extractors.rs` (new -- `AuthenticatedUser` extractor)
- `apps/api/src/error.rs` (add `Unauthorized` and `Forbidden` variants)

The `jwt_auth` middleware always validates JWTs from the `Authorization` header
using `AuthService`. Single code path in all environments — no environment
detection or `RequestContext` branching for auth. Resolves the local user and
injects `AuthenticatedUser`.

The `trace_id` middleware must be updated to handle
`RequestContext::ApiGatewayV2` in addition to (or instead of)
`RequestContext::ApiGatewayV1` (this is a trace_id concern, not auth).

**Success criteria**: Unit tests for token extraction, error responses (401 with
correct WWW-Authenticate header), and user injection pass.

### Task 8: Well-Known Metadata Endpoints

**Files**:

- `apps/api/src/api/routes/well_known.rs` (new)
- `apps/api/src/api/routes/mod.rs` (add module)
- `apps/api/src/api/routes/configure.rs` (add routes)

Implements `GET /.well-known/oauth-protected-resource` returning the metadata
JSON with the WorkOS authorization server URL.

**Success criteria**: E2E test confirms correct JSON response with valid
`authorization_servers` and `resource` fields.

### Task 9: Wire Auth Middleware to Routes

**Files**:

- `apps/api/src/api/routes/configure.rs` (add middleware layers)
- `apps/api/src/api/server.rs` (add AuthService to AppState, update CORS for
  Authorization header)

Apply `jwt_auth` middleware to `/mcp` and `/v1/*` route groups.
Leave only `/health` and `/.well-known/*` public.

**Success criteria**: E2E tests pass. Unauthenticated requests to protected
endpoints return 401. Authenticated requests with test JWTs succeed.

### Task 10: Replace SYSTEM_USER_ID in Services

**Files**:

- `apps/api/src/api/routes/questions.rs`
- `apps/api/src/api/routes/answers.rs`
- `apps/api/src/api/routes/search.rs`
- `apps/api/src/mcp/tools/submit.rs`
- `apps/api/src/mcp/tools/upvote_answer.rs`

Extract the authenticated user ID from `AuthenticatedUser` in request
extensions and pass it through the service layer instead of `SYSTEM_USER_ID`.

**Success criteria**: All existing E2E tests pass with authentication. Questions
and answers are attributed to the authenticated user.

### Task 11: Terraform -- HTTP API Module

**Files**:

- `infra/terraform/modules/http_api/api.tf` (new)
- `infra/terraform/modules/http_api/authorizer.tf` (new)
- `infra/terraform/modules/http_api/stage.tf` (new)
- `infra/terraform/modules/http_api/routes.tf` (new)
- `infra/terraform/modules/http_api/permissions.tf` (new)
- `infra/terraform/modules/http_api/variables.tf` (new)
- `infra/terraform/modules/http_api/outputs.tf` (new)
- `infra/terraform/live/prod/http_api/terragrunt.hcl` (new)

Create the HTTP API Gateway Terraform module with built-in JWT authorizer,
stage-level throttling, and CloudWatch access logs. Route configuration as
described in the Terraform Changes section.

**Success criteria**: `terragrunt plan` shows the new HTTP API, JWT authorizer,
stage, routes, and Lambda permission. No changes to existing resources.

### Task 12: Terraform -- DNS Module Update

**Files**:

- `infra/terraform/modules/dns/api_gateway.tf` (modify: switch to
  apigatewayv2 resources)
- `infra/terraform/modules/dns/variables.tf` (modify: update backend type)
- `infra/terraform/live/prod/dns/terragrunt.hcl` (modify: update dependency
  and inputs)

Switch custom domain resources from REST API (`aws_api_gateway_domain_name`,
`aws_api_gateway_base_path_mapping`) to HTTP API
(`aws_apigatewayv2_domain_name`, `aws_apigatewayv2_api_mapping`).

**Success criteria**: `terragrunt plan` shows the domain name and API mapping
changes. `api.tokenoverflow.io` points to the HTTP API.

### Task 13: Terraform -- Lambda Environment Variable

**Files**:

- `infra/terraform/modules/lambda/variables.tf` (add SSM variable)
- `infra/terraform/modules/lambda/function.tf` (add env var, remove
  `AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH`)
- `infra/terraform/live/prod/lambda/terragrunt.hcl` (add SSM input)

Add `TOKENOVERFLOW_WORKOS_API_KEY` environment variable to the Lambda
function, sourced from SSM Parameter Store. Remove
`AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH` (not needed for HTTP API).

**Success criteria**: `terragrunt plan` shows the environment variable changes.
No other resource changes.

### Task 14: Terraform -- Delete Old REST API Module

**Files**:

- `infra/terraform/modules/api_gateway/` (delete entire directory)
- `infra/terraform/live/prod/api_gateway/` (delete entire directory)

Only after Tasks 11 and 12 are deployed and verified.

**Success criteria**: `terragrunt plan` shows the old REST API, usage plans,
stage, and related resources being destroyed. The HTTP API continues to serve
traffic.

### Task 15: Bruno Testing Setup

**Files**:

- `bruno/TokenOverflow/collections/api/opencollection.yml` (add collection-level
  Bearer auth using `{{auth_token}}`)
- `bruno/TokenOverflow/collections/api/environments/local.yml` (add `base_url`
  and `auth_token` variables with pre-generated long-lived test JWT)
- `bruno/TokenOverflow/collections/api/environments/prod.yml` (add `auth_token`
  variable)
- `bruno/TokenOverflow/collections/api/well_known.yml` (new)

Set up Bruno collections for testing authenticated endpoints. The `local`
environment ships with a pre-committed 10-year test JWT so developers have
zero-setup auth. Cloud environments require manually pasting a real WorkOS
token.

**Success criteria**: Opening Bruno with the `local` environment and hitting any
authenticated endpoint works immediately — no manual token generation. Bruno can
hit `/.well-known/oauth-protected-resource` locally and get a valid response.

### Task 16: Documentation and Dev Tooling

**Files**:

- `README.md` (update Architecture section)
- `scripts/src/dev_token.sh` (new -- generates test JWTs for local dev using
  the test private key; supports `--expiry` and `--sub` flags)
- `.mcp.json` (new -- committed with pre-generated long-lived test JWT for
  Claude Code local MCP testing)

Implements the `dev_token.sh` helper that supports:
- Default: generate a 1-hour test JWT to stdout
- `--expiry 10y`: long-lived token for Bruno `local.yml` and `.mcp.json`
- `--sub <user_id>`: custom subject claim

Generate the initial long-lived tokens and commit them in `local.yml` and
`.mcp.json`.

**Success criteria**: New developer can follow README to get auth working
locally using test JWTs. `./scripts/src/dev_token.sh` outputs a valid JWT.
Clone-and-go: Bruno `local` environment and Claude Code MCP both work
immediately without any manual token setup.

---

## Appendix: Alternatives Research

### Evaluation Criteria

Each solution is evaluated against the user's requirements:

- **Cost at scale**: What does it cost at 10K, 50K, 100K MAU?
- **GitHub OAuth support**: Native or requires workarounds?
- **Maintenance burden**: How much operational work after initial setup?
- **SOC2 path**: Is the provider SOC2 compliant, or can the self-hosted setup
  achieve compliance?
- **Rust/Axum compatibility**: Native SDK, or JWT verification only?
- **Device flow support**: Does it support RFC 8628 for future CLI auth?
- **AWS ecosystem fit**: How well does it integrate with the existing AWS infra?

### Scoring Matrix (1-5, higher is better)

| Criteria (Weight) | WorkOS | Auth0 | Logto | Clerk | Descope | Kinde | Firebase | Hanko | Stack Auth |
|---|---|---|---|---|---|---|---|---|---|
| Cost at scale (30%) | 5 | 4 | 4 | 3 | 2 | 2 | 4 | 3 | 4 |
| Maintenance (25%) | 5 | 5 | 4 | 5 | 5 | 5 | 5 | 3 | 3 |
| GitHub OAuth (15%) | 5 | 5 | 5 | 5 | 5 | 5 | 5 | 5 | 4 |
| SOC2 path (10%) | 5 | 5 | 5 | 5 | 5 | 5 | 5 | 2 | 2 |
| Device flow (10%) | 5 | 5 | 2 | 1 | 5 | 5 | 1 | 1 | 1 |
| Reliability (10%) | 5 | 5 | 4 | 5 | 4 | 4 | 5 | 3 | 3 |
| **Weighted Score** | **5.00** | **4.70** | **3.85** | **3.65** | **3.70** | **3.55** | **3.95** | **2.80** | **3.00** |

### Solution Summaries

21 solutions were evaluated. The top contenders and key eliminations:

| Solution | Free Tier | GitHub OAuth | Device Flow | SOC2 | Verdict |
|---|---|---|---|---|---|
| **WorkOS AuthKit** | 1M MAU | Native | Yes (RFC 8628) | Type II | **Selected** |
| Auth0 | 25K MAU | Native | Yes | Type II | Strong but pricing unpredictable (300% hike in 2023) |
| Logto | 50K MAU | Native | Uncertain | Type II | Third choice |
| Firebase Auth | 50K MAU | Native | No | SOC2 (Google) | No device flow, cross-cloud dependency |
| AWS Cognito | 50 MAU (GitHub via OIDC) | Requires OIDC shim | No | Yes (AWS) | $0.015/MAU for GitHub, no device flow |
| Clerk | 50K MRU | Native | No | Type II | No device flow |
| Keycloak | Free (self-hosted) | Native | Yes | Self-managed | High maintenance, contradicts requirements |
| Descope | 7.5K MAU | Native | Yes | Type II | Steep pricing at scale |
| Kinde | 10.5K MAU | Native | Yes | Type II | $677/mo at 50K MAU |
| Stytch | 10K MAU | Native | Uncertain | Type II | $900/mo at 100K MAU |
| SuperTokens | Free (self-hosted) | Native | No | Self-managed | Java core, $900/mo cloud |
| Hanko | 10K MAU | Native | No | No | Missing device flow and SOC2 |
| Stack Auth | 10K users | Native | No | No | Immature, missing requirements |
| BetterAuth | Free (library) | Native | No | N/A | TypeScript only, Rust port immature |
| Lucia Auth | Deprecated | N/A | N/A | N/A | Deprecated |
| Remaining (Authgear, Zitadel, Ory, FusionAuth, PropelAuth, Corbado) | Various | Various | Various | Various | Eliminated on cost, maintenance, or missing features |
