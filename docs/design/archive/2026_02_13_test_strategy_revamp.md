# Design: test-strategy-revamp

## Architecture Overview

This design revamps the entire testing strategy for TokenOverflow by introducing
a three-tier test split (unit, integration, e2e), applying the "Functional Core,
Imperative Shell" pattern to separate pure business logic from database I/O, and
replacing the fragile per-process PostgreSQL setup with testcontainers for
integration tests.

### The Problem

The current test infrastructure has a single PostgreSQL process spawned via
`tests/common/test_db.rs`. This causes:

- **Slow tests**: `initdb` + postgres startup on every test run
- **100% CPU usage**: postgres server under high parallelism
- **Fragile cleanup**: `pkill` commands to kill orphaned postgres processes
- **macOS shared memory exhaustion**: SHMMNI=32 limit hit with parallel tests
- **Wrong abstraction boundary**: 84 of 142 unit tests spin up a real database
  even though they could test pure logic with mocks

### Three-Tier Test Architecture

```text
                     TEST PYRAMID

     +-------------------------------------------+
     |              e2e (black-box)               |
     |  docker compose up -> HTTP requests        |
     |  Focus: user stories, full system          |
     |  ~30 tests, run last                       |
     +-------------------------------------------+
     |          integration (in-process)          |
     |  testcontainers postgres + axum::oneshot   |
     |  Focus: DB queries, transactions, wiring   |
     |  ~40 tests, run second                     |
     +-------------------------------------------+
     |              unit (pure)                   |
     |  zero external dependencies                |
     |  Focus: business logic, validation,        |
     |  error mapping, serialization              |
     |  ~100 tests, run first (<1 sec)            |
     +-------------------------------------------+
```

### Functional Core, Imperative Shell

The key architectural change is splitting services into two parts:

```text
 BEFORE (current):
 ┌──────────────────────────────────────────────┐
 │            QuestionService::create()          │
 │                                               │
 │  1. Format embed text    ─┐                   │
 │  2. Call embedding.embed() │  Pure logic      │
 │  3. Format tags           ─┘                   │
 │  4. Get DB connection     ─┐                   │
 │  5. INSERT question        │  DB I/O          │
 │  6. INSERT answer          │                   │
 │  7. COMMIT transaction    ─┘                   │
 └──────────────────────────────────────────────┘

 AFTER (new):
 ┌──────────────────────────────────────────────┐
 │  FUNCTIONAL CORE (pure, testable with mocks) │
 │                                               │
 │  QuestionService::create()                    │
 │    1. Format embed text                       │
 │    2. Call embedding.embed()                  │
 │    3. Format tags                             │
 │    4. Call repo.create(...)                   │
 │       ^-- trait object, no DB knowledge       │
 └──────────────────────────────────────────────┘

 ┌──────────────────────────────────────────────┐
 │  IMPERATIVE SHELL (DB I/O, tested via integ) │
 │                                               │
 │  PgQuestionRepository::create()               │
 │    1. Get DB connection                       │
 │    2. INSERT question                         │
 │    3. INSERT answer                           │
 │    4. COMMIT transaction                      │
 └──────────────────────────────────────────────┘
```

The same `EmbeddingService` trait pattern already used in
`src/external/embedding/service.rs` is extended to database access: define a
repository trait, implement it with Diesel for production, and provide an
in-memory mock for unit tests.

### Testcontainers for Integration Tests

Instead of manually spawning postgres with `initdb`/`postgres` commands and
managing process lifecycle, integration tests use testcontainers-rs (v0.27) with
testcontainers-modules (v0.14) to spin up a `pgvector/pgvector:pg17` Docker
container. The container starts ONCE per test run, migrations run ONCE, then
each test gets a fast isolated database created from a template.

```text
 ┌──────────────────────────────────────────────────┐
 │              Integration Test Run                 │
 │                                                   │
 │  1. Start pgvector container (testcontainers)     │
 │     ~ 2-3 seconds, happens once                   │
 │                                                   │
 │  2. Create "template_tokenoverflow" database      │
 │     - Run migrations                              │
 │     - Insert system user                          │
 │     - Create pgvector extension                   │
 │     ~ 200ms, happens once                         │
 │                                                   │
 │  3. For each test:                                │
 │     CREATE DATABASE test_N                        │
 │       TEMPLATE template_tokenoverflow;            │
 │     ~ 30-50ms per test (filesystem copy)          │
 │                                                   │
 │  4. Container auto-removed when tests complete    │
 └──────────────────────────────────────────────────┘
```

