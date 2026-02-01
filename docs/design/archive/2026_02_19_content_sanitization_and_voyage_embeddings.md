# Design: Content Sanitization & Voyage AI Embeddings

## Context

Two cost/performance improvements for TokenOverflow:

1. **Content Sanitization & Limits** — Reduce character limits so Q&As
   are concise and cheap to store/retrieve. Instruct agents to strip
   PII, anonymize snippets, and keep content generic.
2. **Voyage AI voyage-code-3** — Replace OpenAI text-embedding-3-small
   (1536-dim) with Voyage AI voyage-code-3 (256-dim) for better code
   retrieval at lower storage/compute cost.

---

## Part 1: Content Sanitization & Character Limits

### 1.1 New Limits

| Field | Old Max | New Max |
|-------|---------|---------|
| Title | 500 | **150** |
| Body | 10,000 | **1,500** |
| Answer | 50,000 | 50,000 (unchanged) |
| Tags (count) | 10 | **5** |
| Tags (each) | 50 chars | **35 chars** |

Tags must be normalized to **lowercase kebab-case**.

### 1.2 Validation Changes (3 sites)

**File: `apps/api/src/api/types/question.rs`** — REST API validation

- `CreateQuestionRequest.title`: `max = 500` → `max = 150`
- `CreateQuestionRequest.body`: `max = 10000` → `max = 1500`
- `CreateQuestionRequest.tags`: `length(max = 10)` → `length(max = 5)`
- `SearchRequest.tags`: `length(max = 10)` → `length(max = 5)`
- `validate_tags` fn: `tag.len() > 50` → `tag.len() > 35`

**File: `apps/api/src/mcp/tools/submit.rs`** — MCP tool validation

- Title max: `500` → `150`, update error message
- Body max: `10000` → `1500`, update error message
- Add tag validation (currently missing in MCP layer): max 5 tags,
  each 1-35 chars
- Update `SubmitInput` doc comments with new limits and sanitization
  guidance

**File: `apps/api/src/mcp/tools/search_questions.rs`** — MCP search

- Update `SearchQuestionsInput.tags` doc comment to mention max 5
  tags, 1-35 chars, lowercase kebab-case

### 1.3 Tag Normalization

**New file: `apps/api/src/services/tags.rs`**

```rust
pub fn normalize_tag(tag: &str) -> String {
    // 1. Trim whitespace
    // 2. Lowercase
    // 3. Replace spaces and underscores with hyphens
    // 4. Collapse consecutive hyphens
    // 5. Strip leading/trailing hyphens
    // Preserves dots (next.js) and plus signs (c++)
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    // Normalize each tag, deduplicate, filter empty
}
```

**Integration points** (service layer — single chokepoint for both REST
and MCP):

- `apps/api/src/services/question.rs` — normalize tags before
  persistence (line 31-33)
- `apps/api/src/services/search.rs` — normalize search tags for
  consistent matching (line 26)
- `apps/api/src/services/mod.rs` — add `pub mod tags;`

### 1.4 Prompt Updates

**File: `apps/api/src/mcp/server.rs`**

- Add rule 5 to `get_info` instructions: "SANITIZE CONTENT" — strip
  PII (IPs, usernames, paths, credentials, API keys), anonymize code,
  be concise, keep generic
- Update `submit` tool description to mention sanitization requirements
  and new limits
- Update tag example to lowercase kebab-case

**File: `apps/claude/instructions.md`**

- Add rule 5: "ALWAYS sanitize content before submitting" with the
  same sanitization checklist
- Update rule 4 tag examples to lowercase kebab-case

**File: `apps/claude/skills/submit-to-tokenoverflow/SKILL.md`**

- Add sanitization steps before composing submission
- Update character limits in guidance (title max 150, body max 1,500)
- Update tag guidance: max 5 tags, lowercase kebab-case

---

## Part 2: Voyage AI voyage-code-3 Integration

### 2.1 Voyage AI API Contract

```
POST /v1/embeddings
Authorization: Bearer <api_key>

Request:  {
    "input": "text",
    "model": "voyage-code-3",
    "output_dimension": 256,
    "output_dtype": "float"
}
Response: {
    "object": "list",
    "data": [{"object": "embedding", "embedding": [...], "index": 0}],
    "model": "voyage-code-3",
    "usage": {"total_tokens": N}
}
Error:    {"detail": "error message"}
```

