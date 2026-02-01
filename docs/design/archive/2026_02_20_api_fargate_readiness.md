# Design: api-fargate-readiness

## Architecture Overview

### Goal

Make the TokenOverflow API application production-ready for AWS Fargate
(ARM64/Graviton). The current codebase has five gaps that must be addressed
before the container can run reliably in a Fargate environment:

1. No graceful shutdown -- the process ignores SIGTERM, causing in-flight
   requests to be dropped during deployments and Spot interruptions.
2. Health check always returns HTTP 200 -- even when the database is
   unreachable, misleading the ECS health checker.
3. Docker image not optimized for ARM64 -- no explicit platform pinning,
   includes unnecessary packages, and the binary is not stripped.
4. No request timeout -- a slow or stuck request can hold a connection
   indefinitely.
5. No retry logic on the Voyage AI embedding client -- transient network
   errors and rate limits cause immediate failures.

### Scope

**In scope:** Application-level changes only (Rust source code, Dockerfile,
and configuration TOML files).

**Out of scope:** Terraform/ECS infrastructure (covered in a separate design),
PgBouncer/connection pooling changes, CI/CD pipeline, ALB configuration.

### Current State

The API server starts, binds to a TCP port, and serves requests until the
process is killed. There is no signal handling, no timeout middleware, and the
embedding client makes exactly one HTTP attempt per request. The Dockerfile
produces a working image but is not optimized for the target deployment
environment.

**`apps/api/src/api/server.rs` (current):**

```rust
let listener = TcpListener::bind(&bind_addr).await?;
axum::serve(listener, app).await?;
```

No `.with_graceful_shutdown()`. On SIGTERM, the process exits immediately.

**`apps/api/src/api/routes/health.rs` (current):**

```rust
pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let db_status = match check_db(&state).await {
        Ok(()) => "connected".to_string(),
        Err(e) => e,
    };
    Json(HealthResponse {
        status: "ok".to_string(),
        database: db_status,
    })
}
```

Always returns 200 with `"status": "ok"`, even when `check_db` fails. ECS
health checks and ALB target group health checks rely on the status code to
determine container health.

**`apps/api/src/external/embedding/service.rs` (current):**

```rust
let response = self.client.post(&url)
    .bearer_auth(&self.api_key)
    .json(&body)
    .send()
    .await
    .map_err(|e| EmbeddingError::Network(e.to_string()))?;
```

Single attempt. A transient 429 or 503 from Voyage AI fails the entire
embedding request.

**`apps/api/Dockerfile` (current):**

```dockerfile
FROM rust:1.93-slim AS chef
# ...
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 libpq5 curl \
    && rm -rf /var/lib/apt/lists/*
# ...
HEALTHCHECK --interval=10s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1
```

No `--platform` pinning. Includes `curl` (only used by `HEALTHCHECK`, which
Fargate ignores). Binary is not stripped. `HEALTHCHECK` instruction is
redundant when running on Fargate (ECS uses its own health check definition).

### Proposed State

After this design is implemented:

- The server handles SIGTERM gracefully, draining in-flight requests before
  shutting down.
- The health endpoint returns 503 when the database is unreachable.
- The Docker image is pinned to `linux/arm64`, stripped, and free of
  unnecessary packages.
- Every request has a configurable timeout (default 30 seconds).
- The embedding client retries transient failures with exponential backoff.

### New Dependencies

| Crate                | Version | Purpose                                            |
|----------------------|---------|----------------------------------------------------|
| `reqwest`            | 0.13    | Upgraded from 0.12 (required by reqwest-middleware) |
| `reqwest-middleware`  | 0.5     | Middleware layer for reqwest client                 |
| `reqwest-retry`      | 0.9     | Exponential backoff retry on transient HTTP errors  |

The existing `tower` dependency already has the `timeout` feature enabled,
and `tokio::signal` is available through the existing `tokio` dependency
with `features = ["full"]`.

---

## Interfaces

This section documents every file that will be created or modified, and the
contracts between them.

### Modified Files

| File                                                   | Change Summary                                                                                                   |
|--------------------------------------------------------|------------------------------------------------------------------------------------------------------------------|
| `apps/api/src/api/server.rs`                           | Add `shutdown_signal()`, extract `serve_until_shutdown()`, wire `with_graceful_shutdown()`, apply `TimeoutLayer` |
| `apps/api/src/api/routes/health.rs`                    | Return `(StatusCode, Json<HealthResponse>)`, 503 on DB failure                                                   |
| `apps/api/src/config.rs`                               | Add `request_timeout_secs: u64` to `ApiConfig`                                                                   |
| `apps/api/Cargo.toml`                                  | Upgrade reqwest 0.12→0.13, add `reqwest-middleware` + `reqwest-retry`                                            |
| `apps/embedding-service/Cargo.toml`                    | Upgrade reqwest 0.12→0.13 in dev-dependencies                                                                   |
| `apps/api/src/external/embedding/client.rs`            | Use `ClientWithMiddleware`, wire `reqwest-retry` policy                                                          |
| `apps/api/src/external/embedding/service.rs`           | Remove manual retry loop, simplify `embed()` (retries handled by middleware)                                     |
| `apps/api/Dockerfile`                                  | ARM64 platform, remove curl/HEALTHCHECK, strip binary                                                            |
| `apps/api/config/production.toml`                      | Add `request_timeout_secs = 30`                                                                                  |
| `apps/api/config/development.toml`                     | Add `request_timeout_secs = 30`                                                                                  |
| `apps/api/config/local.toml`                           | Add `request_timeout_secs = 30`                                                                                  |
| `apps/api/config/unit_test.toml`                       | Add `request_timeout_secs = 30`                                                                                  |
| `apps/api/tests/unit/api/routes/test_health.rs`        | Update assertions for 503 status code                                                                            |
| `apps/api/tests/integration/api/routes/test_health.rs` | Add status code assertion                                                                                        |

