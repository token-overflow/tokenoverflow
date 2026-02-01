# Design: Sequential ID Enumeration Protection

## Architecture Overview

### Problem

All API endpoints expose sequential `BIGINT IDENTITY` primary keys (`1`, `2`,
`3`, ...) for every table. This allows any client to trivially enumerate every
record by incrementing the ID, enabling full data scraping. For example,
`GET /v1/questions/1`, then `/v1/questions/2`, and so on.

### Solution

Replace all `BIGINT GENERATED ALWAYS AS IDENTITY` primary keys with UUID v7
primary keys across every table. Upgrade PostgreSQL from 17 to 18.3, which
provides a native `uuidv7()` function, allowing DB-side default generation with
`DEFAULT uuidv7()`. No application-side UUID generation is needed.

The production database will be nuked, so the existing init migration is
overwritten in place. No backfill strategy is required.

### Scope

Every table gets UUID v7 primary keys:

| Table            | PK today       | PK after       | FK impact                              |
|------------------|----------------|----------------|----------------------------------------|
| `users`          | `BIGINT`       | `UUID`         | Referenced by answers, questions, votes, api_keys |
| `api_keys`       | `BIGINT`       | `UUID`         | FK to users changes to UUID            |
| `tags`           | `BIGINT`       | `UUID`         | Referenced by tag_synonyms, question_tags |
| `tag_synonyms`   | `BIGINT`       | `UUID`         | FK to tags changes to UUID             |
| `questions`      | `BIGINT`       | `UUID`         | Referenced by answers, question_tags   |
| `question_tags`  | composite PK   | composite UUID | Both FKs change to UUID                |
| `answers`        | `BIGINT`       | `UUID`         | Referenced by votes                    |
| `votes`          | `BIGINT`       | `UUID`         | FKs to answers, users change to UUID   |

Consistent UUID primary keys across all tables eliminates mixed ID types and
simplifies the codebase.

### ID Format Evaluation

| Criteria                  | UUID v4                        | UUID v7                        | ULID                              |
|---------------------------|--------------------------------|--------------------------------|-----------------------------------|
| **Spec / RFC**            | RFC 9562                       | RFC 9562                       | Community spec (ulid/spec)        |
| **Time-sortable**         | No                             | Yes (ms precision)             | Yes (ms precision)                |
| **B-tree friendly**       | No (random distribution)       | Yes (monotonic prefix)         | Yes (monotonic prefix)            |
| **Diesel support**        | Native (`diesel` `uuid` feat)  | Native (`diesel` `uuid` feat)  | Via `diesel-ulid` or manual       |
| **Rust crate**            | `uuid` v1 (stable)             | `uuid` v1 `v7` feat (stable)  | `ulid` v1 (stable)               |
| **Already in Cargo.toml** | Yes (`uuid` v1 with `v4`)      | Yes (same crate, add `v7`)    | No (new dependency)               |
| **PG native generation**  | `gen_random_uuid()` (built-in) | `uuidv7()` (PG 18+)           | Requires extension or app-side    |
| **DB-side default**       | Yes, but random                | Yes, time-sortable             | No native support                 |

### Decision: UUID v7 with PG 18 native `uuidv7()`

1. **Time-sortable.** UUID v7 embeds a millisecond timestamp, producing
   monotonically increasing values that are B-tree friendly (minimal index
   fragmentation, natural chronological ordering).
2. **RFC standard.** UUID v7 is defined in RFC 9562, an IETF standard.
3. **DB-side generation.** PostgreSQL 18 provides `uuidv7()` natively, so
   `DEFAULT uuidv7()` on every ID column eliminates the need for
   application-side UUID generation. Simpler code, fewer moving parts.
4. **Already a dependency.** The `uuid` crate is already in `Cargo.toml`. Adding
   `v7` and `serde` features is a one-line change.
5. **Native Diesel support.** Diesel 2.2 has first-class `uuid` feature support
   with `diesel::sql_types::Uuid`.

### Component Interaction

```
Client Request                     API Layer                 DB Layer
--------------                     ---------                 --------

GET /v1/questions/{uuid}     -->   Route handler parses      UUID PK used for
                                   UUID from path,           all joins and
                                   queries DB by PK          FK lookups
                             <--   Response serializes
                                   UUID as string

POST /v1/questions           -->   Service creates row       DB generates UUID
                             <--   Response returns UUID      v7 PK via DEFAULT
                                                             uuidv7()
```

### Data Flow

1. **Write path (create question/answer):** Service layer calls repository with
   field data (no ID). PostgreSQL generates UUID v7 via `DEFAULT uuidv7()`.
   The `INSERT ... RETURNING id` returns the generated UUID. Response includes
   the UUID.
2. **Read path (get/search):** Route handler receives a UUID string from the URL
   path. Repository queries by primary key (`questions::table.find(id)`).
3. **Internal references:** All foreign keys are UUID, referencing UUID primary
   keys. Joins use UUIDs throughout.

## Interfaces

### PostgreSQL 18 Upgrade

Three files change for the PG 18 upgrade:

**`docker-compose.yml`** -- local development:

```yaml
# Before
image: pgvector/pgvector:pg17

# After
image: pgvector/pgvector:0.8.2-pg18
```

**`infra/terraform/modules/rds/main.tf`** -- production RDS:

```hcl
# Before
family               = "postgres17"
major_engine_version = "17"

# After
family               = "postgres18"
major_engine_version = "18"
```

**`infra/terraform/modules/rds/variables.tf`** -- default engine version:

```hcl
# Before
default = "17"

# After
default = "18.3"
```

### Database Schema Changes

Overwrite `apps/api/migrations/20260131000000_init/up.sql`. All tables change
from `BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY` to
`UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY`. All foreign key columns change
from `BIGINT` to `UUID`.

Key changes per table:

```sql
-- Users
CREATE TABLE users (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    -- ... other columns unchanged
);

-- API keys
CREATE TABLE api_keys (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- ...
);

-- Tags
CREATE TABLE tags (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    -- ...
);

-- Tag synonyms
CREATE TABLE tag_synonyms (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    -- ...
);

-- Questions
CREATE TABLE questions (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    submitted_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    -- ...
);

-- Question tags (composite PK)
CREATE TABLE question_tags (
    question_id UUID NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    tag_id      UUID NOT NULL REFERENCES tags(id) ON DELETE RESTRICT,
    PRIMARY KEY (question_id, tag_id)
);

-- Answers
CREATE TABLE answers (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    question_id UUID NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    submitted_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    -- ...
);

-- Votes
CREATE TABLE votes (
    id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    answer_id UUID NOT NULL REFERENCES answers(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- ...
);
```

The system user insert changes from `OVERRIDING SYSTEM VALUE` with `id = 1` to
a fixed well-known UUID:

```sql
INSERT INTO users (id, workos_id, username)
VALUES ('00000000-0000-0000-0000-000000000001', 'system', 'system');
```

The `setval` call for the identity sequence is removed (no more sequences).

The seed tag and synonym inserts remain unchanged -- `tags.id` and
`tag_synonyms.id` are auto-generated by `DEFAULT uuidv7()`. However, the
synonym inserts reference tags by name via subqueries (e.g.,
`SELECT id FROM tags WHERE name = 'javascript'`), which now returns UUID. The
`$2::bigint[]` cast in `so_tag_sync` raw SQL must change to `$2::uuid[]`.

The down migration remains unchanged (drops the schema).

### Diesel Schema Changes

After running the migration, `diesel print-schema` will regenerate
`apps/api/src/db/schema.rs`. Every `Int8` ID column becomes `Uuid`. Every
`Int8` FK column becomes `Uuid`. The `question_tags` composite PK columns
become `Uuid`.

This requires the `uuid` feature on the `diesel` dependency.

### Cargo.toml Changes

**`apps/api/Cargo.toml`:**

```toml
# Existing -- add "v7" and "serde" features
uuid = { version = "1", features = ["v4", "v7", "serde"] }

# Existing -- add "uuid" feature
diesel = { version = "2.2", features = ["postgres", "chrono", "uuid"] }
```

**`apps/so_tag_sync/Cargo.toml`:**

```toml
# Existing -- add "uuid" feature (for Diesel UUID type support in raw SQL)
diesel = { version = "2.2", features = ["postgres", "chrono", "uuid"] }

# New dependency -- needed for TagIdRow struct
uuid = { version = "1", features = ["v4"] }
```

No other new crate dependencies are introduced.

### Model Changes

All `i64` ID fields become `uuid::Uuid`. All `i64` FK fields become
`uuid::Uuid`.

**`apps/api/src/db/models/question.rs`:**

```rust
use uuid::Uuid;

#[derive(Debug, Insertable)]
#[diesel(table_name = questions)]
pub struct NewQuestion {
    pub title: String,
    pub body: String,
    pub embedding: Vector,
    pub submitted_by: Uuid,      // was i64
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = questions)]
pub struct Question {
    pub id: Uuid,                // was i64
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}
```

Note: `NewQuestion` does not include `id` -- it is generated by the DB default.

**`apps/api/src/db/models/answer.rs`:**

```rust
use uuid::Uuid;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = answers)]
pub struct Answer {
    pub id: Uuid,                // was i64
    pub question_id: Uuid,       // was i64
    pub body: String,
    pub submitted_by: Uuid,      // was i64
    pub upvotes: i32,
    pub downvotes: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = answers)]
pub struct NewAnswer {
    pub question_id: Uuid,       // was i64
    pub body: String,
    pub submitted_by: Uuid,      // was i64
}
```

**`apps/api/src/db/models/user.rs`:**

```rust
use uuid::Uuid;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = users)]
pub struct User {
    pub id: Uuid,                // was i64
    pub workos_id: String,
    pub github_id: Option<i64>,  // GitHub user IDs stay i64 (external system)
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub workos_id: String,
    pub github_id: Option<i64>,
    pub username: String,
}
```

**`apps/api/src/db/models/vote.rs`:**

