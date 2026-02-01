# Design: service_layer_clarity

## Problem Statement

The current `services/` directory in `apps/api/src/` is confusing for two
reasons:

1. **The word "services" is vague.** It groups two fundamentally different
   responsibilities under one roof: business logic orchestration (e.g.,
   `services/answer.rs`) and database access (e.g.,
   `services/repository/pg_answer.rs`). A newcomer opening the `services/`
   folder sees both and cannot tell what belongs where.

2. **The word "repository" is jargon.** It comes from Domain-Driven Design (DDD)
   literature and is not self-explanatory. A developer unfamiliar with DDD
   patterns has to look it up to understand that a "repository" is just "the
   code that talks to the database."

3. **Duplicate file names cause confusion.** There is `services/answer.rs` (the
   business logic) and `services/repository/answer.rs` (the trait definition for
   database access). Both are called `answer.rs` but do completely different
   things.

### What these layers actually do today

The codebase has two distinct layers, but they are nested instead of being
siblings:

```text
services/                      <-- business logic + database access mixed
  answer.rs                    <-- business logic (calls the trait)
  question.rs                  <-- business logic (calls the trait)
  search.rs                    <-- business logic (calls the trait)
  tags.rs                      <-- tag normalization utility
  tag_resolver.rs              <-- tag resolution logic
  repository/                  <-- database access (nested inside services)
    answer.rs                  <-- trait definition
    pg_answer.rs               <-- PostgreSQL implementation of the trait
    question.rs                <-- trait definition
    pg_question.rs             <-- PostgreSQL implementation of the trait
    search.rs                  <-- trait definition
    pg_search.rs               <-- PostgreSQL implementation of the trait
    tag.rs                     <-- trait definition
    pg_tag.rs                  <-- PostgreSQL implementation of the trait
```

**Layer 1: Business Logic (e.g., `services/answer.rs`)**
- Orchestrates operations that involve multiple steps
- Calls external services (e.g., embedding API)
- Applies business rules (e.g., system user ID, tag resolution)
- Does NOT know about PostgreSQL, Diesel, or any database library
- Receives a trait object (`&dyn AnswerRepository`) so it can be tested with
  mocks

**Layer 2: Database Access (e.g., `services/repository/pg_answer.rs`)**
- Talks directly to PostgreSQL using Diesel ORM
- Implements the trait defined in the sibling file (e.g., `answer.rs`)
- Knows about connection pools, SQL queries, and transactions
- Has zero business logic

### Why two layers exist

The separation exists for testability. Unit tests for business logic use mock
implementations of the traits (see `tests/common/mock_repository.rs`), so they
run instantly without a database. Integration tests use the real PostgreSQL
implementations via testcontainers. Without this separation, every test would
need a live database.

## Architecture Overview

This design renames and reorganizes the two layers so that their purpose is
obvious at first glance, without requiring knowledge of DDD terminology.

### Current vs. proposed structure

```text
CURRENT                              PROPOSED (Option A - Recommended)
-------                              ---------
src/                                 src/
  services/                            ops/
    answer.rs                            answer.rs
    question.rs                          question.rs
    search.rs                            search.rs
    tags.rs                              tags.rs
    tag_resolver.rs                      tag_resolver.rs
    repository/                        store/
      answer.rs                          answer.rs        (trait)
      pg_answer.rs                       pg_answer.rs     (Postgres impl)
      question.rs                        question.rs      (trait)
      pg_question.rs                     pg_question.rs   (Postgres impl)
      search.rs                          search.rs        (trait)
      pg_search.rs                       pg_search.rs     (Postgres impl)
      tag.rs                             tag.rs           (trait)
      pg_tag.rs                          pg_tag.rs        (Postgres impl)
```

The key structural change: `store/` is a **sibling** of `ops/`, not a child.
This makes the two layers visually equal in the file tree and removes the false
impression that database access is a sub-concern of business logic.

### Alternatives considered

| # | Business Logic Dir | Database Access Dir | Pros | Cons |
|---|---|---|---|---|
| **A (Recommended)** | `ops/` | `store/` | Short, plain English, no jargon. "ops" = operations/actions. "store" = where data is stored. Both are self-explanatory to non-Rust developers. Siblings in the tree. | "ops" is less conventional than "services" in enterprise patterns. |
| B | `services/` | `store/` | Keeps familiar "services" name. "store" replaces "repository" with plain English. Siblings in the tree. | "services" is still somewhat vague -- could mean HTTP services, microservices, etc. |
| C | `actions/` | `store/` | Very explicit -- each file is an "action" the system performs. | "actions" could be confused with Redux/Flux patterns by frontend developers. |
| D | `services/` | `db/` | "db" is the most direct name possible. | Conflicts with the existing `src/db/` module (connection pool, schema, models). Also too narrow -- what if we add Redis or in-memory caches later? |
| E (Status quo) | `services/` | `services/repository/` | No code changes needed. | The original problem: confusing nesting, jargon, duplicate file names. |