### New Files

| File                                                       | Purpose                                         |
|------------------------------------------------------------|-------------------------------------------------|
| `apps/api/tests/integration/api/test_graceful_shutdown.rs` | Integration test for graceful shutdown behavior |

### Files NOT Modified

| File                                   | Reason                                                                                          |
|----------------------------------------|-------------------------------------------------------------------------------------------------|
| `docker-compose.yml`                   | Build context and Dockerfile paths are unchanged.                                               |
| `apps/api/src/api/routes/configure.rs` | Route wiring is unchanged; the timeout is applied at the router level in `server.rs`.           |
| `infra/terraform/**`                   | Infrastructure is out of scope.                                                                 |

### Contract Changes

**Health endpoint response contract:**

| Condition            | Before                                       | After                                                    |
|----------------------|----------------------------------------------|----------------------------------------------------------|
| Database reachable   | 200 `{"status":"ok","database":"connected"}` | 200 `{"status":"ok","database":"connected"}` (unchanged) |
| Database unreachable | 200 `{"status":"ok","database":"<error>"}`   | 503 `{"status":"degraded","database":"<error>"}`         |

The 503 response is required for ECS health checks. When ECS receives a
non-2xx response, it marks the container as unhealthy after the configured
number of retries. The existing ECS task definition (from the infrastructure
design) uses `curl -f http://localhost:8080/health`, where `curl -f` treats
any HTTP response >= 400 as a failure.

---

## Logic

### Change 1: Graceful Shutdown (SIGTERM Handling)

**File:** `apps/api/src/api/server.rs`

Fargate sends SIGTERM when stopping a task (deployments, scaling, Spot
reclamation). The application has a configurable stop timeout (default 30
seconds in ECS) to finish in-flight requests before SIGKILL is sent. Without
graceful shutdown, all in-flight requests are dropped immediately.

**Implementation:**

Add a `shutdown_signal()` async function that resolves when SIGTERM (Unix) or
Ctrl+C is received. Extract the server startup into `serve_until_shutdown()`
to accept a generic shutdown future for testability.

```rust
use std::future::Future;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tracing::info;

// ... existing imports ...

// Tokio runtime bootstrap -- needs a running server to exercise.
// E2E: tests/e2e/api/ exercises the full server via Docker Compose.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_run())
}

// Server startup: config loading, TCP bind, and service wiring.
// E2E: tests/e2e/api/ exercises the full server via Docker Compose.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn async_run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    let app_state = create_app_state(&config).await?;

    let mcp_app_state = app_state.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(TokenOverflowServer::new(mcp_app_state.clone())),
        Arc::new(LocalSessionManager::default()),
        Default::default(),
    );

    let timeout = std::time::Duration::from_secs(config.api.request_timeout_secs);

    let bind_addr = format!("{}:{}", config.api.host, config.api.port);

    info!("Starting server on {}", bind_addr);

    let app = routes::configure()
        .nest_service("/mcp", mcp_service)
        .with_state(app_state)
        .layer(ServiceBuilder::new().layer(TimeoutLayer::new(timeout)));

    let listener = TcpListener::bind(&bind_addr).await?;

    serve_until_shutdown(listener, app, shutdown_signal()).await
}

/// Run the server until the provided shutdown signal completes.
///
/// Separated from `async_run` so that tests can provide their own
/// shutdown trigger (e.g., a cancellation token) without needing real
/// OS signals.
pub async fn serve_until_shutdown(
    listener: TcpListener,
    app: axum::Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Wait for a shutdown signal (SIGTERM on Unix, Ctrl+C everywhere).
///
/// This function is not testable in unit/integration tests because it
/// blocks on OS signals. The graceful shutdown behavior is tested via
/// `serve_until_shutdown` with a synthetic shutdown future.
// OS signal listener -- cannot be exercised without sending real signals.
// E2E: tests/e2e/api/ exercises the full server via Docker Compose.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown");
        }
        () = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown");
        }
    }
}
```

**Design decisions:**

| Decision                    | Chosen                                                                     | Alternative                      | Reason                                                                                            |
|-----------------------------|----------------------------------------------------------------------------|----------------------------------|---------------------------------------------------------------------------------------------------|
| Where to listen for signals | Dedicated `shutdown_signal()` fn                                           | Inline in `async_run`            | Separation keeps `async_run` focused on wiring; `shutdown_signal` is independently readable       |
| Testability approach        | Extract `serve_until_shutdown()` with generic future                       | Mock `tokio::signal`             | Generic future is simpler; no mocking framework needed; tests pass a `tokio::sync::oneshot`       |
| Coverage annotation         | `#[cfg_attr(coverage_nightly, coverage(off))]` on `shutdown_signal()` only | Also on `serve_until_shutdown()` | `serve_until_shutdown` is testable and should be covered; only the OS signal listener is excluded |