Key differences from OpenAI: `output_dimension` (not `dimensions`),
usage has only `total_tokens` (no `prompt_tokens`), errors use
`{"detail": "..."}` (not `{"error": {...}}`).

### 2.2 Dependency Changes

**File: `apps/api/Cargo.toml`**

- Remove: `async-openai = "0.28"`
- Add to `[dependencies]`:
  `reqwest = { version = "0.12", features = ["json"] }`
- Keep `reqwest` in `[dev-dependencies]` (Cargo deduplicates)

### 2.3 Embedding Client Replacement

**File: `apps/api/src/external/embedding/client.rs`** — Rewrite

- Replace `OpenAiClient` with `VoyageClient`
- Fields: `client: reqwest::Client`, `base_url: String`,
  `model: String`, `output_dimension: u32`, `api_key: String`
- Constructor:

    ```
    VoyageClient::new(base_url: Option<&str>, model: &str, output_dimension: u32, api_key: &str)
    ```

    - Default base_url: `https://api.voyageai.com/v1` when `None`

**File: `apps/api/src/external/embedding/service.rs`** — Update impl

- Remove `async_openai` imports
- Replace `impl EmbeddingService for OpenAiClient` with
  `impl EmbeddingService for VoyageClient`
- Use `reqwest` POST with `bearer_auth`, `json()` body, parse Voyage
  response
- Handle Voyage error format: `{"detail": "..."}`
- Update doc comment: "256-dimensional vector for voyage-code-3"

**File: `apps/api/src/external/embedding/mod.rs`** — Update re-export

- `pub use client::OpenAiClient` → `pub use client::VoyageClient`

### 2.4 Config Changes

**File: `apps/api/src/config.rs`**

- Add to `EmbeddingConfig`:
    - `output_dimension: u32` (deserialized from TOML)
    - `api_key: Option<String>` with `#[serde(skip_deserializing)]`
      (secret from env)
    - `pub fn api_key(&self) -> Option<&str>` accessor
- In `Config::load()`, add:
  `config.embedding.api_key = env::var("TOKENOVERFLOW_EMBEDDING_API_KEY").ok();`

**Config TOML files:**

| File | base_url | model | output_dimension |
|------|----------|-------|-----------------|
| `local.toml` | `http://localhost:3001/v1` | `voyage-code-3` | `256` |
| `development.toml` | `https://api.voyageai.com/v1` | `voyage-code-3` | `256` |
| `production.toml` | `https://api.voyageai.com/v1` | `voyage-code-3` | `256` |
| `unit_test.toml` | (none — uses MockEmbedding) | `voyage-code-3` | `256` |

### 2.5 Server Wiring

**File: `apps/api/src/api/server.rs`**

- Import `VoyageClient` instead of `OpenAiClient`
- Update `create_app_state`: pass `output_dimension` and `api_key` to
  `VoyageClient::new()`

### 2.6 Mock Embedding Service

The mock must perfectly mirror the Voyage API.

**File: `apps/embedding-service/src/types.rs`** — Rewrite to Voyage
format

- Request: add `output_dimension: Option<u32>`,
  `output_dtype: Option<String>`, `input_type: Option<String>`,
  `truncation: Option<bool>` (all optional, `#[serde(default)]`)
- Response `Usage`: only `total_tokens` (remove `prompt_tokens`)

**File: `apps/embedding-service/src/embedder.rs`**

- `OUTPUT_DIMENSION: usize = 1536` → `OUTPUT_DIMENSION: usize = 256`
- Update doc comment to reference voyage-code-3

**File: `apps/embedding-service/src/model.rs`**

- Change padding logic to **truncation**:
  `emb.into_iter().take(OUTPUT_DIMENSION).collect()`
    - BGE-small produces 384 dims; we truncate to 256 (valid — first N
      dims carry most information)

**File: `apps/embedding-service/src/api/routes/embeddings.rs`**

- Error format: `{"error": "..."}` → `{"detail": "..."}`
- Response model: `"bge-small-en-v1.5"` → `"voyage-code-3"`
- Usage: only `total_tokens`

### 2.7 Database Schema

**File: `apps/api/migrations/20260131000000_init/up.sql`**

- Line 34: `embedding vector(1536)` → `embedding vector(256)`
- HNSW index params (`m=16`, `ef_construction=64`) stay — appropriate
  for 256-dim

After editing, run `diesel migration redo` to regenerate
`src/db/schema.rs` (the `Vector` type is dimension-agnostic, so no
manual edits needed).

