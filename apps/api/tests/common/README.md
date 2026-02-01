# Common Test Utilities

Shared utilities used by both unit and integration tests.

## Contents

- `fixtures.rs` - Request builders (QuestionRequestBuilder, etc.)
- `http_client.rs` - HTTP client for integration tests
- `mock_embedding.rs` - Mock embedding service for unit tests
- `test_db.rs` - pglite-oxide database setup for unit tests

## Usage

```rust
mod common {
    include!("../common/mod.rs");
}
use common::{QuestionRequestBuilder, TestDatabase};
```