### Change 2: Health Check Status Codes

**File:** `apps/api/src/api/routes/health.rs`

The health endpoint must return an appropriate HTTP status code so that ECS
container health checks and ALB target group health checks can distinguish
between a healthy and degraded container.

**Implementation:**

```rust
use axum::Json;
use axum::extract::State;
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::api::state::AppState;

#[derive(Serialize, Deserialize, Debug)]
pub struct HealthResponse {
    pub status: String,
    pub database: String,
}

/// Health check endpoint
///
/// Returns 200 with `{"status":"ok","database":"connected"}` when the
/// database is reachable. Returns 503 with `{"status":"degraded","database":"<error>"}`
/// when the database connection fails.
pub async fn health_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<HealthResponse>) {
    match check_db(&state).await {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok".to_string(),
                database: "connected".to_string(),
            }),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "degraded".to_string(),
                database: e,
            }),
        ),
    }
}

async fn check_db(state: &AppState) -> Result<(), String> {
    // Getting a connection from the pool validates DB connectivity
    // (bb8 runs connection health checks internally)
    let _conn = state.pool.get().await.map_err(|e| e.to_string())?;
    Ok(())
}
```

**Key changes:**

- Return type changed from `Json<HealthResponse>` to
  `(StatusCode, Json<HealthResponse>)`.
- On DB failure: status code is 503 (Service Unavailable), `status` field
  is `"degraded"` instead of `"ok"`.
- On DB success: behavior is identical to before (200, `"ok"`,
  `"connected"`).

**Design decisions:**

| Decision                      | Chosen                  | Alternative                | Reason                                                                                                                                                                                                                                             |
|-------------------------------|-------------------------|----------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Failure status code           | 503 Service Unavailable | 500 Internal Server Error  | 503 is semantically correct: the service itself is running but cannot serve requests because a dependency is down. ECS and ALB both treat 503 as unhealthy.                                                                                        |
| Status field value on failure | `"degraded"`            | `"error"` or `"unhealthy"` | `"degraded"` is more precise: the API process is running (not errored), but it cannot serve full functionality. Aligns with industry health check conventions.                                                                                     |
| Check only database           | Yes                     | Also check embedding API   | The embedding API is external and not every request needs it. A failed embedding API should not mark the entire container as unhealthy -- it would cause unnecessary container restarts. Database connectivity is the minimum viability threshold. |

### Change 3: Docker Image (ARM64)

**File:** `apps/api/Dockerfile`

The Fargate task definition specifies `ARM64` architecture. The Dockerfile must
produce an image that is explicitly built for `linux/arm64`. Additionally, the
image should be stripped of unnecessary packages and the binary should be
stripped to reduce image size.

**Implementation (full updated Dockerfile):**

```dockerfile
# =============================================================================
# Stage 1: Chef - Base image with cargo-chef installed
# =============================================================================
FROM --platform=linux/arm64 rust:1.93-slim AS chef
WORKDIR /app

# Install build dependencies needed for cargo-chef and compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef

# =============================================================================
# Stage 2: Planner - Generate dependency recipe
# =============================================================================
FROM chef AS planner

# Copy workspace manifest, lockfile, and only this service's source
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/api ./apps/api

# Replace workspace manifest with single-member workspace
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/api"]\n' > Cargo.toml

# Generate recipe.json for dependency caching
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Stage 3: Builder - Compile dependencies then application
# =============================================================================
FROM chef AS builder

# Copy recipe and build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy workspace manifest, lockfile, and only this service's source
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/api ./apps/api

# Replace workspace manifest with single-member workspace
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/api"]\n' > Cargo.toml

RUN cargo build --release -p tokenoverflow

# Strip debug symbols from the binary to reduce image size
RUN strip /app/target/release/tokenoverflow

# =============================================================================
# Stage 4: Runtime - Minimal production image
# =============================================================================
FROM --platform=linux/arm64 debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/tokenoverflow /app/tokenoverflow
COPY apps/api/config /app/config

RUN useradd -r -u 1001 appuser && chown -R appuser:appuser /app
USER appuser

EXPOSE 8080

CMD ["/app/tokenoverflow"]
```

**Changes from current file:**

1. Added `--platform=linux/arm64` to the `chef` and `runtime` `FROM`
   statements. The `planner` and `builder` stages inherit from `chef`, so
   they do not need their own `--platform` directive.
2. Added `RUN strip /app/target/release/tokenoverflow` after the build step
   in the builder stage.
3. Removed `curl` from the runtime `apt-get install` list.
4. Removed the entire `HEALTHCHECK` instruction.

**Design decisions:**

