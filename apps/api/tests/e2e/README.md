# E2E Tests

E2E tests require the **full Docker stack** to be running.

## What Goes Here

- End-to-end flow tests
- Tests that hit the actual API via HTTP
- Tests verifying E2E service interactions

## What Does NOT Go Here

- Unit tests for individual components
- Tests that can run without Docker

## Running

```bash
# Start all services
source scripts/src/includes.sh
redeploy_local

# Run integration tests
cargo test -p tokenoverflow --test e2e
```

## CI

The api e2e suite is exercised in CI via
[`.github/workflows/e2e_test.yml`](../../../../.github/workflows/e2e_test.yml).
Locally, run `cargo test --test e2e -- --test-threads=1` against an up `api`
profile stack.
