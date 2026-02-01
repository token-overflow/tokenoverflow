# Design: Core Search & Submit (Phase 2)

## Architecture Overview

Phase 2 builds on the foundation from Phase 1 to deliver the core Q&A
functionality: searching questions via semantic similarity and submitting new
questions/answers.

### Component Diagram

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PHASE 2 ARCHITECTURE                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│    ┌─────────────┐                                                          │
│    │   Client    │  curl / Postman / tests                                  │
│    │  (HTTP)     │                                                          │
│    └──────┬──────┘                                                          │
│           │                                                                 │
│           ▼                                                                 │
│    ┌─────────────────────────────────────────────────────────────────┐     │
│    │                    ntex HTTP Server                              │     │
│    │                                                                  │     │
│    │  Routes                                                          │     │
│    │  ├── GET  /health                    (existing)                  │     │
│    │  ├── POST /v1/search                 (new)                       │     │
│    │  ├── POST /v1/questions              (new)                       │     │
│    │  ├── GET  /v1/questions/{id}         (new)                       │     │
│    │  ├── POST /v1/questions/{id}/answers (new)                       │     │
│    │  ├── POST /v1/answers/{id}/upvote    (new)                       │     │
│    │  └── POST /v1/answers/{id}/downvote  (new)                       │     │
│    │                                                                  │     │
│    └──────┬───────────────────────────────────────────────────────────┘     │
│           │                                                                 │
│           ▼                                                                 │
│    ┌─────────────────────────────────────────────────────────────────┐     │
│    │                      Services Layer                              │     │
│    │                                                                  │     │
│    │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │     │
│    │  │ EmbeddingService│  │  SearchService  │  │ QuestionService │  │     │
│    │  │                 │  │                 │  │                 │  │     │
│    │  │ - embed_text()  │  │ - search()      │  │ - create()      │  │     │
│    │  │                 │  │ - by_tags()     │  │ - get_by_id()   │  │     │
│    │  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘  │     │
│    │           │                    │                    │           │     │
│    │           │           ┌────────┴────────┐           │           │     │
│    │           │           │                 │           │           │     │
│    │           │           │  AnswerService  │───────────┘           │     │
│    │           │           │                 │                       │     │
│    │           │           │ - create()      │                       │     │
│    │           │           │ - upvote()      │                       │     │
│    │           │           │ - downvote()    │                       │     │
│    │           │           └────────┬────────┘                       │     │
│    │           │                    │                                │     │
│    └───────────┼────────────────────┼────────────────────────────────┘     │
│                │                    │                                       │
│                ▼                    ▼                                       │
│    ┌─────────────────┐   ┌─────────────────────┐                           │
│    │  OpenAI API     │   │   PostgreSQL        │                           │
│    │  (or Mockoon)   │   │   + pgvector        │                           │
│    │                 │   │                     │                           │
│    │ /v1/embeddings  │   │ questions, answers, │                           │
│    │                 │   │ votes tables        │                           │
│    └─────────────────┘   └─────────────────────┘                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Hexagonal Architecture (Ports & Adapters)

The embedding service uses a trait-based design to allow swapping between real
OpenAI and mock implementations:

```text
┌─────────────────────────────────────────────────────────────────┐
│                        Application Core                          │
│                                                                 │
│   ┌─────────────────┐                                           │
│   │ EmbeddingPort   │◄── trait (interface)                      │
│   │ + embed()       │                                           │
│   └────────▲────────┘                                           │
│            │                                                    │
│   ┌────────┴────────┐                                           │
│   │ implements      │                                           │
│   │                 │                                           │
└───┼─────────────────┼───────────────────────────────────────────┘
    │                 │
    ▼                 ▼
┌─────────────┐  ┌─────────────┐
│ OpenAI      │  │ Mock        │
│ Adapter     │  │ Adapter     │
│             │  │             │
│ Real API    │  │ Static vec  │
└─────────────┘  └─────────────┘
     │                │
     ▼                ▼
┌─────────────┐  ┌─────────────┐
│ OpenAI API  │  │ Mockoon     │
│ (remote)    │  │ (Docker)    │
└─────────────┘  └─────────────┘
```

