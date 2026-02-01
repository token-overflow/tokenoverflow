# Design: docker-build-isolation

## Architecture Overview

### Goal

Decouple service Dockerfiles so that each service's Docker build only requires
its own source code -- not the source of every other workspace member. Today,
both `apps/api/Dockerfile` and `apps/embedding-service/Dockerfile` copy
sibling services into the build context solely to satisfy the root
`Cargo.toml` workspace manifest. This creates two problems:

1. **Unnecessary coupling.** Adding a new workspace member (e.g.,
   `apps/so-tag-sync`) breaks every existing Dockerfile that does not
   account for it.
2. **Larger Docker build contexts and layers.** The API image copies the
   entire `apps/embedding-service` directory even though there is zero Cargo
   dependency between the two.

### Scope

This design covers **two Dockerfiles only**:

- `apps/api/Dockerfile`
- `apps/embedding-service/Dockerfile`

No application code, no runtime behavior, no API contracts, and no
infrastructure changes. The running containers produce identical binaries.

### Current State

**Root `Cargo.toml`:**

```toml
[workspace]
resolver = "2"
members = [
    "apps/api",
    "apps/embedding-service",
    "apps/so-tag-sync",
]
```

**`apps/api/Dockerfile` (problem lines):**

```dockerfile
# Planner stage
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/api ./apps/api
COPY apps/embedding-service ./apps/embedding-service   # <-- unnecessary

# Builder stage
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/api ./apps/api
COPY apps/embedding-service ./apps/embedding-service   # <-- unnecessary
```

The API has no Cargo dependency on `embedding-service`. The only reason it is
copied is that `cargo build -p tokenoverflow` requires all workspace members
declared in the root `Cargo.toml` to be present. Furthermore,
`apps/so-tag-sync` was recently added to the workspace but is not accounted for
in either Dockerfile -- meaning **both Dockerfiles are currently broken**.

**`apps/embedding-service/Dockerfile` (problem lines):**

```dockerfile
COPY Cargo.toml ./Cargo.toml
COPY apps/api/Cargo.toml ./apps/api/Cargo.toml
COPY apps/embedding-service/Cargo.toml ./apps/embedding-service/Cargo.toml

RUN mkdir -p apps/api/src apps/embedding-service/src
RUN echo "fn main() {}" > apps/api/src/main.rs       # <-- dummy stub
RUN echo "pub fn lib() {}" > apps/api/src/lib.rs      # <-- dummy stub
```

This Dockerfile creates dummy stubs for the API just to satisfy the workspace
manifest, and also does not account for `apps/so-tag-sync`.

### Proposed State

Each Dockerfile generates a **minimal single-member workspace manifest** at
build time using a `RUN` command, replacing the root `Cargo.toml` that was
copied in. The generated manifest declares only the service being built.

**Example for the API Dockerfile (conceptual):**

```dockerfile
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/api ./apps/api

# Replace the workspace manifest with a single-member one
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/api"]\n' > Cargo.toml
```

This approach:

- Copies zero sibling service files.
- Requires no dummy/stub source files.
- Is immune to new workspace members being added.
- Uses the shared `Cargo.lock` as-is (Cargo silently ignores entries for
  packages not in the current workspace).

### Alternatives Considered

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **A. Generate single-member manifest (recommended)** | Each Dockerfile writes a minimal `[workspace]` Cargo.toml at build time. | Zero coupling between services. Adding a member requires zero Dockerfile changes. No stubs. cargo-chef works unchanged. | The generated manifest is not identical to the repo root manifest (intentional). |
| **B. Copy all workspace members** | Each Dockerfile copies every `apps/*/Cargo.toml` and creates stubs for siblings. | No manifest manipulation. | Breaks on every new workspace member. Larger build context. Maintenance burden grows linearly with workspace size. This is the current approach and it is already broken. |
| **C. `.dockerignore` per service** | Use Docker's `.dockerignore` to exclude sibling source but keep their `Cargo.toml`. Still need stubs. | Slightly cleaner than (B). | Still requires stubs. Still breaks when new members are added unless `.dockerignore` is updated. Does not eliminate coupling. |
| **D. Use `cargo build --manifest-path` on the member Cargo.toml** | Skip the workspace entirely by pointing to the member's own Cargo.toml. | No workspace needed in Docker. | Ignores the shared Cargo.lock. Dependency versions may drift from what was tested locally. Incompatible with workspace-level settings. |
| **E. Wait for Cargo's `--ignore-missing-members` flag** | [cargo#14566](https://github.com/rust-lang/cargo/issues/14566) proposes this, but it is still in triage. | No workarounds needed. | Not implemented. Status is "S-needs-info". No timeline. Cannot depend on it. |