### Component Diagram (After)

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                        PRODUCTION CODE                                  │
│                                                                         │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐      │
│  │  Route Handlers   │  │   MCP Tools      │  │  Health Check    │      │
│  │  questions.rs     │  │   submit.rs      │  │  health.rs       │      │
│  │  answers.rs       │  │   search_q.rs    │  │                  │      │
│  │  search.rs        │  │   upvote.rs      │  │                  │      │
│  └───────┬───────────┘  └───────┬──────────┘  └──────────────────┘      │
│          │                      │                                        │
│          ▼                      ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐       │
│  │                     Services (Functional Core)                │       │
│  │                                                               │       │
│  │  QuestionService   AnswerService   SearchService              │       │
│  │  - format embed    - validate      - embed query              │       │
│  │  - format tags     - delegate to   - delegate to              │       │
│  │  - delegate to       repo            repo                     │       │
│  │    repo                                                       │       │
│  └────────┬────────────────┬───────────────┬─────────────────────┘       │
│           │                │               │                             │
│    ┌──────▼──────┐  ┌──────▼──────┐ ┌──────▼──────┐                     │
│    │ Question    │  │ Answer      │ │ Search      │                     │
│    │ Repository  │  │ Repository  │ │ Repository  │  <-- TRAITS         │
│    │ (trait)     │  │ (trait)     │ │ (trait)      │                     │
│    └──────┬──────┘  └──────┬──────┘ └──────┬──────┘                     │
│           │                │               │                             │
│    ┌──────▼──────┐  ┌──────▼──────┐ ┌──────▼──────┐                     │
│    │ PgQuestion  │  │ PgAnswer    │ │ PgSearch    │                     │
│    │ Repository  │  │ Repository  │ │ Repository  │  <-- IMPLS          │
│    │ (Diesel)    │  │ (Diesel)    │ │ (Diesel)    │                     │
│    └──────┬──────┘  └──────┬──────┘ └──────┬──────┘                     │
│           └────────────────┼───────────────┘                             │
│                            ▼                                             │
│                     ┌─────────────┐                                      │
│                     │ PostgreSQL  │                                      │
│                     │ + pgvector  │                                      │
│                     └─────────────┘                                      │
│                                                                         │
│    ┌──────────────┐                                                      │
│    │ Embedding    │                                                      │
│    │ Service      │  <-- TRAIT (already exists)                          │
│    │ (trait)      │                                                      │
│    └──────┬───────┘                                                      │
│    ┌──────▼───────┐                                                      │
│    │ OpenAiClient │                                                      │
│    └──────────────┘                                                      │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                        TEST CODE                                        │
│                                                                         │
│  UNIT TESTS (zero deps)           INTEGRATION TESTS (testcontainers)   │
│  ┌──────────────┐                 ┌──────────────┐                      │
│  │ MockQuestion │                 │ PgQuestion   │                      │
│  │ Repository   │                 │ Repository   │                      │
│  │ (in-memory)  │                 │ + real DB    │                      │
│  └──────────────┘                 └──────────────┘                      │
│  ┌──────────────┐                 ┌──────────────┐                      │
│  │ MockAnswer   │                 │ PgAnswer     │                      │
│  │ Repository   │                 │ Repository   │                      │
│  └──────────────┘                 └──────────────┘                      │
│  ┌──────────────┐                 ┌──────────────┐                      │
│  │ MockSearch   │                 │ PgSearch     │                      │
│  │ Repository   │                 │ Repository   │                      │
│  └──────────────┘                 └──────────────┘                      │
│  ┌──────────────┐                                                       │
│  │ MockEmbedding│ (already exists)                                      │
│  └──────────────┘                                                       │
│                                                                         │
│  E2E TESTS (docker compose)                                             │
│  ┌──────────────┐                                                       │
│  │ TestClient   │ --> HTTP --> docker compose stack                      │
│  │ (reqwest)    │    (current integration tests become e2e)             │
│  └──────────────┘                                                       │
└─────────────────────────────────────────────────────────────────────────┘
```

### New Dependencies

| Crate | Version | Feature | Purpose |
|-------|---------|---------|---------|
| `testcontainers` | 0.27 | - | Container lifecycle management |
| `testcontainers-modules` | 0.14 | `postgres` | Postgres container module |

Both are `[dev-dependencies]` only. No changes to production dependencies.

### Dependencies to Remove

| Crate | Reason |
|-------|--------|
| `tempfile` | No longer need temp dirs for postgres data |
| `portpicker` | testcontainers handles port mapping |

---

## Interfaces

### 1. Repository Traits (New Internal Interfaces)

These traits define the contract between the functional core (services) and the
imperative shell (database access). They follow the same pattern as the existing
`EmbeddingService` trait.

**QuestionRepository** (`src/services/repository/question.rs`):

```rust
use async_trait::async_trait;
use uuid::Uuid;

use crate::api::types::{CreateQuestionResponse, QuestionWithAnswers};
use crate::error::AppError;

#[async_trait]
pub trait QuestionRepository: Send + Sync {
    /// Insert a question and its initial answer in a single transaction.
    ///
    /// Accepts a raw Vec<f32> embedding (not pgvector::Vector) to keep
    /// Diesel types out of the trait interface.
    async fn create(
        &self,
        title: &str,
        body: &str,
        answer_body: &str,
        tags: Vec<Option<String>>,
        embedding: Vec<f32>,
        submitted_by: Uuid,
    ) -> Result<CreateQuestionResponse, AppError>;

    /// Fetch a question by ID with all its answers.
    async fn get_by_id(&self, id: Uuid) -> Result<QuestionWithAnswers, AppError>;

    /// Check if a question exists.
    async fn exists(&self, id: Uuid) -> Result<bool, AppError>;
}
```

**AnswerRepository** (`src/services/repository/answer.rs`):

```rust
#[async_trait]
pub trait AnswerRepository: Send + Sync {
    /// Insert an answer for a question.
    async fn create(
        &self,
        question_id: Uuid,
        body: &str,
        submitted_by: Uuid,
    ) -> Result<Uuid, AppError>;

    /// Record an upvote (idempotent per user).
    async fn upvote(&self, answer_id: Uuid, user_id: Uuid) -> Result<(), AppError>;

    /// Record a downvote (idempotent per user).
    async fn downvote(&self, answer_id: Uuid, user_id: Uuid) -> Result<(), AppError>;

    /// Check if an answer exists.
    async fn exists(&self, id: Uuid) -> Result<bool, AppError>;
}
```

**SearchRepository** (`src/services/repository/search.rs`):

```rust
use crate::api::types::SearchResultQuestion;