### New Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `async-openai` | 0.28 | OpenAI API client for embeddings |
| `pgvector` | 0.4 | pgvector type support for sqlx |
| `uuid` | 1 | UUID generation and parsing |
| `thiserror` | 2 | Error type definitions |

### New File Structure

```text
src/rust/tokenoverflow/
├── src/
│   ├── ports/                    # NEW: Port interfaces
│   │   ├── mod.rs
│   │   └── embedding.rs          # EmbeddingPort trait
│   ├── adapters/                 # NEW: Concrete implementations
│   │   ├── mod.rs
│   │   └── embedding/
│   │       ├── mod.rs
│   │       ├── openai.rs         # Real OpenAI adapter
│   │       └── mock.rs           # Mock adapter for testing
│   ├── services/                 # NEW: Business logic
│   │   ├── mod.rs
│   │   ├── embedding.rs          # Embedding orchestration
│   │   ├── search.rs             # Search logic
│   │   ├── question.rs           # Question CRUD
│   │   └── answer.rs             # Answer CRUD + voting
│   ├── routes/
│   │   ├── mod.rs                # Updated
│   │   ├── health.rs             # Existing
│   │   ├── search.rs             # NEW: POST /v1/search
│   │   ├── questions.rs          # NEW: Question endpoints
│   │   └── answers.rs            # NEW: Answer + voting endpoints
│   ├── models/                   # NEW: Domain models
│   │   ├── mod.rs
│   │   ├── question.rs
│   │   ├── answer.rs
│   │   └── vote.rs
│   ├── error.rs                  # NEW: Error types
│   ├── state.rs                  # NEW: App state container
│   └── ...
├── mocks/                        # NEW: Mockoon data files
│   └── openai.json
└── scripts/                      # NEW: Helper scripts
    └── seed-local.sh
```

---

## Interfaces

### 1. POST /v1/search

Search for questions using semantic similarity.

**Request:**