### What each name means in plain English

- **`ops/`** -- Short for "operations." Each file in this directory is a set of
  related operations the system can perform: creating questions, upvoting
  answers, running searches. Think of it as "what the app does."

- **`store/`** -- Where data is stored and retrieved. Each file defines a
  contract (trait) for how data is accessed, plus a PostgreSQL implementation.
  Think of it as "where the app keeps its data."

### Example: how a request flows through the layers

```text
HTTP Request
    |
    v
routes/questions.rs          (API layer: parse request, validate)
    |
    v
ops/question.rs              (Operations: orchestrate logic, call embedding API)
    |
    v
store/pg_question.rs         (Store: run SQL via Diesel, return data)
    |
    v
PostgreSQL
```

## Interfaces

### Module public API changes

The public API of the modules stays identical. Only the module paths change.

**Before:**

```rust
use crate::services::AnswerService;
use crate::services::QuestionService;
use crate::services::SearchService;
use crate::services::TagResolver;
use crate::services::repository::{AnswerRepository, PgAnswerRepository};
```

**After:**

```rust
use crate::ops::AnswerService;
use crate::ops::QuestionService;
use crate::ops::SearchService;
use crate::ops::TagResolver;
use crate::store::{AnswerRepository, PgAnswerRepository};
```

### Files that need import path updates

Every file that imports from `services` or `services::repository` must be
updated. Based on the current codebase, these are:

| File | Current import | New import |
|------|---------------|------------|
| `src/lib.rs` | `pub mod services` | `pub mod ops` + `pub mod store` |
| `src/api/state.rs` | `use crate::services::*` | `use crate::ops::*` + `use crate::store::*` |
| `src/api/routes/questions.rs` | `use crate::services::{...}` | `use crate::ops::{...}` |
| `src/api/routes/answers.rs` | `use crate::services::AnswerService` | `use crate::ops::AnswerService` |
| `src/api/routes/search.rs` | `use crate::services::SearchService` | `use crate::ops::SearchService` |
| `src/api/server.rs` | `use crate::services::*` | `use crate::ops::*` + `use crate::store::*` |
| `src/mcp/server.rs` | (indirect via `AppState`) | No change needed |
| `src/mcp/tools/submit.rs` | (indirect via `AppState`) | Check for direct imports |
| `src/mcp/tools/search_questions.rs` | (indirect via `AppState`) | Check for direct imports |
| `src/mcp/tools/upvote_answer.rs` | (indirect via `AppState`) | Check for direct imports |
| All `tests/unit/services/*.rs` | `use tokenoverflow::services::*` | `use tokenoverflow::ops::*` |
| `tests/common/mock_repository.rs` | `use tokenoverflow::services::repository::*` | `use tokenoverflow::store::*` |
| All `tests/integration/repositories/*.rs` | `use tokenoverflow::services::repository::*` | `use tokenoverflow::store::*` |

### Internal cross-references

Files inside `ops/` currently reference `repository` via `super::repository::*`.
After the split, they will reference the sibling module via `crate::store::*`.

**Before (inside `services/answer.rs`):**

```rust
use super::repository::AnswerRepository;
```

**After (inside `ops/answer.rs`):**

```rust
use crate::store::AnswerRepository;
```

## Logic

### No behavioral changes

This is a pure rename/restructure. Zero business logic changes. The trait
definitions, implementations, service methods, and their signatures all remain
identical. The only changes are:

1. Directory names: `services/` becomes `ops/`, `services/repository/` becomes
   `store/`
2. `mod` declarations in `src/lib.rs`
3. `use` / `mod` paths throughout the codebase
4. Test directory names: `tests/unit/services/` becomes `tests/unit/ops/`,
   `tests/integration/repositories/` becomes `tests/integration/store/`

### mod.rs contents

**`src/ops/mod.rs`** (identical content to current `src/services/mod.rs`, minus
the `repository` sub-module):

```rust
mod answer;
mod question;
mod search;
pub mod tag_resolver;
pub mod tags;

pub use answer::AnswerService;
pub use question::QuestionService;
pub use search::SearchService;
pub use tag_resolver::TagResolver;
```