#[async_trait]
pub trait SearchRepository: Send + Sync {
    /// Search for questions by embedding similarity, optionally filtered by tags.
    async fn search(
        &self,
        embedding: Vec<f32>,
        tags: Option<&[String]>,
        limit: i32,
    ) -> Result<Vec<SearchResultQuestion>, AppError>;
}
```

Key design decisions for the trait interfaces:

- Repositories accept `Vec<f32>` for embeddings, not `pgvector::Vector`. This
  keeps Diesel-specific types out of the trait interface.
- `SYSTEM_USER_ID` stays in service code (business logic). The `submitted_by`
  UUID is passed as a parameter to the repository.
- Return types reuse existing API types (`CreateQuestionResponse`,
  `QuestionWithAnswers`, `SearchResultQuestion`) since they already match what
  callers need.

### 2. Updated AppState

```rust
pub struct AppState {
    pub pool: DbPool,  // retained for health check endpoint
    pub embedding: Arc<dyn EmbeddingService>,
    pub questions: Arc<dyn QuestionRepository>,
    pub answers: Arc<dyn AnswerRepository>,
    pub search: Arc<dyn SearchRepository>,
}
```

The `pool` field stays because the health check endpoint needs it to verify
database connectivity. All business operations go through repository traits.

### 3. Cargo.toml Test Binaries

```toml
[[test]]
name = "unit"
path = "tests/unit/mod.rs"

[[test]]
name = "integration"
path = "tests/integration/mod.rs"

[[test]]
name = "e2e"
path = "tests/e2e/mod.rs"
```

---

## Logic

### 1. Service Refactoring (Functional Core)

Each service becomes a thin orchestrator of pure logic plus trait method calls.
Here is what each service method looks like after refactoring:

**QuestionService::create** (before vs after):

```rust
// BEFORE: Directly uses DbPool and Diesel queries
pub async fn create(
    pool: &DbPool,
    embedding: &dyn EmbeddingService,
    title: &str, body: &str, answer: &str, tags: Option<&[String]>,
) -> Result<CreateQuestionResponse, AppError> {
    let embed_text = format!("{}\n\n{}", title, body);
    let embedding_vec = embedding.embed(&embed_text).await...;
    let embedding_vector = Vector::from(embedding_vec);
    let tags_vec = ...;
    let mut conn = pool.get().await...;
    conn.transaction(|conn| { /* INSERT question, INSERT answer */ }).await?;
    Ok(CreateQuestionResponse { ... })
}

// AFTER: Uses repository trait, no Diesel/pgvector types
pub async fn create(
    repo: &dyn QuestionRepository,
    embedding: &dyn EmbeddingService,
    title: &str, body: &str, answer: &str, tags: Option<&[String]>,
) -> Result<CreateQuestionResponse, AppError> {
    // Pure logic: format embedding input
    let embed_text = format!("{}\n\n{}", title, body);

    // Side effect via trait: generate embedding
    let embedding_vec = embedding
        .embed(&embed_text)
        .await
        .map_err(|e| AppError::EmbeddingUnavailable(e.to_string()))?;

    // Pure logic: format tags
    let tags_vec: Vec<Option<String>> = tags
        .map(|t| t.iter().map(|s| Some(s.clone())).collect())
        .unwrap_or_default();

    let user_id: Uuid = *SYSTEM_USER_ID;

    // Side effect via trait: persist to database
    repo.create(title, body, answer, tags_vec, embedding_vec, user_id).await
}
```

**AnswerService::upvote** (after):

```rust
pub async fn upvote(
    repo: &dyn AnswerRepository,
    answer_id: Uuid,
) -> Result<(), AppError> {
    let user_id: Uuid = *SYSTEM_USER_ID;
    repo.upvote(answer_id, user_id).await
}
```

**SearchService::search** (after):

```rust
pub async fn search(
    repo: &dyn SearchRepository,
    embedding: &dyn EmbeddingService,
    query: &str, tags: Option<&[String]>, limit: i32,
) -> Result<Vec<SearchResultQuestion>, AppError> {
    let query_embedding = embedding
        .embed(query)
        .await
        .map_err(|e| AppError::EmbeddingUnavailable(e.to_string()))?;

    repo.search(query_embedding, tags, limit).await
}
```

### 2. PostgreSQL Repository Implementations (Imperative Shell)

The Diesel query code currently inside each service method moves into `Pg*`
repository structs. For example, `PgQuestionRepository::create` contains
the transaction with INSERT question + INSERT answer, exactly as it exists today
in `QuestionService::create`, but with `pgvector::Vector::from()` called inside
the implementation rather than in the service.

Each Pg repository holds a `DbPool`:

```rust
pub struct PgQuestionRepository {
    pool: DbPool,
}

impl PgQuestionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}
```

The `SearchRow` struct (currently in `src/services/search_row.rs`) moves into
`PgSearchRepository`'s module since it is a Diesel-specific type only used by
the Pg implementation.

### 3. Testcontainers Integration Test Database

The integration test database helper replaces `tests/common/test_db.rs` with a
testcontainers-based approach:

```rust
use std::sync::OnceLock;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use testcontainers::core::ImageExt;
use std::sync::atomic::{AtomicU32, Ordering};

use tokenoverflow::db::DbPool;

/// Counter for unique database names per test
static DB_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Global container + connection URL, started once per test run
static TEST_INFRA: OnceLock<TestInfra> = OnceLock::new();

struct TestInfra {
    /// Keep container alive for the duration of the test run
    _container: ContainerAsync<Postgres>,
    /// Connection URL to the postgres (admin) database
    admin_url: String,
    /// Base connection URL (without database name) for creating per-test DBs
    base_url: String,
}

// SAFETY: TestInfra is written once via OnceLock, then read-only
unsafe impl Sync for TestInfra {}
unsafe impl Send for TestInfra {}