```rust
use uuid::Uuid;

#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = votes)]
pub struct NewVote {
    pub answer_id: Uuid,         // was i64
    pub user_id: Uuid,           // was i64
    pub value: i32,
}
```

### API Type Changes

All response types replace `i64` IDs with `Uuid`:

**`apps/api/src/api/types/question.rs`:**

```rust
use uuid::Uuid;

pub struct CreateQuestionResponse {
    pub question_id: Uuid,       // was i64
    pub answer_id: Uuid,         // was i64
}

pub struct SearchResultQuestion {
    pub id: Uuid,                // was i64
    // ... rest unchanged
}

pub struct QuestionResponse {
    pub id: Uuid,                // was i64
    // ... rest unchanged
}

pub struct QuestionWithAnswers {
    pub id: Uuid,                // was i64
    // ... rest unchanged
}
```

**`apps/api/src/api/types/answer.rs`:**

```rust
use uuid::Uuid;

pub struct AnswerResponse {
    pub id: Uuid,                // was i64
    // ... rest unchanged
}

impl From<Answer> for AnswerResponse {
    fn from(answer: Answer) -> Self {
        Self {
            id: answer.id,       // Uuid -> Uuid, no conversion needed
            // ...
        }
    }
}
```

### Repository Interface Changes

**`apps/api/src/services/repository/interface/question.rs`:**

```rust
use uuid::Uuid;

#[async_trait]
pub trait QuestionRepository: Send + Sync {
    async fn create(
        &self,
        title: &str,
        body: &str,
        answer_body: &str,
        embedding: Vec<f32>,
        submitted_by: Uuid,          // was i64
    ) -> Result<CreateQuestionResponse, AppError>;

    async fn get_by_id(&self, id: Uuid) -> Result<QuestionWithAnswers, AppError>;

    async fn exists(&self, id: Uuid) -> Result<bool, AppError>;
}
```

**`apps/api/src/services/repository/interface/answer.rs`:**

```rust
use uuid::Uuid;

#[async_trait]
pub trait AnswerRepository: Send + Sync {
    async fn create(
        &self,
        question_id: Uuid,           // was i64
        body: &str,
        submitted_by: Uuid,          // was i64
    ) -> Result<Uuid, AppError>;     // was Result<i64>

    async fn upvote(&self, answer_id: Uuid, user_id: Uuid) -> Result<(), AppError>;

    async fn downvote(&self, answer_id: Uuid, user_id: Uuid) -> Result<(), AppError>;

    async fn exists(&self, id: Uuid) -> Result<bool, AppError>;
}
```

**`apps/api/src/services/repository/interface/tag.rs`:**

```rust
use uuid::Uuid;

#[async_trait]
pub trait TagRepository: Send + Sync {
    async fn load_synonyms(&self) -> Result<HashMap<String, String>, AppError>;
    async fn load_canonicals(&self) -> Result<Vec<String>, AppError>;
    async fn find_tag_ids(&self, names: &[String]) -> Result<Vec<(String, Uuid)>, AppError>;
    async fn link_tags_to_question(
        &self,
        question_id: Uuid,           // was i64
        tag_ids: &[Uuid],            // was &[i64]
    ) -> Result<(), AppError>;
    async fn get_question_tags(&self, question_id: Uuid) -> Result<Vec<String>, AppError>;
}
```

**`apps/api/src/services/repository/interface/user.rs`:**

No signature changes. `find_by_workos_id` returns `User` (which now has
`id: Uuid`). `create` returns `User`.

### Extractor and Middleware Changes

**`apps/api/src/api/extractors.rs`:**

```rust
use uuid::Uuid;

pub struct AuthenticatedUser {
    pub user_id: Uuid,           // was i64
    pub workos_id: String,
}
```

**`apps/api/src/api/middleware.rs`:**

The jwt_auth middleware sets `user_id: user.id` from the `User` model. Since
`User.id` is now `Uuid`, this propagates automatically.

### Constants Change

**`apps/api/src/constants.rs`:**

```rust
use uuid::Uuid;

/// System user UUID used before authentication is implemented (Phase 2).
/// Matches the seeded system user in the init migration.
pub const SYSTEM_USER_ID: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
]);
```

This is the `Uuid` representation of `00000000-0000-0000-0000-000000000001`,
matching the seeded system user in the migration. `Uuid::from_bytes` is a
`const fn`, so this remains a compile-time constant.

### Route Handler Changes

**`apps/api/src/api/routes/questions.rs`:**

```rust
use uuid::Uuid;

pub async fn get_question(State(state): State<AppState>, Path(id_str): Path<String>) -> Response {
    let id: Uuid = match id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return AppError::Validation("Invalid question ID format".to_string()).into_response();
        }
    };

    match QuestionService::get_by_id(state.questions.as_ref(), id).await {
        // ...
    }
}
```

The same pattern applies to `add_answer` (parse question UUID from path),
`upvote`/`downvote` (parse answer UUID from path).

### Service Layer Changes

**`apps/api/src/services/question.rs`:**