**`src/store/mod.rs`** (identical content to current
`src/services/repository/mod.rs`):

```rust
mod answer;
mod pg_answer;
mod pg_question;
mod pg_search;
mod pg_tag;
mod question;
mod search;
mod tag;

pub use answer::AnswerRepository;
pub use pg_answer::PgAnswerRepository;
pub use pg_question::PgQuestionRepository;
pub use pg_search::PgSearchRepository;
pub use pg_tag::PgTagRepository;
pub use question::QuestionRepository;
pub use search::SearchRepository;
pub use tag::TagRepository;
```

**`src/lib.rs`** changes:

```rust
// Before:
pub mod services;

// After:
pub mod ops;
pub mod store;
```

## Edge Cases & Constraints

### Risk: broken imports

Every `use crate::services::...` path in production code and tests will fail to
compile after the rename. This is actually a safety net -- the Rust compiler will
catch every single missed update. The risk of a silent regression is zero.

### Risk: git blame disruption

Moving files with `git mv` preserves history tracking. Using a single commit for
the entire rename ensures `git log --follow` works correctly.

### Risk: merge conflicts

If other branches are in flight with changes to `services/` files, they will
conflict. Mitigation: coordinate the rename to land when no other branches touch
these files, or land it early and rebase.

### Constraint: test directory naming

Test directories under `tests/unit/services/` and
`tests/integration/repositories/` must also be renamed to match:
- `tests/unit/services/` becomes `tests/unit/ops/`
- `tests/integration/repositories/` becomes `tests/integration/store/`

### Constraint: snake_case compliance

Both `ops` and `store` are valid snake_case identifiers, consistent with the
project's CLAUDE.md rule: "Use snake_case naming convention for the entire
monorepo."

## Test Plan

### Compilation as verification

Since this is a rename-only change with no behavioral modifications, the primary
verification is that the project compiles and all existing tests pass:

```bash
# All tests must pass with zero changes to assertions or test logic
cargo test --workspace
```

### Checklist

- [ ] `cargo build` succeeds with no warnings
- [ ] `cargo test --workspace --test unit` passes (all unit tests)
- [ ] `cargo test --workspace --test integration` passes (all integration tests)
- [ ] `cargo test -p tokenoverflow --test e2e` passes (all E2E tests)
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No references to `services` or `repository` remain in `src/` (checked
      via `grep -r "mod services" apps/api/src/` returning zero results)
- [ ] `git diff --stat` confirms only renames and import path changes

## Documentation Changes

### New file: `apps/api/src/README.md`

A short README inside the API source directory explaining the layer structure
for newcomers. Content:

```markdown
# API Source Code Structure

## Layer Overview

The API code is organized into layers that separate concerns:

    HTTP Request
        |
        v
    routes/       Handles HTTP: parses requests, validates input, returns responses.
        |
        v
    ops/          Business logic: orchestrates operations, calls external APIs,
                  applies rules. Does NOT know about databases.
        |
        v
    store/        Data access: reads/writes the database. Each file has a trait
                  (the contract) and a pg_* implementation (the PostgreSQL code).
        |
        v
    PostgreSQL

## Why two layers (ops + store)?

Testability. Unit tests for business logic use mock implementations of the
store traits, so they run in milliseconds without a database. Integration tests
use the real PostgreSQL implementations.

## Other directories

| Directory    | Purpose                                          |
|------------- |--------------------------------------------------|
| `api/`       | HTTP server setup, route configuration, types    |
| `db/`        | Connection pool, Diesel schema, ORM model structs|
| `external/`  | Third-party API clients (e.g., embedding service)|
| `mcp/`       | MCP (Model Context Protocol) server and tools    |
| `ops/`       | Business logic operations                        |
| `store/`     | Database access traits and implementations       |
```

### Updates to existing files

No changes needed to the root `README.md` since it does not reference internal
source directory names.

## Development Environment Changes

None. This change does not affect:
- Brewfile
- Docker Compose
- Environment variables
- Configuration files
- Build scripts
- CI/CD pipelines

## Tasks

### Task 1: Create `ops/` and `store/` directories, move files

**Scope:** File moves only, no content changes yet.