async fn init_infra() -> &'static TestInfra {
    // Double-checked init pattern using OnceLock
    if let Some(infra) = TEST_INFRA.get() {
        return infra;
    }

    let container = Postgres::default()
        .with_name("pgvector/pgvector")
        .with_tag("pg17")
        .with_db_name("postgres")
        .with_user("postgres")
        .with_password("postgres")
        .start()
        .await
        .expect("Failed to start postgres container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();

    let admin_url = format!(
        "postgresql://postgres:postgres@{}:{}/postgres",
        host, port
    );

    let base_url = format!(
        "postgresql://postgres:postgres@{}:{}",
        host, port
    );

    // Create the template database
    create_template_database(&admin_url).await;

    TEST_INFRA.get_or_init(|| TestInfra {
        _container: container,
        admin_url,
        base_url,
    })
}

async fn create_template_database(admin_url: &str) {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(admin_url);
    let pool = Pool::builder().max_size(1).build(config).await.unwrap();
    let mut conn = pool.get().await.unwrap();

    // Create template database
    diesel::sql_query("CREATE DATABASE template_tokenoverflow")
        .execute(&mut conn)
        .await
        .unwrap();

    // Connect to the template DB, enable pgvector, run migrations
    let template_url = format!(
        "{}/template_tokenoverflow",
        admin_url.rsplit_once('/').unwrap().0
    );
    let template_config =
        AsyncDieselConnectionManager::<AsyncPgConnection>::new(&template_url);
    let template_pool = Pool::builder().max_size(1).build(template_config)
        .await.unwrap();
    let mut template_conn = template_pool.get().await.unwrap();

    diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&mut template_conn).await.unwrap();

    run_migrations(&mut template_conn).await;

    // Disconnect before marking as template
    drop(template_conn);
    drop(template_pool);

    // Mark as template so CREATE DATABASE ... TEMPLATE works
    diesel::sql_query(
        "UPDATE pg_database SET datistemplate = true \
         WHERE datname = 'template_tokenoverflow'"
    )
    .execute(&mut conn)
    .await
    .unwrap();
}

pub struct IntegrationTestDb {
    pool: DbPool,
    db_name: String,
}