```rust
use uuid::Uuid;

impl QuestionService {
    pub async fn create(
        // ... same params but submitted_by: Uuid
        submitted_by: Uuid,          // was i64
    ) -> Result<CreateQuestionResponse, AppError> {
        // ...
        // tag_pairs type changes: Vec<(String, Uuid)>
        let tag_ids: Vec<Uuid> = tag_pairs.into_iter().map(|(_, id)| id).collect();
        tag_repo.link_tags_to_question(response.question_id, &tag_ids).await?;
        // ...
    }

    pub async fn get_by_id(repo: &dyn QuestionRepository, id: Uuid) -> ...
    pub async fn exists(repo: &dyn QuestionRepository, id: Uuid) -> ...
}
```

**`apps/api/src/services/answer.rs`:** All `i64` params become `Uuid`.

### MCP Tool Changes

**`apps/api/src/mcp/tools/submit.rs`:**

`SYSTEM_USER_ID` is now `Uuid`. The `response.question_id.to_string()` call
naturally produces UUID string representation. No structural change needed.

**`apps/api/src/mcp/tools/upvote_answer.rs`:**

```rust
let answer_id: Uuid = input
    .answer_id
    .parse()
    .map_err(|_| AppError::Validation("Invalid answer ID format".to_string()))?;

AnswerService::upvote(self.state.answers.as_ref(), answer_id, SYSTEM_USER_ID).await?;
```

**`apps/api/src/mcp/tools/search_questions.rs`:**

The `SearchResult` and `AnswerResult` structs use `String` for IDs. Since
`SearchResultQuestion.id` and `AnswerResponse.id` are now `Uuid`, the
`.to_string()` calls naturally produce UUID strings. No structural change.

### Postgres Repository Implementation Changes

**`apps/api/src/services/repository/postgres/question.rs`:**

```rust
// create() -- no app-side UUID generation needed
let new_question = NewQuestion {
    title,                       // no id field -- DB generates it
    body,
    embedding: embedding_vector,
    submitted_by,                // now Uuid
};

let question_id: Uuid = diesel::insert_into(questions::table)
    .values(&new_question)
    .returning(questions::id)    // returns UUID from DB
    .get_result(conn)
    .await?;

// get_by_id() -- still uses .find() since id IS the PK
let question: Question = questions::table
    .find(id)                    // id is now Uuid, .find() works on PK
    .select(Question::as_select())
    .first(&mut conn)
    .await
    .optional()?;
```

**`apps/api/src/services/repository/postgres/answer.rs`:**

The `vote()` method changes all `i64` ID parameters to `Uuid`. The
`diesel_fk_not_found` helper still works since its `id` parameter is
`impl std::fmt::Display`, which `Uuid` implements.

**`apps/api/src/services/repository/postgres/search.rs`:**

The `SearchRow` struct changes `id` from `BigInt`/`i64` to
`diesel::sql_types::Uuid`/`uuid::Uuid`. The raw SQL queries stay the same
(they select `q.id` which is now a UUID column). The answer/tag join logic
changes from `Vec<i64>` to `Vec<Uuid>` for `question_ids`.

**`apps/api/src/services/repository/postgres/tag.rs`:**

`find_tag_ids` returns `Vec<(String, Uuid)>`. `link_tags_to_question` accepts
`Uuid` for question_id and `&[Uuid]` for tag_ids. `get_question_tags` accepts
`Uuid`.

### so_tag_sync Crate Changes

**`apps/so_tag_sync/src/db.rs`:**

The `TagIdRow` struct changes from `BigInt`/`i64` to
`diesel::sql_types::Uuid`/`uuid::Uuid`:

```rust
#[derive(QueryableByName)]
struct TagIdRow {
    #[diesel(sql_type = VarChar)]
    name: String,
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    id: uuid::Uuid,
}
```

The `upsert_synonyms` function changes:
- `tag_map` type: `HashMap<String, i64>` becomes `HashMap<String, uuid::Uuid>`
- `tag_ids` vec type: `Vec<i64>` becomes `Vec<uuid::Uuid>`
- The raw SQL bind changes from `$2::bigint[]` to `$2::uuid[]`
- The Diesel bind type changes from `Array<BigInt>` to `Array<diesel::sql_types::Uuid>`

The `upsert_tags` and `get_last_sync_date` functions do not reference IDs
directly and require no changes.

**`apps/so_tag_sync/Cargo.toml`** needs `uuid` as a dependency and the `uuid`
feature on `diesel`.

The so_tag_sync tests (`tests/integration/test_db.rs`) do not assert on ID
values directly -- they only check row counts and existence. No test changes
are expected beyond what the type changes require for compilation.

### Bruno Collection Changes

Update default variable values from numeric IDs to example UUIDs:

**`bruno/tokenoverflow/collections/api/v1/get-question.yml`:**

```yaml
runtime:
  variables:
    - name: question_id
      value: "01961e5a-b574-7430-9bfb-cd3248718aa0"
```

**`bruno/tokenoverflow/collections/api/v1/add-answer.yml`:**