| Decision           | Chosen                                           | Alternative                        | Reason                                                                                                                                            |
|--------------------|--------------------------------------------------|------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------|
| Base image         | `debian:bookworm-slim`                           | `distroless`, Alpine, Chainguard   | See alternatives analysis below                                                                                                                   |
| Platform pinning   | `--platform=linux/arm64` on `chef` and `runtime` | `--platform` on every `FROM`       | Intermediate stages inherit from `chef`, so only the base stages need pinning                                                                     |
| Binary stripping   | `RUN strip` in builder stage                     | `cargo build` profile strip option | `RUN strip` is explicit and visible in the Dockerfile; profile-level strip requires Cargo.toml changes that affect local builds                   |
| Remove curl        | Yes                                              | Keep for debugging                 | curl was only used by the `HEALTHCHECK` instruction. For debugging in production, `docker exec` or ECS Exec can install tools temporarily         |
| Remove HEALTHCHECK | Yes                                              | Keep it                            | Fargate ignores Dockerfile `HEALTHCHECK` instructions entirely. ECS defines its own health check in the task definition. Keeping it is misleading |

**Alternatives analysis for base image:**

| Option                                         | Pros                                                                                                                         | Cons                                                                                                                                                                                                                           |
|------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **debian:bookworm-slim (chosen)**              | Straightforward `apt-get` for `libpq5` and its 8 transitive dependencies. Same base as the build stage. Well-tested, stable. | Larger than distroless (~80 MB vs ~30 MB). Includes a shell (minor security surface).                                                                                                                                          |
| **distroless (gcr.io/distroless/cc-debian12)** | Smallest image. No shell (smaller attack surface).                                                                           | Requires manually copying `libpq5` and all 8 transitive `.so` files from the builder stage. Fragile: any `libpq` dependency change (e.g., a Debian point release) silently breaks the image. No package manager for debugging. |
| **Alpine / musl**                              | Small image (~5 MB base). Single static binary possible.                                                                     | musl's allocator is up to 6x slower than glibc's for multi-threaded workloads. Requires `jemalloc` or `mimalloc` workaround. `libpq` must be compiled against musl or use a static build.                                      |
| **Chainguard**                                 | Minimal, hardened images. Good security posture.                                                                             | Same `.so` copying problem as distroless. Risk of ABI mismatch between Chainguard's libc and Debian-compiled `libpq`. Smaller community; fewer debugging resources.                                                            |

### Change 4: Request Timeout Middleware

**Files:** `apps/api/src/api/server.rs`, `apps/api/src/config.rs`, all config
TOMLs

A request that takes too long (e.g., a slow database query or a stuck
embedding call) should not hold a connection indefinitely. A timeout layer
on the router ensures that every request has a hard upper bound.

**Config change (`apps/api/src/config.rs`):**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
}

fn default_request_timeout_secs() -> u64 {
    30
}
```

The `#[serde(default = "default_request_timeout_secs")]` ensures backward
compatibility: if an older config file does not include `request_timeout_secs`,
it defaults to 30 seconds.

**TOML changes (all 4 config files):**

Add under the `[api]` section:

```toml
request_timeout_secs = 30
```

**Server wiring (shown in Change 1 above):**

```rust
let timeout = std::time::Duration::from_secs(config.api.request_timeout_secs);

let app = routes::configure()
    .nest_service("/mcp", mcp_service)
    .with_state(app_state)
    .layer(ServiceBuilder::new().layer(TimeoutLayer::new(timeout)));
```

The `TimeoutLayer` wraps the entire router (including the MCP service).
When a request exceeds the timeout, Tower returns a 408 Request Timeout
response automatically.

**Design decisions:**

| Decision       | Chosen                                | Alternative               | Reason                                                                                                                                                                                                          |
|----------------|---------------------------------------|---------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Timeout scope  | Entire router (all routes)            | Per-route timeout         | A global default is simpler and catches all routes. Per-route overrides can be added later if specific routes need different timeouts.                                                                          |
| Default value  | 30 seconds                            | 10s, 60s                  | 30s balances between allowing legitimate slow operations (embedding generation, complex searches) and preventing stuck connections. Voyage AI's p99 latency is under 5 seconds; 30s provides a generous buffer. |
| Configuration  | `request_timeout_secs` in `ApiConfig` | Hardcoded                 | Configurable per environment allows production to use a tighter timeout than development if needed.                                                                                                             |
| Layer position | Outermost layer on the router         | Inside the route handlers | Outermost ensures even middleware processing is bounded. Putting it inside handlers would not protect against slow middleware.                                                                                  |

### Change 5: Voyage API Retry with `reqwest-retry`

**Files:** `apps/api/src/external/embedding/client.rs`,
`apps/api/src/external/embedding/service.rs`

The Voyage AI embedding API can return transient errors: HTTP 429 (rate
limited), 5xx (server errors), and network-level failures. The
`reqwest-retry` crate provides battle-tested HTTP-aware retry middleware
that attaches to the reqwest client. Retry logic lives at the HTTP layer,
so `embed()` has zero retry code.

**Client setup (`client.rs`):**

```rust
use std::time::Duration;

use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

let retry_policy = ExponentialBackoff::builder()
    .retry_bounds(Duration::from_secs(2), Duration::from_secs(8))
    .build_with_max_retries(2); // 2 retries = 3 total attempts

let client = ClientBuilder::new(reqwest::Client::new())
    .with(RetryTransientMiddleware::new_with_policy(retry_policy))
    .build();
```