**Steps:**
1. `git mv apps/api/src/services/answer.rs apps/api/src/ops/answer.rs`
2. `git mv apps/api/src/services/question.rs apps/api/src/ops/question.rs`
3. `git mv apps/api/src/services/search.rs apps/api/src/ops/search.rs`
4. `git mv apps/api/src/services/tags.rs apps/api/src/ops/tags.rs`
5. `git mv apps/api/src/services/tag_resolver.rs apps/api/src/ops/tag_resolver.rs`
6. Create `apps/api/src/ops/mod.rs` with re-exports (no `repository` sub-module)
7. `git mv apps/api/src/services/repository/answer.rs apps/api/src/store/answer.rs`
8. `git mv apps/api/src/services/repository/pg_answer.rs apps/api/src/store/pg_answer.rs`
9. `git mv apps/api/src/services/repository/question.rs apps/api/src/store/question.rs`
10. `git mv apps/api/src/services/repository/pg_question.rs apps/api/src/store/pg_question.rs`
11. `git mv apps/api/src/services/repository/search.rs apps/api/src/store/search.rs`
12. `git mv apps/api/src/services/repository/pg_search.rs apps/api/src/store/pg_search.rs`
13. `git mv apps/api/src/services/repository/tag.rs apps/api/src/store/tag.rs`
14. `git mv apps/api/src/services/repository/pg_tag.rs apps/api/src/store/pg_tag.rs`
15. Create `apps/api/src/store/mod.rs` with re-exports
16. Remove `apps/api/src/services/` directory

**Success criteria:** All files exist in the new locations. The old `services/`
directory is gone.

### Task 2: Update all import paths in production code

**Scope:** Change `use crate::services::` to `use crate::ops::` and
`use crate::services::repository::` to `use crate::store::` across all source
files.

**Steps:**
1. Update `src/lib.rs`: replace `pub mod services` with `pub mod ops` +
   `pub mod store`
2. Update `src/ops/*.rs`: change `super::repository::*` to `crate::store::*`
3. Update `src/api/state.rs`
4. Update `src/api/server.rs`
5. Update `src/api/routes/questions.rs`
6. Update `src/api/routes/answers.rs`
7. Update `src/api/routes/search.rs`
8. Check and update any `src/mcp/tools/*.rs` files with direct service imports

**Success criteria:** `cargo build` succeeds with no errors.

### Task 3: Update all import paths in test code

**Scope:** Rename test directories and update imports.

**Steps:**
1. Rename `tests/unit/services/` to `tests/unit/ops/`
2. Rename `tests/integration/repositories/` to `tests/integration/store/`
3. Update `tests/unit/mod.rs` to reference `ops` instead of `services`
4. Update `tests/integration/mod.rs` to reference `store` instead of
   `repositories`
5. Update `tests/common/mock_repository.rs` imports
6. Update all individual test files with new import paths

**Success criteria:** `cargo test --workspace` passes with zero failures.

### Task 4: Add `apps/api/src/README.md`

**Scope:** Create the README documented in the Documentation Changes section.

**Success criteria:** File exists and accurately describes the layer structure.

### Task 5: Final verification

**Scope:** Run the full validation suite.

**Steps:**
1. `cargo build`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace --test unit`
4. `cargo test --workspace --test integration`
5. `cargo test -p tokenoverflow --test e2e`
6. Verify no remaining references: `grep -r "mod services" apps/api/src/`
   returns nothing
7. Verify no remaining references: `grep -r "services::repository" apps/api/`
   returns nothing

**Success criteria:** All commands pass. Zero references to old names remain.

---

## Implementation Notes (2026-02-20)

The initial implementation focused on the repository directory reorganization
only, as a prerequisite for the broader `ops/store` rename. The full rename is
deferred to a follow-up.

### What was implemented

The flat `services/repository/` directory was reorganized into two
subdirectories:

```text
BEFORE                              AFTER
------                              -----
repository/                         repository/
  mod.rs                              mod.rs
  answer.rs      (trait)              interface/
  pg_answer.rs   (impl)                mod.rs
  question.rs    (trait)                answer.rs
  pg_question.rs (impl)                question.rs
  search.rs      (trait)                search.rs
  pg_search.rs   (impl)                tag.rs
  tag.rs         (trait)              postgres/
  pg_tag.rs      (impl)                mod.rs
                                       answer.rs
                                       question.rs
                                       search.rs
                                       tag.rs
```

Key decisions:
- **`interface/`** holds trait definitions (the contracts).
- **`postgres/`** holds PostgreSQL implementations (the `pg_` prefix was dropped
  from filenames since the directory name already conveys the backend).
- **`repository/mod.rs`** re-exports all types from both subdirectories so the
  public API (`crate::services::repository::*`) remains identical.
- No external import paths changed. All consumers (services, tests, server.rs,
  state.rs) continue to use `crate::services::repository::{...}` as before.
- `apps/api/src/README.md` was added explaining the layer structure.