```yaml
runtime:
  variables:
    - name: question_id
      value: "01961e5a-b574-7430-9bfb-cd3248718aa0"
```

**`bruno/tokenoverflow/collections/api/v1/upvote-question.yml`:**

```yaml
runtime:
  variables:
    - name: answer_id
      value: "01961e5a-b574-7431-8a2c-e19f3c5d7b01"
```

**`bruno/tokenoverflow/collections/api/v1/downvote-question.yml`:**

```yaml
runtime:
  variables:
    - name: answer_id
      value: "01961e5a-b574-7431-8a2c-e19f3c5d7b01"
```

These are example UUIDs for manual testing. They will not resolve to real
records but serve as format examples.

## Logic

### UUID v7 Generation

All UUID v7 values are generated database-side via `DEFAULT uuidv7()`.
PostgreSQL 18 provides this function natively. Application code does not call
`uuid::Uuid::now_v7()` for ID generation.

On insert, the application omits the `id` field from the `Insertable` struct.
The DB fills it via the default. The `INSERT ... RETURNING id` clause returns
the generated UUID to the application.

### Primary Key Lookup

Since the UUID is now the actual primary key, all read operations continue to
use `questions::table.find(id)` (Diesel's PK lookup). No change from the
current lookup pattern -- only the type changes from `i64` to `Uuid`.

### Answer Creation

The `POST /v1/questions/{id}/answers` endpoint receives the question UUID in
the path. Since `answers.question_id` is now a UUID FK referencing
`questions.id` (also UUID), the route handler parses the UUID from the path
and passes it directly to `NewAnswer.question_id`. No internal ID resolution
step is needed.

If the question UUID does not exist, the FK constraint violation is caught by
`diesel_fk_not_found` and returned as a 404.

### Voting

The `upvote` and `downvote` endpoints receive an answer UUID from the path.
Since `votes.answer_id` is now a UUID FK referencing `answers.id` (also UUID),
the handler parses the UUID and uses it directly in the vote upsert and the
answer count update queries. No internal ID resolution step is needed.

### Search Result Assembly

In `PgSearchRepository::search()`, the raw SQL query returns `q.id` (now UUID).
The `SearchRow` struct uses `diesel::sql_types::Uuid` and `uuid::Uuid`. The
answer and tag joins use `Vec<Uuid>` for `question_ids`. The final result
assembly maps `q.id` (UUID) directly to `SearchResultQuestion.id` (UUID).

### Tag Linking

`TagRepository::find_tag_ids` returns `Vec<(String, Uuid)>`.
`TagRepository::link_tags_to_question` accepts `question_id: Uuid` and
`tag_ids: &[Uuid]`. The `QuestionService::create()` method resolves tags and
passes UUID tag IDs to the tag repository. This is the same flow as today,
just with UUIDs instead of i64s.

## Edge Cases & Constraints

### UUID Collision

UUID v7 has 74 random bits per millisecond. The probability of collision is
approximately 1 in 2^74 per millisecond window -- astronomically low. The
primary key constraint provides a hard guarantee: if a collision somehow occurs,
the INSERT fails with a unique constraint violation. No application-side retry
logic is needed.

### URL Length

UUID v7 string representation is 36 characters (e.g.,
`01961e5a-b574-7430-9bfb-cd3248718aa0`). Well within URL path segment limits.

### Case Sensitivity

PostgreSQL's `UUID` type is case-insensitive. `uuid::Uuid::parse_str()` is also
case-insensitive. No special handling needed.

### Performance

- **PK lookup by UUID:** Primary key B-tree index. Same O(log n) performance as
  BIGINT PK lookups.
- **Storage:** UUID is 16 bytes vs. BIGINT's 8 bytes. For the scale of this
  application, the difference is negligible.
- **Index performance:** UUID v7 is time-sorted, so B-tree inserts append to the
  right side of the index (similar to sequential BIGINT). No fragmentation
  concern.
- **Join performance:** UUID joins are slightly slower than BIGINT joins due to
  16-byte vs 8-byte comparisons. At the table sizes this application will see
  (thousands to low millions of rows), this is immeasurable.
- **DB-side generation:** `uuidv7()` is a built-in PG 18 function. No overhead
  compared to identity sequence generation.

### MCP Tool Input Validation

The MCP `upvote_answer` tool currently parses `answer_id` as `i64`. After this
change, it parses as `Uuid`. Invalid UUID format strings produce the same
`AppError::Validation` error.

### System User ID

`SYSTEM_USER_ID` changes from `i64 = 1` to a well-known UUID constant
(`00000000-0000-0000-0000-000000000001`). This is used by MCP tools as the
`submitted_by` / `user_id` value until MCP-specific auth is implemented. The
seeded system user in the migration uses this same UUID.

### pgvector Compatibility

pgvector 0.8.2 supports PostgreSQL 18. The Docker image
`pgvector/pgvector:0.8.2-pg18` provides both.

### PgBouncer Compatibility

PgBouncer 1.25.1+ supports PostgreSQL 18. The `edoburu/pgbouncer:latest` image
uses a recent PgBouncer version.

### Diesel/diesel-async Compatibility

Diesel 2.2 and diesel-async 0.5 work with PostgreSQL 18. The PostgreSQL wire
protocol is backward-compatible; the client libraries do not need PG-version-
specific builds.

### Search Repository Raw SQL

The `PgSearchRepository` uses raw SQL queries via `diesel::sql_query()`. The
SQL itself does not change (it selects `q.id`, `q.title`, `q.body` and
computes similarity). The only change is the `SearchRow` struct: `id` changes
from `BigInt`/`i64` to `diesel::sql_types::Uuid`/`uuid::Uuid`.

## Test Plan

### Unit Tests

**Affected test files:**

- `tests/unit/api/routes/test_questions.rs` -- Update ID types in route handler
  tests
- `tests/unit/api/routes/test_answers.rs` -- Update vote endpoint tests
- `tests/unit/api/routes/test_search.rs` -- Update search response assertions
- `tests/unit/api/types/test_question.rs` -- Update type serialization tests
- `tests/unit/api/types/test_answer.rs` -- Update type serialization tests
- `tests/unit/mcp/tools/test_submit.rs` -- Verify UUID in submit response
- `tests/unit/mcp/tools/test_search_questions.rs` -- Verify UUID in search
  results
- `tests/unit/mcp/tools/test_upvote_answer.rs` -- Verify UUID parsing
- `tests/unit/services/test_question.rs` -- Update service tests
- `tests/unit/services/test_answer.rs` -- Update service tests
- `tests/unit/services/test_search.rs` -- Update service tests

**New test cases:**

- Route handler returns 422 for invalid UUID format (e.g., "not-a-uuid",
  "12345")
- Route handler returns 422 for old-style numeric ID ("1", "42")
- Serialization of `Uuid` fields produces standard UUID string format
- Mock repository uses UUID values throughout

**Mock repository updates (`tests/common/mock_repository.rs`):**

- `StoredQuestion.id`, `StoredAnswer.id`, `StoredVote.answer_id`,
  `StoredVote.user_id`, `StoredQuestionTag.question_id`,
  `StoredQuestionTag.tag_id`, `StoredTag.id` all change from `i64` to `Uuid`
- `next_id()` changes from `AtomicI64` to `Uuid::now_v7()`
- `MockUserRepository` seeds the system user with the well-known UUID constant
- All repository implementations updated to match new trait signatures

### Integration Tests

**Affected test files:**

- `tests/integration/repositories/test_question_repo.rs` -- Verify UUID PKs
- `tests/integration/repositories/test_answer_repo.rs` -- Verify UUID PKs
- `tests/integration/repositories/test_search_repo.rs` -- Verify search returns
  UUIDs
- `tests/integration/repositories/test_tag_repo.rs` -- Verify UUID tag IDs
- `tests/integration/repositories/test_user_repo.rs` -- Verify UUID user IDs

**New test cases:**

- Insert a question, verify returned ID is a valid UUID
- Retrieve a question by UUID PK, verify correct row returned
- Insert two questions, verify different UUID values
- Lookup by non-existent UUID returns `NotFound`

### E2E Tests

**Affected test files:**

- `tests/e2e/api/routes/test_questions.rs` -- Full HTTP round-trip with UUID
- `tests/e2e/api/routes/test_answers.rs` -- Vote endpoints with UUID
- `tests/e2e/api/routes/test_search.rs` -- Search results contain UUIDs
- `tests/e2e/mcp/tools/test_submit.rs` -- MCP submit returns UUID
- `tests/e2e/mcp/tools/test_search_questions.rs` -- MCP search returns UUID
- `tests/e2e/mcp/tools/test_upvote_answer.rs` -- MCP upvote with UUID

**New test cases:**

- Create a question via POST, verify response contains valid UUID strings
- Retrieve the created question using the returned UUID
- Attempt to GET a question with a numeric ID ("1") -- expect 422
- Attempt to GET a question with a malformed UUID -- expect 422
- Attempt to GET a question with a valid but non-existent UUID -- expect 404
- Full flow: create question -> search -> upvote answer (all via UUIDs)

## Documentation Changes

### MCP Tool Descriptions

The `UpvoteAnswerInput` struct's `answer_id` field description should be updated
to mention UUID format:

```rust
/// ID of the answer to upvote (UUID format). Get this from search_questions results.
pub answer_id: String,
```

### Bruno Collection

The Bruno collection files serve as living API documentation. Default variable
values change from numeric IDs to example UUIDs as described in the Interfaces
section.

### README.md

No changes needed. The README documents setup, architecture, and testing
commands, none of which change.

## Development Environment Changes

- **Docker Compose:** PostgreSQL image changes from `pgvector/pgvector:pg17` to
  `pgvector/pgvector:0.8.2-pg18`. Existing local `postgres_data` volume must be
  deleted and recreated (`docker compose down -v && docker compose up -d`).
- **Brewfile:** No new system dependencies.
- **Environment Variables:** No new env vars needed.
- **Setup Scripts:** No changes.
- **Config Files:** No changes to TOML config files.
- **Diesel Migrations Dockerfile:** No changes. The `diesel_cli` binary links
  against `libpq` which supports connecting to PG 18 servers.

## Tasks

### Task 1: PostgreSQL 18 Upgrade and Migration Rewrite

**Scope:** Upgrade PG from 17 to 18.3 in all infrastructure files and rewrite
the init migration to use UUID v7 primary keys on all tables.

**Requirements:**

- Update `docker-compose.yml`: `pgvector/pgvector:pg17` to
  `pgvector/pgvector:0.8.2-pg18`
- Update `infra/terraform/modules/rds/main.tf`: `family` to `"postgres18"`,
  `major_engine_version` to `"18"`
- Update `infra/terraform/modules/rds/variables.tf`: default engine_version to
  `"18.3"`
- Overwrite `apps/api/migrations/20260131000000_init/up.sql`:
    - All tables use `UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY`
    - All FK columns use `UUID NOT NULL REFERENCES ...`
    - System user insert uses fixed UUID
      `00000000-0000-0000-0000-000000000001`
    - Remove `setval` call (no more sequences)
    - Tag and synonym seed data unchanged (IDs auto-generated)
- Down migration unchanged (drops schema)

**Success criteria:**

- `docker compose down -v && docker compose up -d` succeeds with PG 18
- `diesel migration run` succeeds on a fresh PG 18 database
- `diesel migration redo` succeeds
- All tables have UUID primary keys
- System user has the expected UUID

### Task 2: Update Cargo.toml Dependencies and Regenerate Diesel Schema

**Scope:** Add required feature flags to both crates and regenerate the Diesel
schema.

**Requirements:**

- `apps/api/Cargo.toml`: add `"v7"` and `"serde"` to `uuid` features, add
  `"uuid"` to `diesel` features
- `apps/so_tag_sync/Cargo.toml`: add `"uuid"` to `diesel` features, add `uuid`
  dependency with `"v4"` feature
- Run `diesel print-schema` to regenerate `apps/api/src/db/schema.rs`
- Verify the generated schema shows `Uuid` for all ID and FK columns

**Success criteria:**

- `cargo check --workspace` passes with the new features
- Schema file matches the PG 18 database

### Task 3: Update Constants, Extractors, and DB Models

**Scope:** Change `SYSTEM_USER_ID` to UUID, update `AuthenticatedUser.user_id`
to UUID, and update all Diesel model structs from `i64` to `Uuid`.

**Requirements:**

- `apps/api/src/constants.rs`: change `SYSTEM_USER_ID` from `i64 = 1` to
  `Uuid` constant `00000000-0000-0000-0000-000000000001`
- `apps/api/src/api/extractors.rs`: change `user_id` from `i64` to `Uuid`
- `apps/api/src/db/models/question.rs`: `Question.id` and
  `NewQuestion.submitted_by` to `Uuid`
- `apps/api/src/db/models/answer.rs`: `Answer.id`, `Answer.question_id`,
  `Answer.submitted_by`, `NewAnswer.question_id`, `NewAnswer.submitted_by` to
  `Uuid`
- `apps/api/src/db/models/user.rs`: `User.id` to `Uuid`
  (`github_id` stays `Option<i64>`)
- `apps/api/src/db/models/vote.rs`: `NewVote.answer_id`, `NewVote.user_id`
  to `Uuid`
- Import `uuid::Uuid` in all modified files
- Ensure field ordering matches Diesel schema column order

**Success criteria:**

- `cargo check` passes
- All derives (`Queryable`, `Selectable`, `Insertable`, `AsChangeset`) compile

### Task 4: Update API Types and Repository Interfaces

**Scope:** Replace `i64` with `Uuid` in all API response types and repository
trait signatures.

**Requirements:**

- API types: `CreateQuestionResponse`, `SearchResultQuestion`,
  `QuestionResponse`, `QuestionWithAnswers`, `AnswerResponse` -- all `i64` ID
  fields become `Uuid`
- `AnswerResponse::from(Answer)` maps `answer.id` (Uuid) to `id` (Uuid)
- `QuestionRepository`: `get_by_id(Uuid)`, `exists(Uuid)`,
  `create(..., submitted_by: Uuid)`
- `AnswerRepository`: `create(question_id: Uuid, ..., submitted_by: Uuid)
  -> Result<Uuid>`, `upvote(Uuid, Uuid)`, `downvote(Uuid, Uuid)`,
  `exists(Uuid)`
- `TagRepository`: `find_tag_ids -> Vec<(String, Uuid)>`,
  `link_tags_to_question(Uuid, &[Uuid])`, `get_question_tags(Uuid)`
- `SearchRepository`: no signature change (returns updated types)
- `UserRepository`: no signature change (returns updated `User` model)

**Success criteria:**

- Trait definitions compile
- Consistent Uuid types across all interface boundaries

### Task 5: Update Postgres Repository Implementations

**Scope:** Update all Postgres repository implementations for UUID-based
queries.

**Requirements:**

- `PgQuestionRepository::create`: no `id` in `NewQuestion`, return UUID from
  `RETURNING id`
- `PgQuestionRepository::get_by_id`: `find(id)` with UUID (PK lookup unchanged)
- `PgQuestionRepository::exists`: filter by `questions::id.eq(id)` with UUID
- `PgAnswerRepository::create`: `question_id` is UUID, return UUID from
  `RETURNING id`
- `PgAnswerRepository::vote`: all ID params are UUID
- `PgSearchRepository::search`: `SearchRow.id` is UUID, `question_ids` is
  `Vec<Uuid>`, tag join uses UUID
- `PgTagRepository`: `find_tag_ids` returns UUID, `link_tags_to_question`
  accepts UUID, `get_question_tags` accepts UUID

**Success criteria:**

- All repository implementations compile
- UUID values correctly flow through queries

### Task 6: Update Service Layer, Route Handlers, and MCP Tools

**Scope:** Propagate `Uuid` type through services, routes, and MCP tools.

**Requirements:**

- `QuestionService`: `create(..., submitted_by: Uuid)`, `get_by_id(Uuid)`,
  `exists(Uuid)`, tag_ids as `Vec<Uuid>`
- `AnswerService`: `create(Uuid, ..., Uuid)`, `upvote(Uuid, Uuid)`,
  `downvote(Uuid, Uuid)`, `exists(Uuid)`
- Route handlers (`questions.rs`, `answers.rs`): parse `Uuid` from path instead
  of `i64`, pass `user.user_id` (now Uuid) to services
- MCP tools: `upvote_answer.rs` parses `Uuid` instead of `i64`,
  `SYSTEM_USER_ID` is now `Uuid`, update `UpvoteAnswerInput` doc comment

**Success criteria:**

- All services, routes, and MCP tools compile
- Invalid UUID strings in path produce 422 validation error
- `SYSTEM_USER_ID` flows correctly as UUID

### Task 7: Update so_tag_sync Crate

**Scope:** Update the so_tag_sync database code for UUID tag IDs.

**Requirements:**

- `apps/so_tag_sync/src/db.rs`: `TagIdRow.id` from `BigInt`/`i64` to
  `diesel::sql_types::Uuid`/`uuid::Uuid`
- `tag_map` type from `HashMap<String, i64>` to `HashMap<String, Uuid>`
- `tag_ids` vec from `Vec<i64>` to `Vec<Uuid>`
- Raw SQL bind from `$2::bigint[]` to `$2::uuid[]`
- Diesel bind type from `Array<BigInt>` to `Array<diesel::sql_types::Uuid>`
- Remove unused `BigInt` import if no longer needed

**Success criteria:**

- `cargo check -p so_tag_sync` passes
- `cargo test -p so_tag_sync --test integration` passes (against PG 18
  container)

### Task 8: Update Mock Repository and Test Helpers

**Scope:** Update the test mock infrastructure for UUID-based IDs.

**Requirements:**

- `StoredQuestion.id`, `StoredAnswer.id`, `StoredAnswer.question_id`,
  `StoredVote.answer_id`, `StoredVote.user_id`, `StoredQuestionTag.question_id`,
  `StoredQuestionTag.tag_id`, `StoredTag.id` all change to `Uuid`
- Replace `AtomicI64` `next_id()` with `Uuid::now_v7()` based helper
- `MockUserRepository`: seed system user with
  `00000000-0000-0000-0000-000000000001`
- Update `MockQuestionRepository`, `MockAnswerRepository`,
  `MockSearchRepository`, `MockTagRepository` to match new trait signatures
- Update `FailingQuestionRepository`, `FailingAnswerRepository`,
  `FailingTagRepository` parameter types
- Update `tests/common/http_client.rs` if it references i64 IDs

**Success criteria:**

- Mock repositories compile and match updated trait signatures
- Unit tests using mocks can exercise UUID-based lookups

### Task 9: Update All Tests

**Scope:** Fix all existing tests and add new UUID-specific test cases.

**Requirements:**

- Update all unit tests to use UUID-based IDs
- Update all integration tests to verify UUID persistence
- Update all E2E tests to send UUID strings in HTTP requests
- Add new test cases for UUID validation (invalid format, numeric ID rejection)
- Add new test case for non-existent UUID returning 404
- Update test database infrastructure if it references PG 17 specifically

**Success criteria:**

- `cargo test --workspace --test unit` passes
- `cargo test --workspace --test integration` passes
- `cargo test -p tokenoverflow --test e2e` passes

### Task 10: Update Bruno Collection

**Scope:** Replace numeric ID defaults with example UUIDs.

**Requirements:**

- Update `get-question.yml`: `question_id` to example UUID
- Update `add-answer.yml`: `question_id` to example UUID
- Update `upvote-question.yml`: `answer_id` to example UUID
- Update `downvote-question.yml`: `answer_id` to example UUID

**Success criteria:**

- Bruno collection files contain valid UUID format examples
- Manual API testing with Bruno works after creating a question