**Decision: Approach A.** It is the simplest, most maintainable, and
future-proof solution. A single `printf` or `echo` line in each Dockerfile
replaces dozens of lines of `COPY` and `RUN mkdir/echo` stubs.

---

## Interfaces

This change has no external interfaces. It modifies only internal Docker build
files. The contract between docker-compose.yml and each Dockerfile (context,
dockerfile path, exposed port, healthcheck, CMD) remains identical.

### Dockerfile Build Contract (unchanged)

| Property | API | Embedding Service |
|----------|-----|-------------------|
| Build context | `.` (repo root) | `.` (repo root) |
| Dockerfile path | `apps/api/Dockerfile` | `apps/embedding-service/Dockerfile` |
| Binary produced | `/app/tokenoverflow` | `/app/embedding-service` |
| Exposed port | 8080 | 8080 |
| CMD | `["/app/tokenoverflow"]` | `["/app/embedding-service"]` |

### Files Modified

| File | Change Summary |
|------|----------------|
| `apps/api/Dockerfile` | Remove `COPY apps/embedding-service`. Add `RUN printf` to generate single-member workspace manifest. |
| `apps/embedding-service/Dockerfile` | Remove `COPY apps/api/Cargo.toml`, dummy stubs. Add `RUN printf` to generate single-member workspace manifest. |

### Files NOT Modified

| File | Reason |
|------|--------|
| `Cargo.toml` (root) | The repo-root workspace manifest stays as-is. Only the Docker build uses a generated one. |
| `docker-compose.yml` | Build context and dockerfile paths are unchanged. |
| `Cargo.lock` | Shared lockfile is unchanged. Cargo ignores entries for packages not in the workspace. |
| Any `*.rs` file | No application code changes. |

---

## Logic

### `apps/api/Dockerfile` -- Full Updated File

```dockerfile
# =============================================================================
# Stage 1: Chef - Base image with cargo-chef installed
# =============================================================================
FROM rust:1.93-slim AS chef
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

# =============================================================================
# Stage 4: Runtime - Minimal production image
# =============================================================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/tokenoverflow /app/tokenoverflow
COPY apps/api/config /app/config

RUN useradd -r -u 1001 appuser && chown -R appuser:appuser /app
USER appuser

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/tokenoverflow"]
```

**Changes from current file:**

1. Removed `COPY apps/embedding-service ./apps/embedding-service` from both the
   planner and builder stages (lines that existed in both places).
2. Added `RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/api"]\n' > Cargo.toml`
   after each `COPY Cargo.toml` to replace the multi-member workspace manifest
   with a single-member one.

### `apps/embedding-service/Dockerfile` -- Full Updated File

```dockerfile
# =============================================================================
# Stage 1: Chef - Base image with cargo-chef installed
# =============================================================================
FROM rust:1.93-slim AS chef
WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    g++ \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef

# =============================================================================
# Stage 2: Planner - Generate dependency recipe
# =============================================================================
FROM chef AS planner

# Copy workspace manifest, lockfile, and only this service's source
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/embedding-service ./apps/embedding-service

# Replace workspace manifest with single-member workspace
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/embedding-service"]\n' > Cargo.toml

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
COPY apps/embedding-service ./apps/embedding-service

# Replace workspace manifest with single-member workspace
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/embedding-service"]\n' > Cargo.toml

# Build the binary
RUN cargo build --release -p embedding-service

# =============================================================================
# Stage 4: Runtime - Minimal production image
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary
COPY --from=builder /app/target/release/embedding-service /app/embedding-service

# Create cache directory for model (will be downloaded on first run)
RUN mkdir -p /app/.fastembed_cache

# Create non-root user for security
RUN useradd -r -u 1001 appuser && chown -R appuser:appuser /app
USER appuser

EXPOSE 8080

ENV HOST=0.0.0.0
ENV PORT=8080
ENV RUST_LOG=info

HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/embedding-service"]
```