```json
{
    "query": "TypeError: Cannot read property 'map' of undefined",
    "tags": ["javascript", "react"],
    "limit": 5
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `query` | string | yes | 10-10,000 chars |
| `tags` | string[] | no | max 10 tags |
| `limit` | integer | no | 1-10, default 5 |

**Response (200 OK):**

```json
{
    "questions": [
        {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "TypeError: Cannot read property 'map' in React",
            "body": "I'm getting this error when rendering a list...",
            "tags": ["javascript", "react"],
            "similarity": 0.95,
            "answers": [
                {
                    "id": "660e8400-e29b-41d4-a716-446655440001",
                    "body": "Check if array exists before calling map...",
                    "upvotes": 42,
                    "downvotes": 2
                }
            ]
        }
    ]
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Invalid query length or too many tags |
| 503 | OpenAI API unavailable |

### 2. POST /v1/questions

Create a new question with an initial answer.

**Request:**

```json
{
    "title": "How to handle async errors in Python",
    "body": "When using async/await with multiple concurrent tasks...",
    "answer": "Use try/except with asyncio.gather(return_exceptions=True)...",
    "tags": ["python", "async", "error-handling"]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `title` | string | yes | 10-500 chars |
| `body` | string | yes | 10-10,000 chars |
| `answer` | string | yes | 1-50,000 chars |
| `tags` | string[] | no | max 10 tags, each 1-50 chars |

**Response (201 Created):**

```json
{
    "question_id": "550e8400-e29b-41d4-a716-446655440001",
    "answer_id": "660e8400-e29b-41d4-a716-446655440003"
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Validation failed |
| 503 | OpenAI API unavailable |

### 3. GET /v1/questions/{id}

Get a question with all its answers.

**Response (200 OK):**

```json
{
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "TypeError: Cannot read property 'map' in React",
    "body": "I'm getting this error when rendering a list...",
    "tags": ["javascript", "react"],
    "created_at": "2026-01-15T10:30:00Z",
    "answers": [
        {
            "id": "660e8400-e29b-41d4-a716-446655440001",
            "body": "Check if array exists before calling map...",
            "upvotes": 42,
            "downvotes": 2,
            "created_at": "2026-01-15T10:35:00Z"
        }
    ]
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Invalid UUID format |
| 404 | Question not found |

### 4. POST /v1/questions/{id}/answers

Add an answer to an existing question.

**Request:**

```json
{
    "body": "Another approach is to use default values with destructuring..."
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `body` | string | yes | 1-50,000 chars |

**Response (201 Created):**

```json
{
    "id": "660e8400-e29b-41d4-a716-446655440004"
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Invalid body length |
| 404 | Question not found |

### 5. POST /v1/answers/{id}/upvote

Upvote an answer. Idempotent - calling twice has no additional effect.

**Request:** Empty body

**Response (200 OK):**

```json
{
    "status": "upvoted"
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Invalid UUID format |
| 404 | Answer not found |

### 6. POST /v1/answers/{id}/downvote

Downvote an answer. Idempotent - calling twice has no additional effect.

**Request:** Empty body

**Response (200 OK):**

```json
{
    "status": "downvoted"
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| 400 | Invalid UUID format |
| 404 | Answer not found |

### 7. EmbeddingPort Trait (Internal)

```rust
#[async_trait]
pub trait EmbeddingPort: Send + Sync {
    /// Generate embedding vector for the given text
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}
```

### 8. OpenAI Embeddings API (External)

**Endpoint:** `POST https://api.openai.com/v1/embeddings`

**Request:**

```json
{
    "model": "text-embedding-3-small",
    "input": "The query text to embed"
}
```

**Response:**

```json
{
    "object": "list",
    "data": [
        {
            "object": "embedding",
            "index": 0,
            "embedding": [0.0023, -0.0091, ...]
        }
    ],
    "model": "text-embedding-3-small",
    "usage": {
        "prompt_tokens": 5,
        "total_tokens": 5
    }
}
```

---

## Logic

### 1. Search Flow

```text
POST /v1/search
      │
      ▼
┌─────────────────────┐
│ Validate request    │
│ - query length      │
│ - tags count        │
│ - limit range       │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ EmbeddingService    │
│ - Call OpenAI API   │
│ - Get 1536-dim vec  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ SearchService       │
│ - pgvector query    │  SELECT q.*, 1 - (q.embedding <=> $1) AS similarity
│ - Tag filter (AND)  │  FROM questions q
│ - Order by sim DESC │  WHERE ($2::text[] IS NULL OR q.tags @> $2)
│ - Limit results     │  ORDER BY similarity DESC
└──────────┬──────────┘  LIMIT $3
           │
           ▼
┌─────────────────────┐
│ Fetch answers       │
│ for each question   │  SELECT * FROM answers
│ ordered by upvotes  │  WHERE question_id = ANY($1)
└──────────┬──────────┘  ORDER BY upvotes DESC
           │
           ▼
┌─────────────────────┐
│ Return response     │
└─────────────────────┘
```

**Similarity Calculation:**

pgvector's `<=>` operator returns cosine distance (0 = identical, 2 = opposite).
We convert to similarity: `similarity = 1 - distance`.

A similarity threshold of 0.7 is recommended to filter out irrelevant results,
but for Phase 2 we return all results and let the client decide.

### 2. Create Question Flow

```text
POST /v1/questions
      │
      ▼
┌─────────────────────┐
│ Validate request    │
│ - title length      │
│ - body length       │
│ - answer length     │
│ - tags count        │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Generate embedding  │  Embed: title + "\n\n" + body
│ for question        │  (concatenate for richer semantic)
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Begin transaction   │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Insert question     │  INSERT INTO questions
│                     │  (title, body, tags, embedding, submitted_by)
└──────────┬──────────┘  VALUES ($1, $2, $3, $4, $5)
           │             RETURNING id
           ▼
┌─────────────────────┐
│ Insert answer       │  INSERT INTO answers
│                     │  (question_id, body, submitted_by)
└──────────┬──────────┘  VALUES ($1, $2, $3)
           │             RETURNING id
           ▼
┌─────────────────────┐
│ Commit transaction  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Return IDs          │
└─────────────────────┘
```

### 3. Voting Logic

Votes are idempotent per user. The `votes` table has a unique constraint on
`(answer_id, user_id)`.

**Upvote Flow:**

```sql
-- Insert or update vote to +1
INSERT INTO votes (answer_id, user_id, value)
VALUES ($1, $2, 1)
ON CONFLICT (answer_id, user_id)
DO UPDATE SET value = 1
WHERE votes.value != 1
RETURNING id;

-- Update answer upvotes/downvotes counters
-- (trigger or application-level recalculation)
```

**Vote Counter Update:**

```sql
UPDATE answers SET
    upvotes = (SELECT COUNT(*) FROM votes WHERE answer_id = $1 AND value = 1),
    downvotes = (SELECT COUNT(*) FROM votes WHERE answer_id = $1 AND value = -1)
WHERE id = $1;
```

### 4. Temporary User for Phase 2

Since authentication (Phase 5) is not yet implemented, Phase 2 uses a seeded
"system" user for all submissions:

```sql
-- Created by seed script
INSERT INTO users (id, cognito_sub, email)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'system',
    'system@tokenoverflow.local'
);
```

All `submitted_by` fields reference this user until Phase 5.

### 5. OpenAI Adapter Implementation

```rust
pub struct OpenAiAdapter {
    client: async_openai::Client<OpenAIConfig>,
    model: String,
}

impl OpenAiAdapter {
    pub fn new() -> Result<Self, EmbeddingError> {
        // async-openai reads OPENAI_API_KEY from environment
        let client = Client::new();
        Ok(Self {
            client,
            model: "text-embedding-3-small".to_string(),
        })
    }

    pub fn with_base_url(base_url: &str) -> Result<Self, EmbeddingError> {
        // For Mockoon: OPENAI_BASE_URL=http://localhost:3001
        let config = OpenAIConfig::new().with_api_base(base_url);
        let client = Client::with_config(config);
        Ok(Self {
            client,
            model: "text-embedding-3-small".to_string(),
        })
    }
}

#[async_trait]
impl EmbeddingPort for OpenAiAdapter {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(text)
            .build()?;

        let response = self.client.embeddings().create(request).await?;

        response.data
            .first()
            .map(|e| e.embedding.clone())
            .ok_or(EmbeddingError::EmptyResponse)
    }
}
```

### 6. Mock Adapter for Testing

```rust
pub struct MockEmbeddingAdapter {
    dimension: usize,
}

impl MockEmbeddingAdapter {
    pub fn new() -> Self {
        Self { dimension: 1536 }
    }
}

#[async_trait]
impl EmbeddingPort for MockEmbeddingAdapter {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        // Generate deterministic embedding based on text hash
        // This ensures same text = same embedding for testing
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let seed = hasher.finish();

        let mut rng = StdRng::seed_from_u64(seed);
        let embedding: Vec<f32> = (0..self.dimension)
            .map(|_| rng.gen_range(-1.0..1.0))
            .collect();

        // Normalize to unit vector for cosine similarity
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        Ok(embedding.iter().map(|x| x / magnitude).collect())
    }
}
```

---

## Edge Cases & Constraints

### Input Validation

| Field | Constraint | Error Message |
|-------|------------|---------------|
| `query` | 10-10,000 chars | "Query must be between 10 and 10,000 characters" |
| `title` | 10-500 chars | "Title must be between 10 and 500 characters" |
| `body` | 10-10,000 chars | "Body must be between 10 and 10,000 characters" |
| `answer` | 1-50,000 chars | "Answer must be between 1 and 50,000 characters" |
| `tags` | max 10, each 1-50 chars | "Maximum 10 tags allowed" / "Tag must be 1-50 characters" |
| `limit` | 1-10 | "Limit must be between 1 and 10" |

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Embedding service unavailable: {0}")]
    EmbeddingUnavailable(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<AppError> for HttpResponse {
    fn from(err: AppError) -> Self {
        match err {
            AppError::Validation(msg) => HttpResponse::BadRequest().json(&ErrorResponse { error: msg }),
            AppError::NotFound(msg) => HttpResponse::NotFound().json(&ErrorResponse { error: msg }),
            AppError::EmbeddingUnavailable(msg) => HttpResponse::ServiceUnavailable().json(&ErrorResponse { error: msg }),
            _ => HttpResponse::InternalServerError().json(&ErrorResponse { error: "Internal server error".into() }),
        }
    }
}
```

### OpenAI API Failures

| Failure | Behavior |
|---------|----------|
| Network timeout | Return 503, log error |
| Rate limit (429) | Return 503, `async-openai` auto-retries with backoff |
| Invalid API key | Return 503, log error (don't expose key details) |
| Empty response | Return 503, log "unexpected empty embedding response" |

### Database Constraints

| Constraint | Handling |
|------------|----------|
| Duplicate vote | `ON CONFLICT` upsert |
| Foreign key violation (question not found) | Return 404 |
| Foreign key violation (user not found) | Should not happen with system user |

### Concurrency

| Scenario | Handling |
|----------|----------|
| Concurrent votes on same answer | `ON CONFLICT` ensures consistency |
| Concurrent question creation | UUIDs prevent collision |
| Vote count updates | Recalculated from votes table (eventually consistent) |

---

## Test Plan

### Unit Tests

Located in `tests/unit/`. Run with `cargo test --test unit`.

| Test | Description |
|------|-------------|
| `search::validates_query_length` | Rejects queries < 10 or > 10,000 chars |
| `search::validates_tags_count` | Rejects > 10 tags |
| `search::validates_limit_range` | Rejects limit < 1 or > 10 |
| `questions::validates_title_length` | Rejects titles outside 10-500 chars |
| `questions::validates_body_length` | Rejects bodies outside 10-10,000 chars |
| `questions::validates_answer_length` | Rejects answers outside 1-50,000 chars |
| `answers::validates_body_length` | Rejects bodies outside 1-50,000 chars |
| `embedding::mock_generates_deterministic_vectors` | Same input = same output |
| `embedding::mock_generates_normalized_vectors` | Magnitude ≈ 1.0 |
| `voting::upvote_sets_value_to_one` | Vote value = 1 |
| `voting::downvote_sets_value_to_negative_one` | Vote value = -1 |

### Integration Tests

Located in `tests/integration/`. Run with `cargo test --test integration`.

Require running PostgreSQL (via Docker Compose).

| Test | Description |
|------|-------------|
| `search::returns_similar_questions` | Searches find semantically similar content |
| `search::filters_by_tags` | Tag filter correctly narrows results |
| `search::respects_limit` | Returns at most `limit` results |
| `questions::creates_question_and_answer` | Full creation flow |
| `questions::returns_404_for_nonexistent` | GET unknown ID returns 404 |
| `answers::adds_answer_to_question` | Answer links to question |
| `answers::returns_404_for_nonexistent_question` | POST to unknown question returns 404 |
| `voting::upvote_increments_count` | Upvote increases upvotes counter |
| `voting::downvote_increments_count` | Downvote increases downvotes counter |
| `voting::duplicate_vote_is_idempotent` | Voting twice doesn't double-count |
| `voting::changing_vote_updates_counts` | Upvote then downvote adjusts both |

### Manual Testing Script

`scripts/seed-local.sh` creates test data for manual exploration:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Configuration
DB_URL="${TOKENOVERFLOW_DATABASE_URL:-postgres://tokenoverflow:localdev@localhost:6432/tokenoverflow}"
API_URL="${TOKENOVERFLOW_API_URL:-http://localhost:8080}"

echo "Seeding database..."

# Create system user
psql "$DB_URL" <<'SQL'
INSERT INTO users (id, cognito_sub, email)
VALUES ('00000000-0000-0000-0000-000000000001', 'system', 'system@tokenoverflow.local')
ON CONFLICT (id) DO NOTHING;
SQL

echo "Creating sample questions via API..."

# Create sample question 1
curl -s -X POST "$API_URL/v1/questions" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "TypeError: Cannot read property map of undefined in React",
    "body": "I am getting this error when trying to render a list of items in my React component. The data comes from an API call and sometimes the array is undefined.",
    "answer": "The error occurs because you are trying to call .map() on undefined. Add a check before mapping: `{items && items.map(item => ...)}` or use optional chaining: `{items?.map(item => ...)}`",
    "tags": ["javascript", "react", "typescript"]
  }'

# Create sample question 2
curl -s -X POST "$API_URL/v1/questions" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "How to handle async errors in Python with asyncio.gather",
    "body": "When using asyncio.gather with multiple coroutines, if one fails, all results are lost. How can I handle individual failures while still getting results from successful coroutines?",
    "answer": "Use return_exceptions=True parameter: `results = await asyncio.gather(*coros, return_exceptions=True)`. Then check each result: `for r in results: if isinstance(r, Exception): handle_error(r)`",
    "tags": ["python", "async", "error-handling"]
  }'

# Create sample question 3
curl -s -X POST "$API_URL/v1/questions" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Rust borrow checker error: cannot borrow as mutable",
    "body": "Getting error E0502: cannot borrow `x` as mutable because it is also borrowed as immutable. I need to modify a struct while iterating over it.",
    "answer": "You cannot mutate while holding an immutable reference. Solutions: 1) Use indices instead of references: `for i in 0..vec.len() { vec[i].modify(); }`, 2) Collect indices first, then mutate, 3) Use interior mutability with RefCell or Mutex.",
    "tags": ["rust", "borrow-checker"]
  }'

echo ""
echo "Seed complete! Test with:"
echo "  curl -X POST $API_URL/v1/search -H 'Content-Type: application/json' -d '{\"query\": \"map undefined react\"}'"
```

### Acceptance Criteria Verification

From the product brief:

| Criterion | How to Verify |
|-----------|---------------|
| >10% searches → submission | Track in Phase 7 analytics |
| >30% searches → upvote | Track in Phase 7 analytics |

Phase 2 acceptance criteria:

| Criterion | Test |
|-----------|------|
| Can search via curl | `curl -X POST localhost:8080/v1/search ...` returns results |
| Can submit via curl | `curl -X POST localhost:8080/v1/questions ...` returns IDs |
| Can vote via curl | `curl -X POST localhost:8080/v1/answers/{id}/upvote` returns "upvoted" |
| Works offline with mock | Start with `--profile offline`, all operations succeed |

---

## Mockoon Configuration

File: `mocks/openai.json`

```json
{
  "uuid": "openai-mock",
  "name": "OpenAI Mock",
  "port": 3001,
  "routes": [
    {
      "uuid": "embeddings",
      "method": "post",
      "endpoint": "v1/embeddings",
      "responses": [
        {
          "uuid": "success",
          "body": "{\n  \"object\": \"list\",\n  \"data\": [\n    {\n      \"object\": \"embedding\",\n      \"index\": 0,\n      \"embedding\": {{repeat 1536 0.001}}\n    }\n  ],\n  \"model\": \"text-embedding-3-small\",\n  \"usage\": {\n    \"prompt_tokens\": 10,\n    \"total_tokens\": 10\n  }\n}",
          "latency": 50,
          "statusCode": 200,
          "headers": [
            { "key": "Content-Type", "value": "application/json" }
          ]
        }
      ]
    }
  ]
}
```

**Usage:**

```bash
# Start with offline profile
docker compose --profile offline up -d

# Set environment variable
export OPENAI_BASE_URL=http://localhost:3001

# Run API
cargo run
```

---

## Configuration Changes

Add to `config.rs`:

```rust
pub struct Config {
    // ... existing fields ...

    /// OpenAI API key (required for production)
    pub openai_api_key: Option<String>,

    /// OpenAI base URL (for mocking)
    pub openai_base_url: Option<String>,

    /// Use mock embedding adapter (for testing)
    pub use_mock_embedding: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        // ... existing code ...

        Ok(Self {
            // ... existing fields ...
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            openai_base_url: env::var("OPENAI_BASE_URL").ok(),
            use_mock_embedding: env::var("TOKENOVERFLOW_USE_MOCK_EMBEDDING")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        })
    }
}
```

---

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TOKENOVERFLOW_DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `OPENAI_API_KEY` | Prod only | - | OpenAI API key |
| `OPENAI_BASE_URL` | No | api.openai.com | Override for mock server |
| `TOKENOVERFLOW_USE_MOCK_EMBEDDING` | No | false | Use mock adapter |

---

## Result

After Phase 2 completion:

```bash
# Search for questions
curl -X POST http://localhost:8080/v1/search \
  -H "Content-Type: application/json" \
  -d '{"query": "react map undefined error"}'

# Create a question
curl -X POST http://localhost:8080/v1/questions \
  -H "Content-Type: application/json" \
  -d '{
    "title": "My question title here",
    "body": "Detailed description of the problem...",
    "answer": "The solution that worked...",
    "tags": ["javascript"]
  }'

# Upvote an answer
curl -X POST http://localhost:8080/v1/answers/{answer_id}/upvote
```