### 2.8 Docker Compose

**File: `docker-compose.yml`**

- Replace `OPENAI_API_KEY: sk-mock-local-development-key` →
  `TOKENOVERFLOW_EMBEDDING_API_KEY: voy-mock-local-development-key`

---

## Test Impact

### Tests requiring limit updates

- `tests/unit/api/types/test_question.rs` — title max 501→151, body
  max 10001→1501, tags 11→6, tag length 51→36
- `tests/unit/api/types/test_search.rs` — tags 11→6
- E2E tests with tag count assertions

### New tests needed

- Tag normalization unit tests
  (`tests/unit/services/test_tags.rs`): lowercase, kebab-case, dedup,
  dots, plus signs
- MCP submit tag validation tests: too many tags, tag too long, empty
  tag
- Tag normalization integration: submit with mixed-case tags → verify
  stored as kebab-case

### Tests requiring dimension updates (1536 → 256)

- `tests/common/mock_embedding.rs` — `dimension: 256`
- `tests/unit/external/embedding/test_embedding.rs` — `VoyageClient`,
  dim 256
- `tests/e2e/external/test_embedding.rs` — `VoyageClient`, dim 256
- `tests/integration/repositories/test_search_repo.rs` —
  `vec![val; 256]`
- `tests/integration/repositories/test_question_repo.rs` —
  `vec![val; 256]`
- `tests/integration/repositories/test_answer_repo.rs` —
  `vec![val; 256]`
- `tests/unit/test_config.rs` — model name, `output_dimension`
  assertion, API key test
- `apps/embedding-service/tests/unit/api/routes/test_embeddings.rs`
  — model name, error format, usage fields
- `apps/embedding-service/tests/integration/api/routes/test_embeddings.rs`
  — model name, usage

---

## Documentation Changes

**File: `README.md`**

- Line 32: Update embedding service description from "Local embeddings
  using fastembed-rs (BGE-small-en)" to "Voyage AI-compatible local
  embeddings (fastembed-rs)"
- No other README changes needed (ports, setup flow, test commands
  unchanged)

---

## Implementation Sequence

### Task 1: Tag normalization module

Create `apps/api/src/services/tags.rs` with `normalize_tag` and
`normalize_tags`. Add unit tests. Register in `services/mod.rs`.

### Task 2: Character limit and validation updates

Update limits in `api/types/question.rs`, `mcp/tools/submit.rs` (add
tag validation). Update all related tests.

### Task 3: Integrate tag normalization

Wire into `QuestionService::create` and `SearchService::search`. Update
service tests.

### Task 4: Prompt and instruction updates

Update MCP server instructions, tool descriptions, SubmitInput doc
comments, Claude plugin `instructions.md` and `SKILL.md`.

### Task 5: Config changes for Voyage AI

Add `output_dimension` and `api_key` to `EmbeddingConfig`. Update all
TOML files. Update config tests.

### Task 6: Replace embedding client

Remove `async-openai`, add `reqwest`. Rewrite `client.rs` as
`VoyageClient`. Update `service.rs` impl, `mod.rs` re-export,
`server.rs` wiring. Update embedding unit tests.

### Task 7: Update mock embedding service

Rewrite `types.rs` for Voyage format. Change `embedder.rs` constant to
256. Change `model.rs` from padding to truncation. Update route handler
for Voyage response/error format. Update embedding service tests.

### Task 8: Database and dimension updates

Edit migration `vector(1536)` → `vector(256)`. Update
`mock_embedding.rs` dimension. Update all integration/e2e tests with
1536→256. Update docker-compose env var.

### Task 9: Documentation

Update README embedding service description.

---

## Verification

1. `cargo test -p tokenoverflow --test unit` — all unit tests pass with
   new limits, tag normalization, VoyageClient
2. `cargo test -p embedding-service --test unit` — mock service tests
   pass with Voyage format
3. `cargo test -p tokenoverflow --test integration` — testcontainers
   tests pass with 256-dim vectors
4. `docker compose up -d --build` +
   `cargo test -p tokenoverflow --test e2e` — full stack works
   end-to-end
5. Manual: run the following command and verify it returns a 256-dim
   vector:

    ```
    curl -X POST http://localhost:3001/v1/embeddings \
      -H 'Content-Type: application/json' \
      -d '{"input":"test","model":"voyage-code-3","output_dimension":256}'
    ```
