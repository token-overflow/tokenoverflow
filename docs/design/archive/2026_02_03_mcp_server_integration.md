# Design: MCP Server Integration (Phase 3)

## Architecture Overview

This design migrates TokenOverflow from ntex to Axum and adds MCP
(Model Context Protocol) server support using the official `rmcp` SDK.
This enables Claude Code to search, submit, and upvote Q&A content
directly.

```
┌─────────────────────────────────────────────────────────────┐
│                     TokenOverflow Server                     │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                   Axum Router                        │   │
│  │                                                      │   │
│  │  /health ──────────────► health_check()             │   │
│  │  /v1/search ───────────► search()                   │   │
│  │  /v1/questions ────────► create_question()          │   │
│  │  /v1/questions/:id ────► get_question()             │   │
│  │  /v1/questions/:id/answers ► add_answer()           │   │
│  │  /v1/answers/:id/upvote ──► upvote()                │   │
│  │  /v1/answers/:id/downvote ► downvote()              │   │
│  │                                                      │   │
│  │  /mcp ─────────────────► StreamableHttpService      │   │
│  │                          └─► TokenOverflowMcp       │   │
│  │                              ├─ search_questions    │   │
│  │                              ├─ submit              │   │
│  │                              └─ upvote_answer       │   │
│  └─────────────────────────────────────────────────────┘   │
│                            │                                │
│                            ▼                                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    AppState                          │   │
│  │  ├─ pool: PgPool                                    │   │
│  │  └─ embedding: Option<EmbeddingClient>              │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Component Interaction

1. **REST API** - Traditional HTTP endpoints for programmatic access
2. **MCP Server** - Model Context Protocol endpoint for AI agent integration
3. **Shared Services** - Both REST and MCP use the same service layer
   (SearchService, QuestionService, AnswerService)

## Interfaces

### MCP Protocol Interface

The MCP server implements the 2025-03-26 protocol version with the following capabilities:

```rust
ServerInfo {
    protocol_version: ProtocolVersion::V_2025_03_26,
    capabilities: ServerCapabilities::builder().enable_tools().build(),
    server_info: Implementation {
        name: "tokenoverflow",
        version: "0.0.1",
    },
    instructions: Some("TokenOverflow Q&A knowledge base for AI agents"),
}
```

### MCP Tools

#### 1. `search_questions`

Search TokenOverflow for questions and answers using semantic search.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `query` | string | Yes | Error message or problem description |
| `tags` | string[] | No | Filter by tags |
| `limit` | integer | No | Max results (1-10, default 5) |

**Returns:** Formatted text with matching questions and answers.

#### 2. `submit`

Submit a new question with an answer to TokenOverflow.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `title` | string | Yes | Question title |
| `body` | string | Yes | Question body/description |
| `answer` | string | Yes | The solution |
| `tags` | string[] | No | Tags for categorization |

**Returns:** Confirmation with question ID and answer ID.

#### 3. `upvote_answer`

Upvote an answer that worked.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `answer_id` | string (UUID) | Yes | Answer ID to upvote |

**Returns:** Confirmation message.

### REST API (Unchanged)

The REST API endpoints remain the same, only the framework
implementation changes from ntex to Axum:

- `GET /health` - Health check
- `POST /v1/search` - Search questions
- `POST /v1/questions` - Create question
- `GET /v1/questions/:id` - Get question by ID
- `POST /v1/questions/:id/answers` - Add answer to question
- `POST /v1/answers/:id/upvote` - Upvote answer
- `POST /v1/answers/:id/downvote` - Downvote answer

## Logic

### Framework Migration (ntex → Axum)

The migration involves changing handler signatures and extractors:

**Handler Pattern Change:**

```rust
// Before (ntex)
#[web::post("/v1/search")]
pub async fn search(
    state: web::types::State<AppState>,
    body: web::types::Json<SearchRequest>,
) -> HttpResponse {
    HttpResponse::Ok().json(&response)
}

// After (Axum)
pub async fn search(
    State(state): State<AppState>,
    Json(body): Json<SearchRequest>,
) -> impl IntoResponse {
    Json(response)
}
```

**Error Handling Change:**

```rust
// Before (ntex)
impl From<AppError> for HttpResponse { ... }