The stored client type changes from `reqwest::Client` to
`reqwest_middleware::ClientWithMiddleware`.

**Simplified `embed()` (`service.rs`):**

```rust
#[async_trait]
#[cfg_attr(coverage_nightly, coverage(off))]
impl EmbeddingService for VoyageClient {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let url = format!("{}/embeddings", self.base_url);

        let body = VoyageRequest { /* ... */ };

        let response = self.client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| EmbeddingError::Network(e.to_string()))?;

        // ... handle success/error responses (no retry logic) ...
    }
}
```

Key simplifications vs. the manual loop:
- No `loop`, `attempt`, or `max_attempts`
- No `tokio::time::sleep` or backoff calculation
- No conditional retry logic for 429/5xx vs 4xx
- `reqwest-retry` logs retries via `tracing::warn` automatically

**Retry behavior:**

| Condition                                        | Retry? | Backoff               |
|--------------------------------------------------|--------|-----------------------|
| Network error (DNS, connection refused, timeout) | Yes    | 2s min, 8s max, exp   |
| HTTP 429 Too Many Requests                       | Yes    | 2s min, 8s max, exp   |
| HTTP 408 Request Timeout                         | Yes    | 2s min, 8s max, exp   |
| HTTP 500, 502, 503, 504                          | Yes    | 2s min, 8s max, exp   |
| HTTP 400 Bad Request                             | No     | --                    |
| HTTP 401 Unauthorized                            | No     | --                    |
| HTTP 403 Forbidden                               | No     | --                    |
| HTTP 404 Not Found                               | No     | --                    |
| Successful response but empty data               | No     | --                    |
| Successful response but JSON parse error         | No     | --                    |

The maximum total attempts is 3 (1 initial + 2 retries). Combined with the
30s request timeout, a retrying embedding call will always complete within
the timeout window.

**Design decisions:**

| Decision                       | Chosen                                                       | Alternative                       | Reason                                                                                                                                                                        |
|--------------------------------|--------------------------------------------------------------|-----------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Retry library                  | `reqwest-retry` (HTTP middleware)                            | `backon`, `tower::retry`, inline  | HTTP-aware; zero retry code in `embed()`; default strategy handles 429, 5xx, network errors. `backon` wraps closures (ugly); `tower::retry` needs Service boilerplate.       |
| Max attempts                   | 3 (1 + 2 retries)                                           | 5, configurable                   | 3 matches the previous manual loop. More attempts would exceed the request timeout.                                                                                           |
| Backoff bounds                 | 2s min, 8s max, exponential                                 | Fixed 1s delay, jitter            | Exponential backoff is the industry standard for rate-limited APIs. Jitter could be added later if thundering herd becomes a concern.                                         |
| Coverage annotation            | Keep existing `#[cfg_attr(coverage_nightly, coverage(off))]` | Remove it                         | The `impl EmbeddingService for VoyageClient` block requires a live API and is already excluded from coverage. Tests use `MockEmbeddingService`.                               |
| reqwest version                | 0.13                                                         | Stay on 0.12                      | `reqwest-retry 0.9` + `reqwest-middleware 0.5` require reqwest 0.13. The upgrade is low-risk: all API methods used are stable across versions.                                |

---

## Edge Cases & Constraints

### 1. SIGTERM during database transaction

**Risk:** If SIGTERM arrives while a request is mid-transaction (e.g.,
inserting a question and its answer), the graceful shutdown waits for
in-flight requests to complete. If the request finishes within the ECS stop
timeout (default 30s), the transaction commits normally. If it does not finish
in time, ECS sends SIGKILL, the process dies, and the database rolls back the
uncommitted transaction. No data corruption occurs.

**Mitigation:** None needed beyond the graceful shutdown implementation.
PostgreSQL automatically rolls back uncommitted transactions when the client
connection drops.

### 2. Request timeout vs. embedding retry

**Risk:** The embedding retry loop can take up to 6 seconds (2s + 4s backoff).
If the request timeout is set lower than this, retries will be cut short by
the timeout layer.

**Mitigation:** The default request timeout is 30 seconds, which is well above
the 6-second maximum retry window. If a shorter timeout is configured, the
timeout layer will cancel the request (including the retry loop), returning
408 to the client. This is correct behavior -- the client should not wait
longer than the configured timeout.

### 3. Health check during startup

**Risk:** The health endpoint returns 503 before the database connection pool
is initialized. ECS health checks begin after the `startPeriod` (60 seconds
in the ECS task definition). If the pool takes longer than 60 seconds to
initialize, the container will be marked unhealthy and restarted.

**Mitigation:** The connection pool is initialized synchronously during
`create_app_state()` before the server starts listening. The health endpoint
is only reachable after the server binds to the port, at which point the pool
is already initialized. The 60-second `startPeriod` in the ECS task definition
provides additional buffer.

### 4. ARM64 cross-compilation on x86 CI/CD

**Risk:** Building a `linux/arm64` image on an x86 host requires QEMU
emulation via `docker buildx`. This is significantly slower than native
compilation (roughly 3-5x).