impl IntegrationTestDb {
    pub async fn new() -> Self {
        let infra = init_infra().await;
        let db_id = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_name = format!("test_{}", db_id);

        // Create a fresh database from the template (~30-50ms)
        let config =
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(&infra.admin_url);
        let admin_pool = Pool::builder().max_size(1).build(config).await.unwrap();
        let mut admin_conn = admin_pool.get().await.unwrap();

        diesel::sql_query(format!(
            "CREATE DATABASE {} TEMPLATE template_tokenoverflow",
            db_name
        ))
        .execute(&mut admin_conn)
        .await
        .unwrap();

        // Connect to the per-test database
        let test_url = format!("{}/{}", infra.base_url, db_name);
        let test_config =
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(&test_url);
        let pool = Pool::builder()
            .max_size(2)
            .build(test_config)
            .await
            .unwrap();

        IntegrationTestDb { pool, db_name }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}
```

### 4. Template Database Pattern Performance

| Operation | Time | Frequency |
|-----------|------|-----------|
| Start pgvector container | ~2-3 seconds | Once per test run |
| Create template + run migrations | ~200ms | Once per test run |
| Create per-test DB from template | ~30-50ms | Per test |
| Total for 40 integration tests | ~4-5 seconds | Per test run |

Compare with current approach:

| Operation | Time | Frequency |
|-----------|------|-----------|
| pkill + cleanup | ~500ms | Once per test run |
| initdb | ~1 second | Once per test run |
| postgres startup + wait | ~5 seconds | Once per test run |
| Create test_db + extensions | ~200ms | Once per test run |
| Per-test connection (shared DB) | ~10ms | Per test |
| Total for 84 "unit" tests hitting DB | ~7 seconds | Per test run |

The testcontainers approach is comparable in startup time but eliminates:
- Process management (`pkill`, orphaned processes)
- Shared memory exhaustion (SHMMNI)
- Local postgres installation requirement
- `tempfile`/`portpicker` dependencies

### 5. Mock Repository for Unit Tests

The in-memory mock repositories let unit tests run without any external
dependencies. They share state via `Arc<Mutex<Vec<...>>>` so that
submit-then-search flows work in MCP tool tests.

```rust
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// In-memory storage for mock repositories
#[derive(Clone)]
pub struct MockStore {
    pub questions: Arc<Mutex<Vec<StoredQuestion>>>,
    pub answers: Arc<Mutex<Vec<StoredAnswer>>>,
    pub votes: Arc<Mutex<Vec<StoredVote>>>,
}

impl MockStore {
    pub fn new() -> Self {
        Self {
            questions: Arc::new(Mutex::new(Vec::new())),
            answers: Arc::new(Mutex::new(Vec::new())),
            votes: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

pub struct StoredQuestion {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub embedding: Vec<f32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct StoredAnswer {
    pub id: Uuid,
    pub question_id: Uuid,
    pub body: String,
    pub upvotes: i32,
    pub downvotes: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct StoredVote {
    pub answer_id: Uuid,
    pub user_id: Uuid,
    pub value: i32,
}
```

The `MockQuestionRepository` creates UUIDs and stores in the vector.
`MockSearchRepository` reads from the same shared store and computes a fake
similarity score. `MockAnswerRepository` validates that the question exists
in the shared store before inserting.

### 6. E2E Test Relocation

Current integration tests (located in `tests/integration/`) become e2e tests
(located in `tests/e2e/`). They remain black-box tests against a running Docker
Compose stack via HTTP. No logic changes -- just a directory rename and test
binary rename.

### 7. New Integration Tests

The new integration tests (in `tests/integration/`) test the Pg repository
implementations with a real database via testcontainers. They use
`IntegrationTestDb::new()` instead of `TestClient::from_config()`.

These tests cover:
- `PgQuestionRepository`: create, get_by_id, exists with real postgres
- `PgAnswerRepository`: create, upvote/downvote transactions, exists
- `PgSearchRepository`: vector similarity search, tag filtering, limit
- Route handlers wired with real DB (axum oneshot with real repos)
- MCP tools wired with real DB

### 8. Coverage Hook Update

The coverage script (`src/shell/tokenoverflow/git_hooks/cargo_coverage.sh`)
updates to include all three test tiers:

```bash
RUST_TEST_THREADS=8 cargo +nightly llvm-cov \
    --manifest-path "${MANIFEST_PATH}" \
    --lib --test unit --test integration --test e2e \
    --fail-under-lines "${REQUIRED_COVERAGE}"
```

---

## Edge Cases & Constraints

### Template Database Limitations

- PostgreSQL's `CREATE DATABASE ... TEMPLATE` requires no other sessions
  connected to the template database while copying. The design handles this by
  creating the template once, disconnecting from it, and never reconnecting.
  All per-test databases are created via the admin connection to `postgres`.

- The template database must be marked with `datistemplate = true` in
  `pg_database` to allow concurrent copies.

### Testcontainers Docker Requirement

- Integration tests require Docker to be running. If Docker is not available,
  integration tests will fail with a clear error message from testcontainers.
  Unit tests and e2e tests are unaffected.

- The `pgvector/pgvector:pg17` image must be available. On first run,
  testcontainers will pull it (~400MB). Subsequent runs use the cached image.

### Mock Repository Fidelity

The mock repositories intentionally do NOT replicate all PostgreSQL behavior.
They provide:
- Basic CRUD operations with UUID generation
- Simple existence checks
- Vote idempotency (upsert behavior)
- Shared state between question/answer/search mocks

They do NOT replicate:
- Foreign key constraints (the mock validates question existence manually)
- Transaction semantics
- Vector similarity scoring (returns a fixed fake similarity)
- SQL-level tag array containment (`@>` operator)

This is by design. The mock tests verify that the service layer correctly
orchestrates calls. The PostgreSQL-specific behavior (constraints, vector
operations, transactions) is verified by integration tests.

### Failing Mock Repositories

For error-path testing, `Failing*Repository` structs always return
`AppError::Internal`, replacing the current `create_broken_pool()` pattern:

```rust
pub struct FailingQuestionRepository;

#[async_trait]
impl QuestionRepository for FailingQuestionRepository {
    async fn create(...) -> Result<CreateQuestionResponse, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }
    // ... all methods return Err
}
```

### Coverage Exclusions

The goal is 100% coverage with ZERO exclusions. Here is how each currently
excluded item is handled:

| Current Exclusion | File | New Strategy |
|-------------------|------|-------------|
| `search_row` module | `services/mod.rs` | Moves into `PgSearchRepository`, tested by integration tests. Remove exclusion. |
| `run()` | `api/server.rs` | Production bootstrap. Keep exclusion (cannot test without running server). |
| `async_run()` | `api/server.rs` | Production bootstrap. Keep exclusion. |
| `create_app_state()` | `api/server.rs` | Production wiring. Keep exclusion. |
| `db::init()` | `db/pool.rs` | Production pool init. Keep exclusion. |
| `schema` module | `db/mod.rs` | Diesel-generated. Keep exclusion. |
| `configure()` | `api/routes/configure.rs` | Route configuration. Keep exclusion. |
| `EmbeddingService for OpenAiClient` | `external/embedding/service.rs` | Needs live API. Keep exclusion. |
| `main()` | `main.rs` | Entry point. Keep exclusion. |

Pg repository implementations do NOT get `#[coverage(off)]` because they are
tested by integration tests which contribute to the coverage report. This is an
improvement over the previous plan which proposed excluding them.

### Test Ordering in Pre-commit

The pre-commit hook runs all three tiers:
1. Unit tests first (fastest, catch logic errors early)
2. Integration tests second (need Docker, catch DB issues)
3. E2e tests last (need full Docker Compose stack, catch system issues)

### Environment Variable Naming

Per CLAUDE.md, all custom environment variables must be prefixed with
`TOKENOVERFLOW_`. The testcontainers setup uses no custom environment variables
-- the container configuration is fully programmatic.

---

## Test Plan

### Unit Tests (`cargo test --test unit`)

Zero external dependencies. Run in under 1 second.

| Area | What is Tested | Mock Used |
|------|---------------|-----------|
| `QuestionService::create` | Embedding text formatting, tag formatting, `SYSTEM_USER_ID` pass-through, error mapping from embedding failure | `MockQuestionRepository` + `MockEmbedding` |
| `QuestionService::get_by_id` | Direct delegation to repo, error propagation | `MockQuestionRepository` |
| `QuestionService::exists` | Direct delegation to repo | `MockQuestionRepository` |
| `AnswerService::create` | `SYSTEM_USER_ID` pass-through, error propagation | `MockAnswerRepository` |
| `AnswerService::upvote/downvote` | `SYSTEM_USER_ID` pass-through, delegation | `MockAnswerRepository` |
| `AnswerService::exists` | Direct delegation | `MockAnswerRepository` |
| `SearchService::search` | Embedding call, error mapping, delegation | `MockSearchRepository` + `MockEmbedding` |
| Route handlers | Validation (title/body length), UUID parsing, status codes, response shapes, error responses | `MockAppState` with mock repos |
| MCP server | `get_info()`, `list_tools()`, tool descriptions, `call_tool()` dispatch, unknown tool error | `MockAppState` with mock repos |
| MCP tools | Input validation (length checks), `submit` flow, `search_questions` flow, `upvote_answer` flow, response hints | `MockAppState` with mock repos |
| Error types | `AppError` variants to HTTP status mapping, `diesel_fk_not_found`, `From<ValidationErrors>`, `From<bb8::RunError>` | None (pure logic) |
| Config | Config loading, env overrides | None |
| Logging | Tracing setup | None |
| API types | Serialization/deserialization, validation | None |

### Integration Tests (`cargo test --test integration`)

Require Docker. Use testcontainers with pgvector.

| Area | What is Tested | Setup |
|------|---------------|-------|
| `PgQuestionRepository::create` | Transaction commits, both question and answer inserted, embedding stored | `IntegrationTestDb` |
| `PgQuestionRepository::get_by_id` | Correct join with answers, ordering | `IntegrationTestDb` |
| `PgQuestionRepository::exists` | COUNT query correctness | `IntegrationTestDb` |
| `PgAnswerRepository::create` | FK constraint (nonexistent question returns error) | `IntegrationTestDb` |
| `PgAnswerRepository::upvote/downvote` | Vote upsert, count recalculation, idempotency, vote switching | `IntegrationTestDb` |
| `PgSearchRepository::search` | Vector similarity via pgvector `<=>`, tag filtering via `@>`, limit, answer grouping | `IntegrationTestDb` |
| Health check with real DB | `database: "connected"` | `IntegrationTestDb` |
| Full route handler with real DB | axum oneshot with real `PgXxxRepository` wired | `IntegrationTestDb` |

### E2E Tests (`cargo test --test e2e`)

Require `docker compose up -d --build api`. Black-box HTTP tests.

| Area | What is Tested |
|------|---------------|
| Question creation flow | POST /v1/questions returns 201 with IDs |
| Question retrieval | GET /v1/questions/{id} returns question with answers |
| Answer submission | POST /v1/questions/{id}/answers returns 201 |
| Voting | POST /v1/answers/{id}/upvote returns 200 |
| Search | POST /v1/search returns results with similarity |
| Health check | GET /health returns connected |
| MCP over HTTP | Streamable HTTP MCP protocol end-to-end |
| User story flows | Multi-step scenarios (create, search, upvote) |

### Coverage Accounting

| Code Area | Covered By | Exclusion Needed? |
|-----------|-----------|-------------------|
| Service logic (functional core) | Unit tests | No |
| Route handler logic | Unit tests | No |
| MCP tool logic | Unit tests | No |
| Error types | Unit tests | No |
| API types (serde, validation) | Unit tests | No |
| Config loading | Unit tests | No |
| Pg repository implementations | Integration tests | No |
| `SearchRow` (QueryableByName) | Integration tests | No |
| `EmbeddingService for OpenAiClient` | Needs live API | Yes (existing) |
| `main.rs` | Entry point | Yes (existing) |
| `server.rs` bootstrap fns | Needs running server | Yes (existing) |
| `db::init()` | Production pool | Yes (existing) |
| `db::schema` | Diesel-generated | Yes (existing) |
| `routes::configure()` | Route wiring | Yes (existing) |

Result: Same existing exclusions, no new exclusions needed. The `search_row`
module exclusion is REMOVED because the code moves into the Pg repository
which is now covered by integration tests.

---

## Documentation Changes

No new documentation files will be created as part of this change. The existing
`docs/plans/repository-pattern-unit-tests.md` is superseded by this design
document. It should be moved to `docs/plans/archive/` or deleted.

---

## Development Environment Changes

### Docker Requirement

Integration tests now require Docker to be running (for testcontainers).
Docker is already required for e2e tests and local development, so this is not
a new requirement.

### Removed Requirements

- Local PostgreSQL installation (`initdb`, `postgres` binaries) is no longer
  needed for running tests. The `Brewfile` may have had postgres-related
  formulas; if so, they can be removed since testcontainers pulls its own
  Docker image.

### Coverage Script Update

`src/shell/tokenoverflow/git_hooks/cargo_coverage.sh` adds `--test e2e` to
the cargo llvm-cov invocation.

---

## Tasks

### Task 1: Create Repository Traits

**Scope**: Define the three repository traits in new files under
`src/services/repository/`.

**Files to create**:
- `src/rust/tokenoverflow/src/services/repository/mod.rs`
- `src/rust/tokenoverflow/src/services/repository/question.rs`
- `src/rust/tokenoverflow/src/services/repository/answer.rs`
- `src/rust/tokenoverflow/src/services/repository/search.rs`

**Requirements**:
- Each trait uses `#[async_trait]` and requires `Send + Sync`
- Method signatures match the interfaces defined in this document
- Repositories accept `Vec<f32>` for embeddings, not pgvector types
- `submitted_by: Uuid` is a parameter, not hardcoded
- Return types use existing API types from `crate::api::types`
- All trait methods return `Result<T, AppError>`
- `mod.rs` re-exports all traits

**Success criteria**: Traits compile. No production code changes yet.

---

### Task 2: Create PostgreSQL Repository Implementations

**Scope**: Extract Diesel query code from services into Pg repository structs.

**Files to create**:
- `src/rust/tokenoverflow/src/services/repository/pg_question.rs`
- `src/rust/tokenoverflow/src/services/repository/pg_answer.rs`
- `src/rust/tokenoverflow/src/services/repository/pg_search.rs`

**Requirements**:
- Each struct holds a `DbPool` field
- `PgQuestionRepository::create` contains the transaction + INSERT code
  currently in `QuestionService::create` (lines 46-86 of question.rs)
- `PgQuestionRepository::get_by_id` contains the two-query fetch code
  currently in `QuestionService::get_by_id` (lines 95-125)
- `PgQuestionRepository::exists` contains the COUNT query (lines 128-143)
- `PgAnswerRepository::create` contains the INSERT code from
  `AnswerService::create` (lines 20-43)
- `PgAnswerRepository::upvote/downvote/vote` contains the transaction code
  from `AnswerService::vote` (lines 60-118)
- `PgAnswerRepository::exists` contains the COUNT query (lines 121-134)
- `PgSearchRepository::search` contains the raw SQL pgvector query +
  answer grouping from `SearchService::search` (lines 17-100)
- `SearchRow` struct moves from `src/services/search_row.rs` into
  `pg_search.rs` (only used by Pg impl)
- Convert `Vec<f32>` parameter to `pgvector::Vector` inside the Pg impl
- `mod.rs` re-exports Pg implementations
- Do NOT add `#[coverage(off)]` -- these are tested by integration tests

**Success criteria**: Implementations compile and implement the traits from
Task 1.

---

### Task 3: Refactor Services to Use Repository Traits

**Scope**: Change service methods to accept `&dyn XxxRepository` instead of
`&DbPool`.

**Files to modify**:
- `src/rust/tokenoverflow/src/services/question.rs`
- `src/rust/tokenoverflow/src/services/answer.rs`
- `src/rust/tokenoverflow/src/services/search.rs`
- `src/rust/tokenoverflow/src/services/mod.rs`

**Requirements**:
- `QuestionService::create` signature: `repo: &dyn QuestionRepository` instead
  of `pool: &DbPool`
- `QuestionService::get_by_id` and `exists`: same change
- `AnswerService::create`, `upvote`, `downvote`, `exists`: same change
- `SearchService::search`: `repo: &dyn SearchRepository` instead of
  `pool: &DbPool`
- Services keep `SYSTEM_USER_ID` logic and pass it to repos
- Services keep embedding error mapping logic
- Services keep tag formatting logic
- Remove `use diesel::prelude::*`, `use diesel_async::*`, etc. from services
- Remove `use crate::db::*` from services
- `mod.rs`: add `pub mod repository`, remove `search_row` module

**Success criteria**: Services compile with the new trait-based signatures.
All Diesel imports removed from service files.

---

### Task 4: Update AppState and Callers

**Scope**: Add repository fields to AppState and update all callers (route
handlers, MCP tools, production wiring).

**Files to modify**:
- `src/rust/tokenoverflow/src/api/state.rs`
- `src/rust/tokenoverflow/src/api/server.rs`
- `src/rust/tokenoverflow/src/api/routes/questions.rs`
- `src/rust/tokenoverflow/src/api/routes/answers.rs`
- `src/rust/tokenoverflow/src/api/routes/search.rs`
- `src/rust/tokenoverflow/src/mcp/tools/submit.rs`
- `src/rust/tokenoverflow/src/mcp/tools/search_questions.rs`
- `src/rust/tokenoverflow/src/mcp/tools/upvote_answer.rs`

**Requirements**:
- `AppState` gains `questions: Arc<dyn QuestionRepository>`, `answers: Arc<dyn
  AnswerRepository>`, `search: Arc<dyn SearchRepository>` fields
- `AppState::new()` takes the new fields
- Route handlers use `state.questions.as_ref()` instead of `&state.pool`
- MCP tools use `self.state.questions.as_ref()` etc.
- Production wiring in `server.rs` creates `PgXxxRepository::new(pool.clone())`
- Health check continues using `state.pool` directly (no change)

**Success criteria**: Production code compiles. `cargo build` succeeds.

---

### Task 5: Create Mock Repositories and Update Test Helpers

**Scope**: Create in-memory mock repositories for unit tests. Update the test
helper module.

**Files to create**:
- `src/rust/tokenoverflow/tests/common/mock_repository.rs`

**Files to modify**:
- `src/rust/tokenoverflow/tests/common/mock_embedding.rs`
- `src/rust/tokenoverflow/tests/common/mod.rs`

**Requirements**:
- `MockStore` struct with shared `Arc<Mutex<Vec<...>>>` collections
- `MockQuestionRepository` implementing `QuestionRepository`:
    - `create`: generates UUIDs, stores in vector, returns IDs
    - `get_by_id`: looks up by ID, returns question + associated answers
    - `exists`: checks if ID is in vector
- `MockAnswerRepository` implementing `AnswerRepository`:
    - `create`: validates question exists in shared store, generates UUID
    - `upvote/downvote`: upserts vote, recalculates counts
    - `exists`: checks if ID is in vector
- `MockSearchRepository` implementing `SearchRepository`:
    - `search`: returns all questions from shared store with a fixed
      similarity score of 0.95, applies tag filtering, respects limit
- `FailingQuestionRepository`, `FailingAnswerRepository`,
  `FailingSearchRepository`: always return `AppError::Internal`
- Helper functions:
    - `create_mock_app_state() -> AppState` (uses mock repos + mock
      embedding + broken pool for health check field)
    - `create_failing_mock_app_state() -> AppState` (failing repos +
      failing embedding)
    - `create_app_state_with_repos(store: &MockStore) -> AppState`
      (custom store)
- Remove old helpers: `create_broken_pool`, `create_mock_pool`,
  `create_app_state_with_pool`, `create_failing_app_state_with_pool`
- `mod.rs` adds `mock_repository` module and re-exports new helpers

**Success criteria**: Mock repositories compile and implement the traits.
Helper functions create valid `AppState` instances.

---

### Task 6: Rewrite All Unit Tests

**Scope**: Replace every `TestDatabase::new().await` in unit tests with mock
repositories.

**Files to modify** (all files in `tests/unit/`):
- `tests/unit/services/test_question.rs`
- `tests/unit/services/test_answer.rs`
- `tests/unit/services/test_search.rs`
- `tests/unit/api/routes/test_questions.rs`
- `tests/unit/api/routes/test_answers.rs`
- `tests/unit/api/routes/test_search.rs`
- `tests/unit/api/routes/test_health.rs`
- `tests/unit/mcp/test_server.rs`
- `tests/unit/mcp/tools/test_submit.rs`
- `tests/unit/mcp/tools/test_search_questions.rs`
- `tests/unit/mcp/tools/test_upvote_answer.rs`

**Requirements**:
- Replace `TestDatabase::new().await` with `MockStore::new()` + mock repos
- Replace `create_app_state_with_pool(test_db.pool().clone())` with
  `create_mock_app_state()` or `create_app_state_with_repos(&store)`
- Replace `create_failing_app_state_with_pool(...)` with
  `create_failing_mock_app_state()`
- Replace `create_broken_pool()` with `FailingXxxRepository`
- Service test calls change from `QuestionService::create(pool, ...)` to
  `QuestionService::create(repo, ...)`
- Delete `health_check_with_real_database_returns_connected` from unit tests
  (will be covered by integration tests)
- All unit tests pass with `cargo test --test unit` and no Docker running
- All unit tests pass with no PostgreSQL installed

**Success criteria**: `cargo test --test unit` passes in under 2 seconds with
zero external dependencies. No `postgres` or `docker` processes needed.

---

### Task 7: Rename Integration Tests to E2E

**Scope**: Move current integration tests to e2e test directory.

**Files to move**:
- `tests/integration/` -> `tests/e2e/`

**Files to modify**:
- `src/rust/tokenoverflow/Cargo.toml` (add e2e test binary)

**Requirements**:
- Create `tests/e2e/` directory with same structure as current
  `tests/integration/`
- Add `[[test]] name = "e2e" path = "tests/e2e/mod.rs"` to Cargo.toml
- `tests/integration/` directory is emptied (will be repopulated in Task 8)
- E2e tests continue using `TestClient::from_config()` -- no logic changes
- `cargo test --test e2e` passes against running Docker Compose stack

**Success criteria**: `docker compose up -d --build api && cargo test --test e2e`
passes with all existing integration tests now running as e2e.

---

### Task 8: Create New Integration Tests with Testcontainers

**Scope**: Write integration tests for Pg repository implementations using
testcontainers.

**Files to create**:
- `tests/integration/mod.rs`
- `tests/integration/test_db.rs` (testcontainers-based `IntegrationTestDb`)
- `tests/integration/repositories/mod.rs`
- `tests/integration/repositories/test_question_repo.rs`
- `tests/integration/repositories/test_answer_repo.rs`
- `tests/integration/repositories/test_search_repo.rs`
- `tests/integration/api/mod.rs`
- `tests/integration/api/routes/mod.rs`
- `tests/integration/api/routes/test_health.rs`

**Files to modify**:
- `src/rust/tokenoverflow/Cargo.toml` (add testcontainers dev-dependencies,
  remove tempfile/portpicker)

**Requirements**:
- `IntegrationTestDb` uses testcontainers-rs with `pgvector/pgvector:pg17` image
- Template database pattern: create once, copy per test
- Container started ONCE per test run (OnceLock pattern)
- Repository tests mirror the tests currently in unit test service files but
  test the real Pg implementations
- Must test: create, get_by_id, exists, upvote, downvote, vote switching,
  FK violations, vector similarity, tag filtering, limit
- Health check test with real DB (moved from unit tests)
- Tests pass with `cargo test --test integration` (Docker must be running)

**Success criteria**: All Pg repository code paths are exercised by integration
tests. `cargo test --test integration` passes.

---

### Task 9: Delete Old Test Infrastructure and Clean Up

**Scope**: Remove the old `test_db.rs` and update dependencies.

**Files to delete**:
- `src/rust/tokenoverflow/tests/common/test_db.rs`
- `src/rust/tokenoverflow/src/services/search_row.rs`

**Files to modify**:
- `src/rust/tokenoverflow/tests/common/mod.rs` (remove `test_db` module)
- `src/rust/tokenoverflow/src/services/mod.rs` (remove `search_row` module
  and its `coverage(off)` exclusion)
- `src/rust/tokenoverflow/Cargo.toml` (remove `tempfile`, `portpicker` from
  dev-dependencies)

**Success criteria**: No references to `test_db.rs`, `search_row.rs`,
`tempfile`, or `portpicker` remain in the codebase. `cargo build` and
`cargo test` succeed.

---

### Task 10: Update Coverage Hook and Verify 100%

**Scope**: Update the pre-commit coverage hook and verify 100% coverage is
maintained.

**Files to modify**:
- `src/shell/tokenoverflow/git_hooks/cargo_coverage.sh`

**Requirements**:
- Add `--test e2e` to the cargo llvm-cov invocation
- Verify the `search_row` coverage exclusion is removed
- Verify no new `#[coverage(off)]` attributes were introduced
- Run full coverage check and confirm 100% line coverage
- Run with `RUST_TEST_THREADS=8` to avoid connection exhaustion

**Success criteria**:
- `cargo test --test unit` passes with no external dependencies
- `cargo test --test integration` passes with Docker running
- `docker compose up -d --build api && cargo test --test e2e` passes
- `RUST_TEST_THREADS=8 cargo +nightly llvm-cov --lib --test unit --test
  integration --test e2e --fail-under-lines 100` passes
- No `postgres` processes spawned during unit tests
- All pre-commit hooks pass