**Changes from current file:**

The current embedding-service Dockerfile uses a manual dummy-stub approach
(no cargo-chef). The rewrite:

1. Adopts the same cargo-chef pattern as the API Dockerfile for consistency
   and better dependency caching.
2. Removes all references to `apps/api` (Cargo.toml copy, dummy stub
   creation).
3. Removes the `2>/dev/null || true` suppression of the dependency build
   errors (no longer needed since the workspace is valid).
4. Adds the `RUN printf` line to generate a single-member workspace manifest.
5. Copies only `apps/embedding-service` -- no other service directories.

### Why `Cargo.lock` Works Unchanged

Cargo's lockfile (`Cargo.lock` v4 format) contains entries for every package
resolved across the full workspace. When the workspace is narrowed to a single
member, Cargo reads the lockfile, uses the entries it needs, and silently
ignores entries for packages that are not part of the current dependency graph.
It does not error on "extra" entries. This is the same behavior that occurs when
you remove a workspace member locally -- `cargo build` still works without
regenerating the lockfile. The lockfile may be updated to prune unused entries,
but this only happens inside the Docker build layer (which is discarded) and
does not affect the repo-root lockfile.

### Why cargo-chef Works Unchanged

`cargo chef prepare` reads the workspace manifest and the Cargo.toml files of
all members to generate a `recipe.json`. Since the generated workspace manifest
has only one member, the recipe naturally scopes to that single service. No
special flags or configuration are needed. `cargo chef cook` then restores and
builds only the dependencies of that single member.

---

## Edge Cases & Constraints

### 1. Root Cargo.toml `[workspace]` keys beyond `resolver` and `members`

**Risk:** If the root `Cargo.toml` ever gains workspace-level keys like
`[workspace.dependencies]`, `[workspace.package]`, `[profile.release]`, or
`[patch.crates-io]`, the generated single-member manifest would not include
them. Builds would fail or produce different binaries.

**Mitigation:** The current root `Cargo.toml` has exactly two keys:
`resolver = "2"` and `members = [...]`. The generated manifest faithfully
reproduces both (with a different members list). If workspace-level keys are
added in the future, the `printf` command in each Dockerfile must be updated
to include them. This is a conscious trade-off: the simplicity of the current
approach outweighs the low probability of this scenario, and the failure mode is
a clear build error (not a silent misconfiguration).

**Future-proofing option:** If `[workspace.dependencies]` or similar keys are
adopted, the `printf` can be replaced with a small `sed` command that rewrites
only the `members` array while preserving the rest of the file:

```dockerfile
RUN sed -i 's/^members = \[.*\]/members = ["apps\/api"]/' Cargo.toml
```

This is not used today because `printf` is simpler and the root manifest is
minimal.

### 2. Cargo.lock drift during Docker build

**Risk:** Cargo may regenerate or prune the `Cargo.lock` inside the Docker
build layer when it detects the workspace has fewer members than the lockfile
expects.

**Mitigation:** This is harmless. The lockfile modification happens only inside
the ephemeral build container. The repo-root `Cargo.lock` is never modified.
The dependency versions used for the build still come from the original
lockfile -- Cargo preserves existing version pins and only removes entries
for packages no longer in the dependency graph.

### 3. Shared crates (future `crates/` directory)

**Risk:** If a shared crate is extracted into `crates/` and both services
depend on it, the Dockerfile must copy that crate and include it in the
generated workspace manifest.

**Mitigation:** When this happens, the `printf` line grows to include the
shared crate path, and a `COPY crates/shared-crate ./crates/shared-crate` is
added. This is a natural evolution and the change is localized to the
Dockerfile of the service that gains the dependency. The key guarantee of this
design is preserved: a service's Dockerfile only copies what it actually
depends on.

### 4. Docker build cache invalidation