**Mitigation:** This is expected. For local development, builders with Apple
Silicon (M-series) compile natively without emulation. For CI/CD (future
scope), ARM64 runners are available on GitHub Actions (
`runs-on: ubuntu-24.04-arm`)
and AWS CodeBuild (`ARM_CONTAINER`). Cross-compilation is a one-time cost per
deployment, not a developer workflow concern.

### 5. curl removal breaks local Docker HEALTHCHECK

**Risk:** The current `docker-compose.yml` or local testing workflows might
rely on the Dockerfile's `HEALTHCHECK` instruction to know when the container
is ready. Removing `curl` and `HEALTHCHECK` from the Dockerfile means
`docker compose up` will not wait for the health check.

**Mitigation:** Docker Compose's `depends_on.condition: service_healthy`
requires a `HEALTHCHECK` in the Dockerfile or a `healthcheck` in the Compose
file. If the Compose file already defines a `healthcheck` override for the
API service, removing the Dockerfile `HEALTHCHECK` has no effect. If it does
not, one should be added to `docker-compose.yml`. Either way, the Dockerfile
should not include `curl` in the runtime image just for a health check that
Fargate ignores. The `docker-compose.yml` health check can use
`wget --spider` (available in bookworm-slim without additional packages) or
the Compose file can define its own `healthcheck` using `curl` installed on
the host.

**Update needed:** Check `docker-compose.yml` and, if necessary, add a
`healthcheck` entry for the API service that does not depend on `curl` being
inside the container. For example:

```yaml
services:
  api:
    healthcheck:
      test: ["CMD-SHELL", "wget --spider -q http://localhost:8080/health || exit 1"]
      interval: 10s
      timeout: 5s
      start_period: 5s
      retries: 3
```

### 6. TimeoutLayer returns empty 408 response

**Risk:** When `TimeoutLayer` triggers, it returns a bare 408 status code
with no body. Clients (including MCP clients) might not handle this gracefully.

**Mitigation:** This is acceptable for the MVP. The 408 response clearly
indicates the request timed out. Clients can retry. If a JSON error body is
needed in the future, the `TimeoutLayer` can be replaced with a custom
middleware that returns a structured error. This is an enhancement, not a
blocker.

### 7. Retry on idempotent vs. non-idempotent requests

**Risk:** The `embed()` method is called during both search (read-only) and
question creation (write). Retrying during question creation means the
embedding call is retried, but the database write has not happened yet (the
embedding is generated first, then inserted). This is safe because the
embedding call is idempotent -- calling Voyage AI with the same input always
produces the same output.

**Mitigation:** No special handling needed. The embedding API is inherently
idempotent (same input produces same vector). The retry only repeats the
embedding HTTP call, not any database operations.

---

## Test Plan

### Unit Tests

#### Updated: `apps/api/tests/unit/api/routes/test_health.rs`

The existing tests need to be updated to assert on status codes:

```rust
use axum::Router;
use axum::body::Body;
use axum::routing::get;
use http::{Request, StatusCode};
use tokenoverflow::api::routes::health::health_check;
use tower::ServiceExt;

mod common {
    include!("../../../common/mod.rs");
}

#[tokio::test]
async fn health_check_returns_ok_status() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/health", get(health_check))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Mock pool cannot connect -- should return 503
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn health_check_returns_degraded_when_db_unreachable() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route("/health", get(health_check))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: serde_json::Value = serde_json::from_slice(&body).expect("Failed to parse JSON");

    assert_eq!(health["status"], "degraded");
    let db_status = health["database"].as_str().unwrap();
    assert_ne!(
        db_status, "connected",
        "Expected non-connected database status with mock pool, got: {}",
        db_status
    );
}
```

**Changes:** The first test now asserts `SERVICE_UNAVAILABLE` (503) instead
of `is_success()`, because the mock pool cannot establish a real database
connection. The second test verifies the `"degraded"` status field and the
503 status code.

#### Updated: `apps/api/tests/unit/test_config.rs`

Add a test for the new `request_timeout_secs` field:

```rust
#[test]
fn test_request_timeout_secs_loaded() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(config.api.request_timeout_secs, 30);
}
```

### Integration Tests

#### Updated: `apps/api/tests/integration/api/routes/test_health.rs`

Add a status code assertion to the existing test:

```rust
#[tokio::test]
async fn health_check_with_real_database_returns_connected() {
    let db = IntegrationTestDb::new().await;
    let pool = db.pool().clone();

    let tag_repo = Arc::new(PgTagRepository::new(pool.clone()));
    let tag_resolver = Arc::new(
        TagResolver::new(tag_repo.as_ref())
            .await
            .expect("tag resolver init should succeed"),
    );

    let state = AppState::new(
        pool.clone(),
        Arc::new(StubEmbedding),
        Arc::new(PgQuestionRepository::new(pool.clone())),
        Arc::new(PgAnswerRepository::new(pool.clone())),
        Arc::new(PgSearchRepository::new(pool)),
        tag_repo,
        tag_resolver,
    );

    let app: Router = Router::new()
        .route("/health", get(health_check))
        .with_state(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: HealthResponse = serde_json::from_slice(&body).expect("valid JSON");

    assert_eq!(health.status, "ok");
    assert_eq!(health.database, "connected");
}
```

**Change:** Added `assert_eq!(response.status(), StatusCode::OK)` and the
`StatusCode` import.