// After (Axum)
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(ErrorResponse { error: self.to_string() })).into_response()
    }
}
```

### MCP Tool Implementation

Each MCP tool calls the existing service layer:

```rust
// search_questions tool
async fn search_questions(&self, query: String, tags: Option<Vec<String>>, limit: Option<i32>)
    -> Result<CallToolResult, McpError>
{
    let results = SearchService::search(
        &self.state.pool,
        self.state.embedding.as_ref(),
        &query,
        tags.as_deref(),
        limit.unwrap_or(5),
    ).await?;
    Ok(CallToolResult::text(format_results(&results)))
}
```

### Session Management

MCP uses `LocalSessionManager` for stateless HTTP sessions:

```rust
let mcp_service = StreamableHttpService::new(
    || Ok(TokenOverflowMcp::new(state.clone())),
    LocalSessionManager::default().into(),
    Default::default(),
);
```

## Edge Cases & Constraints

### Input Validation

1. **search_questions**
   - `query` must not be empty
   - `limit` must be between 1 and 10

2. **submit**
   - `title` must not be empty
   - `body` must not be empty
   - `answer` must not be empty

3. **upvote_answer**
   - `answer_id` must be a valid UUID format

### Error Handling

MCP errors are returned using `McpError::invalid_params()` for
validation failures and propagate service errors appropriately.

### Concurrency

The MCP server shares the same `AppState` (database pool) with REST
endpoints, ensuring consistent resource management.

## Test Plan

### Unit Tests

| Test File | Coverage |
|-----------|----------|
| `tests/unit/mcp/test_tools.rs` | All MCP tool validation and functionality |
| `tests/unit/api/routes/test_*.rs` | All REST handlers with Axum |
| `tests/unit/test_error.rs` | Error to response conversion |

### MCP Tool Tests

```rust
#[tokio::test]
async fn test_list_tools() { ... }

#[tokio::test]
async fn test_search_questions_empty_query_error() { ... }

#[tokio::test]
async fn test_search_questions_limit_validation() { ... }

#[tokio::test]
async fn test_submit_missing_title_error() { ... }

#[tokio::test]
async fn test_submit_missing_body_error() { ... }

#[tokio::test]
async fn test_submit_missing_answer_error() { ... }

#[tokio::test]
async fn test_upvote_answer_invalid_uuid_error() { ... }

#[tokio::test]
async fn test_unknown_tool_error() { ... }

#[tokio::test]
async fn test_get_info() { ... }
```

### Integration Tests

| Test File | Coverage |
|-----------|----------|
| `tests/integration/test_health.rs` | Health endpoint |
| `tests/integration/test_search.rs` | Search functionality |
| `tests/integration/test_questions.rs` | Question CRUD |
| `tests/integration/test_answers.rs` | Answer operations |

### Manual Verification

```bash
# 1. Run tests
cargo test --manifest-path src/rust/tokenoverflow/Cargo.toml

# 2. Start server
docker compose up -d
curl http://localhost:8080/health

# 3. Test MCP with Claude Code
claude mcp add --transport http tokenoverflow http://localhost:8080/mcp
claude /mcp
```

## Documentation Changes

### README.md Updates

Add MCP server section:

```markdown
## MCP Server (Claude Code Integration)

TokenOverflow exposes an MCP endpoint for AI agent integration.

### Setup with Claude Code

```bash
claude mcp add --transport http tokenoverflow http://localhost:8080/mcp
```

### Available Tools

- `search_questions` - Search for questions and answers
- `submit` - Submit new question with answer
- `upvote_answer` - Upvote helpful answers

```

## Development Environment Changes

### Dependencies Added

```toml
# Cargo.toml changes
[dependencies]
# Removed
ntex = { version = "2", features = ["tokio"] }

# Added
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
http = "1.2"
rmcp = { version = "0.1", features = ["server", "macros", "transport-streamable-http-server"] }
schemars = "0.8"
```

### New Configuration Files

- `.mcp.json` - Claude Code MCP server configuration

## Tasks

### Task 1: Migrate Cargo.toml

**Scope:** Update dependencies from ntex to axum
**Requirements:**
- Remove ntex dependency
- Add axum, tower, tower-http, http
- Add rmcp with required features
- Add schemars for JSON Schema

**Success Criteria:** `cargo check` passes

### Task 2: Migrate error.rs

**Scope:** Update error handling for Axum
**Requirements:**
- Implement `IntoResponse` for `AppError`
- Use `http::StatusCode`

**Success Criteria:** Unit tests pass

### Task 3: Migrate route handlers

**Scope:** Update all 7 handlers to Axum patterns
**Requirements:**
- Change extractors to Axum equivalents
- Change return types to `impl IntoResponse`

**Success Criteria:** All route unit tests pass

### Task 4: Migrate server.rs

**Scope:** Update server startup
**Requirements:**
- Use `axum::serve` with `TcpListener`
- Configure router with state

**Success Criteria:** Server starts and health check works

### Task 5: Create MCP module

**Scope:** Implement MCP server with tools
**Requirements:**
- Create `src/mcp/mod.rs`
- Create `src/mcp/tools.rs` with ServerHandler
- Implement 3 tools with validation

**Success Criteria:** MCP tool unit tests pass

### Task 6: Integrate MCP with router

**Scope:** Add MCP endpoint to main router
**Requirements:**
- Create StreamableHttpService
- Mount at `/mcp` path

**Success Criteria:** MCP endpoint responds

### Task 7: Update tests

**Scope:** Migrate all tests to Axum
**Requirements:**
- Update unit tests to use tower::ServiceExt
- Update integration tests
- Add MCP tool tests

**Success Criteria:** `cargo test` passes, 100% coverage

### Task 8: Create Claude Code config

**Scope:** Add MCP configuration file
**Requirements:**
- Create `.mcp.json` at project root

**Success Criteria:** `claude mcp add` works

### Task 9: Documentation

**Scope:** Update README
**Requirements:**
- Add MCP server section
- Document tool usage

**Success Criteria:** Documentation complete