**Risk:** The `COPY Cargo.toml` followed by `RUN printf ... > Cargo.toml`
pattern means the original root Cargo.toml is copied and then immediately
overwritten. If a new workspace member is added to the root Cargo.toml (but
the service's own Cargo.toml is unchanged), the Docker layer cache for the
`COPY Cargo.toml` line will be invalidated even though the generated manifest
is identical.

**Mitigation:** This is a minor inefficiency. The `COPY Cargo.toml` layer is
tiny (a few hundred bytes). The real caching benefit comes from the
cargo-chef cook layer (which caches compiled dependencies), and that layer is
unaffected because the generated manifest and the service's Cargo.toml are both
unchanged. In practice, the only time dependency caching is invalidated is when
the service's own Cargo.toml changes -- which is the correct behavior.

---

## Test Plan

No new tests are needed. This is a build infrastructure change that produces
identical binaries. Verification is done by building and running the existing
test suite.

### Verification Steps

#### 1. Docker images build successfully

```bash
docker compose build api
docker compose build embedding-service
```

**Success:** Both images build without errors.

#### 2. Docker stack runs and passes health checks

```bash
docker compose up -d
curl -f http://localhost:8080/health
curl -f http://localhost:3001/health
```

**Success:** Both services start and health endpoints return 200.

#### 3. E2E tests pass against the Docker stack

```bash
docker compose up -d --build api
cargo test -p tokenoverflow --test e2e
```

**Success:** All E2E tests pass (same results as before the change).

#### 4. Produced binaries are functionally identical

The binary names, paths, and runtime behavior are unchanged. The same Cargo.lock
pins the same dependency versions. The only difference is that sibling workspace
members are absent from the build container, which has no effect on the compiled
binary because there are no Cargo dependencies between them.

---

## Documentation Changes

No documentation changes are needed. The Dockerfiles are internal build
infrastructure and are not referenced in any user-facing documentation. The
README, docker-compose.yml, and all other files remain unchanged.

---

## Development Environment Changes

No changes to the development environment. The Brewfile, setup scripts,
environment variables, and local development workflow are unaffected. The
change is entirely contained within Docker build files.

---

## Tasks

### Task 1: Update `apps/api/Dockerfile`

**What:** Remove the embedding-service COPY lines and add single-member
workspace manifest generation.

**Steps:**

1. Read the current `apps/api/Dockerfile`.
2. In the **planner** stage:
   - Remove `COPY apps/embedding-service ./apps/embedding-service`.
   - After `COPY apps/api ./apps/api`, add:
     `RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/api"]\n' > Cargo.toml`
3. In the **builder** stage:
   - Remove `COPY apps/embedding-service ./apps/embedding-service`.
   - After `COPY apps/api ./apps/api`, add:
     `RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/api"]\n' > Cargo.toml`
4. All other stages remain unchanged.

**Verify:** `docker compose build api` succeeds.

**Success:** The API Docker image builds without copying any
embedding-service files.

### Task 2: Update `apps/embedding-service/Dockerfile`

**What:** Replace the dummy-stub approach with cargo-chef and single-member
workspace manifest generation.

**Steps:**

1. Read the current `apps/embedding-service/Dockerfile`.
2. Rewrite the build stages to use cargo-chef (matching the API pattern):
   - Add a `chef` stage with `cargo install cargo-chef`.
   - Add a `planner` stage that copies only `Cargo.toml`, `Cargo.lock`, and
     `apps/embedding-service`, then generates the single-member manifest
     and runs `cargo chef prepare`.
   - Add a `builder` stage that runs `cargo chef cook`, then copies the
     source and builds.
3. Remove all references to `apps/api` (no Cargo.toml copy, no dummy stubs).
4. Remove the `2>/dev/null || true` error suppression.
5. Keep the runtime stage unchanged (same binary path, ports, user, healthcheck).

**Verify:** `docker compose build embedding-service` succeeds.

**Success:** The embedding-service Docker image builds without copying any
API files and without creating dummy stub files.

### Task 3: Verify the full stack

**What:** Run the complete Docker stack and E2E tests to confirm nothing is
broken.

**Steps:**

1. `docker compose build` -- both images build.
2. `docker compose up -d` -- all services start.
3. `curl -f http://localhost:8080/health` -- API health check passes.
4. `curl -f http://localhost:3001/health` -- Embedding service health check
   passes.
5. `docker compose up -d --build api && cargo test -p tokenoverflow --test e2e`
   -- E2E tests pass.

**Success:** All services run and all tests pass identically to before.