#### New: `apps/api/tests/integration/api/test_graceful_shutdown.rs`

```rust
use std::time::Duration;
use tokio::net::TcpListener;

use tokenoverflow::api::server::serve_until_shutdown;

/// Verify that the server shuts down cleanly when the shutdown signal fires.
#[tokio::test]
async fn server_shuts_down_on_signal() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let server_handle = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/health",
            axum::routing::get(|| async { "ok" }),
        );
        serve_until_shutdown(listener, app, async { rx.await.ok(); })
            .await
            .expect("Server should shut down cleanly");
    });

    // Verify the server is accepting connections
    let client = reqwest::Client::new();
    let url = format!("http://{}/health", addr);

    // Wait briefly for the server to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    let response = client.get(&url).send().await.expect("Request should succeed");
    assert!(response.status().is_success());

    // Trigger shutdown
    tx.send(()).expect("Shutdown signal should send");

    // Server should complete within a reasonable time
    tokio::time::timeout(Duration::from_secs(5), server_handle)
        .await
        .expect("Server should shut down within 5 seconds")
        .expect("Server task should not panic");
}
```

**Module registration:** Add `mod test_graceful_shutdown;` to
`apps/api/tests/integration/api/mod.rs`.

### E2E Tests

No new E2E tests are needed. The existing E2E health check test at
`apps/api/tests/e2e/api/routes/test_health.rs` already asserts:

```rust
assert_eq!(response.status(), StatusCode::OK);
let body: Value = response.json();
assert_eq!(body["status"], "ok");
assert_eq!(body["database"], "connected");
```

This test runs against the full Docker Compose stack where the database is
available, so it verifies the 200 / `"ok"` path. The 503 path is covered by
unit tests (mock pool, no real DB).

### Verification Commands

| Step | Command                                                                      | Expected Result                                                                         |
|------|------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------|
| 1    | `cargo test -p tokenoverflow --test unit`                                    | All tests pass, including updated health check assertions and new config test           |
| 2    | `cargo test -p tokenoverflow --test integration`                             | All tests pass, including health check status code assertion and graceful shutdown test |
| 3    | `docker compose build api`                                                   | ARM64 Dockerfile builds successfully                                                    |
| 4    | `docker compose up -d --build api && cargo test -p tokenoverflow --test e2e` | E2E tests pass against the Docker stack                                                 |

---

## Documentation Changes

### Files to Update

| File        | Change                                                                                                                                                           |
|-------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `README.md` | No changes needed. The health endpoint response format documented in the README (`{"status":"ok","database":"connected"}`) remains correct for the healthy case. |

### Files NOT Updated

Historical design documents are not updated. They are a snapshot of the
codebase at the time they were written.

---

## Development Environment Changes

### Brewfile

No changes needed. No new tools or system dependencies are required.

### Environment Variables

No new environment variables. The `request_timeout_secs` configuration is
loaded from TOML files, not from environment variables. (It can still be
overridden via `TOKENOVERFLOW__API__REQUEST_TIMEOUT_SECS` using the existing
env var override mechanism, but no explicit env var is introduced.)

### Setup Flow

No changes. `source scripts/src/includes.sh && setup` continues to work.

### docker-compose.yml

May need a `healthcheck` entry for the API service if the existing Compose
file relies on the Dockerfile's `HEALTHCHECK` instruction. See Edge Case 5
for details.

---

## Tasks

### Task 1: Health check status codes

**What:** Change the health endpoint to return 503 when the database is
unreachable, and update the response status field from `"ok"` to `"degraded"`.

**Steps:**

1. Read `apps/api/src/api/routes/health.rs`.
2. Change the return type of `health_check` from `Json<HealthResponse>` to
   `(StatusCode, Json<HealthResponse>)`.
3. Return `(StatusCode::OK, ...)` when `check_db` succeeds.
4. Return `(StatusCode::SERVICE_UNAVAILABLE, ...)` with
   `status: "degraded"` when `check_db` fails.
5. Update `apps/api/tests/unit/api/routes/test_health.rs`:
    - First test: assert `StatusCode::SERVICE_UNAVAILABLE` (mock pool
      cannot connect).
    - Second test: assert `"degraded"` status and 503 code.
6. Update `apps/api/tests/integration/api/routes/test_health.rs`:
    - Add `assert_eq!(response.status(), StatusCode::OK)`.
7. Run `cargo test -p tokenoverflow --test unit` -- health tests pass.
8. Run `cargo test -p tokenoverflow --test integration` -- health test
   passes with status code assertion.

**Success:** Unit tests assert 503 for mock pool. Integration test asserts
200 for real database. E2E test still passes unchanged.

### Task 2: Request timeout middleware

**What:** Add a configurable request timeout to the API server.

**Steps:**

1. Read `apps/api/src/config.rs`.
2. Add `request_timeout_secs: u64` to `ApiConfig` with
   `#[serde(default = "default_request_timeout_secs")]`.
3. Add `fn default_request_timeout_secs() -> u64 { 30 }`.
4. Add `request_timeout_secs = 30` under `[api]` in all 4 config TOML files:
    - `apps/api/config/production.toml`
    - `apps/api/config/development.toml`
    - `apps/api/config/local.toml`
    - `apps/api/config/unit_test.toml`
5. Read `apps/api/src/api/server.rs`.
6. Import `tower::ServiceBuilder` and `tower::timeout::TimeoutLayer`.
7. Create timeout duration from `config.api.request_timeout_secs`.
8. Apply `.layer(ServiceBuilder::new().layer(TimeoutLayer::new(timeout)))` on
   the router after `.with_state()`.
9. Add a config test in `apps/api/tests/unit/test_config.rs` that asserts
   `config.api.request_timeout_secs == 30`.
10. Run `cargo test -p tokenoverflow --test unit` -- config tests pass.

**Success:** All config tests pass with the new field. The server applies
the timeout layer.

### Task 3: Graceful shutdown

**What:** Add SIGTERM/SIGINT handling so the server drains in-flight requests
before exiting.

**Steps:**

1. Read `apps/api/src/api/server.rs`.
2. Add `shutdown_signal()` async fn:
    - Listen for Ctrl+C via `tokio::signal::ctrl_c()`.
    - Listen for SIGTERM via
      `tokio::signal::unix::signal(SignalKind::terminate())`
      on Unix.
    - Use `tokio::select!` to wait for either signal.
    - Log which signal was received.
    - Apply `#[cfg_attr(coverage_nightly, coverage(off))]`.
3. Extract `serve_until_shutdown(listener, app, shutdown)` as a `pub` async fn:
    - Takes a `TcpListener`, `Router`, and a generic `impl Future<Output = ()>`.
    - Calls `axum::serve(listener, app).with_graceful_shutdown(shutdown).await`.
    - Logs shutdown completion.
4. Update `async_run()` to call
   `serve_until_shutdown(listener, app, shutdown_signal()).await`.
5. Create `apps/api/tests/integration/api/test_graceful_shutdown.rs`:
    - Start server with a oneshot channel as the shutdown signal.
    - Send a request to verify the server is running.
    - Fire the oneshot to trigger shutdown.
    - Assert the server task completes without error.
6. Add `mod test_graceful_shutdown;` to
   `apps/api/tests/integration/api/mod.rs`.
7. Run `cargo test -p tokenoverflow --test integration` -- shutdown test
   passes.

**Success:** The server shuts down cleanly when the shutdown signal fires.
In-flight requests complete before the server exits.

### Task 4: Docker image optimization

**What:** Optimize the Dockerfile for ARM64 deployment on Fargate.

**Steps:**

1. Read `apps/api/Dockerfile`.
2. Add `--platform=linux/arm64` to the `chef` stage `FROM` statement.
3. Add `--platform=linux/arm64` to the `runtime` stage `FROM` statement.
4. Add `RUN strip /app/target/release/tokenoverflow` after the `cargo build`
   step in the builder stage.
5. Remove `curl` from the runtime `apt-get install` list.
6. Remove the entire `HEALTHCHECK` instruction.
7. If `docker-compose.yml` relies on the Dockerfile `HEALTHCHECK`, add a
   `healthcheck` entry in the Compose file.
8. Run `docker compose build api` -- builds successfully.
9. Run
   `docker compose up -d --build api && cargo test -p tokenoverflow --test e2e`
   -- E2E tests pass.

**Success:** Docker image builds for ARM64. Image is smaller (no curl, binary
stripped). E2E tests pass against the Docker stack.

### Task 5: Voyage API retry with `reqwest-retry`

**What:** Replace the manual retry loop in the Voyage AI embedding client
with `reqwest-retry` middleware for transient failure handling.

**Steps:**

1. Add `reqwest-middleware = "0.5"` and `reqwest-retry = "0.9"` to
   `apps/api/Cargo.toml` dependencies. Upgrade `reqwest` from 0.12 to 0.13
   in both `[dependencies]` and `[dev-dependencies]`.
2. Upgrade `reqwest` from 0.12 to 0.13 in
   `apps/embedding-service/Cargo.toml` `[dev-dependencies]`.
3. In `apps/api/src/external/embedding/client.rs`:
    - Change `pub(super) client` type from `reqwest::Client` to
      `reqwest_middleware::ClientWithMiddleware`.
    - In `new()`, build the client with `ExponentialBackoff` (2s min, 8s max,
      2 retries) and `RetryTransientMiddleware`.
4. In `apps/api/src/external/embedding/service.rs`:
    - Remove the manual retry loop (`loop`, `attempt`, `max_attempts`,
      `tokio::time::sleep`, backoff calculation).
    - Simplify `embed()` to a single-attempt call — the middleware handles
      retries transparently.
5. Keep the existing `#[cfg_attr(coverage_nightly, coverage(off))]` on the
   `impl` block.
6. Run `cargo build -p tokenoverflow` -- compiles with new deps.
7. Run `cargo test -p tokenoverflow --test unit` -- all tests pass.
8. Run `cargo test -p tokenoverflow --test integration` -- all tests pass.
9. Run
   `docker compose up -d --build api && cargo test -p tokenoverflow --test e2e`
   -- E2E tests pass (embedding service is available in Docker Compose).

**Success:** The embedding client retries transient failures via
`reqwest-retry` middleware. No retry code in `embed()`. Non-retryable errors
fail immediately. The retry behavior completes within the request timeout
window.
