# Design: monorepo-restructure

## Architecture Overview

### Goal

Restructure the TokenOverflow monorepo from a language-centric layout
(`src/<language>/`) to a purpose-centric layout (`apps/`, `crates/`, `infra/`,
`scripts/`) that is simpler to navigate, scales with team growth, and follows
industry monorepo conventions.

### Scope

This design covers **directory restructuring only**. No application code logic,
API contracts, database schemas, or runtime behavior changes. The application
must work identically before and after the restructure.

### Current Structure (Before)

```text
tokenoverflow/
├── src/
│   ├── docker/
│   │   ├── api/Dockerfile
│   │   ├── diesel/Dockerfile
│   │   └── embedding-service/Dockerfile
│   ├── md/                                 # Distributable Claude Code plugin
│   │   ├── instructions.md
│   │   └── claude/
│   │       ├── .claude-plugin/plugin.json
│   │       ├── .mcp.json
│   │       ├── agents/tokenoverflow-researcher.md
│   │       ├── hooks/settings.json
│   │       └── skills/
│   │           ├── search-tokenoverflow/SKILL.md
│   │           └── submit-to-tokenoverflow/SKILL.md
│   ├── rust/
│   │   ├── Cargo.toml                    # Workspace root
│   │   ├── tokenoverflow/                # Main app
│   │   │   ├── Cargo.toml
│   │   │   ├── config/
│   │   │   ├── diesel.toml
│   │   │   ├── migrations/
│   │   │   ├── src/
│   │   │   └── tests/
│   │   └── crates/
│   │       └── embedding-service/
│   │           ├── Cargo.toml
│   │           ├── src/
│   │           └── tests/
│   ├── shell/
│   │   ├── tokenoverflow/
│   │   │   ├── includes.sh
│   │   │   ├── docs.sh
│   │   │   └── git_hooks/
│   │   └── tests/
│   └── terraform/
│       ├── .tflint.hcl
│       ├── live/
│       └── modules/
├── .claude/                              # Claude Code local project config (DO NOT MOVE)
│   ├── agents/
│   ├── hooks/
│   ├── rules/
│   └── settings.json
├── .agents/                              # OpenCode agent config
├── docs/
├── docker-compose.yml
├── .pre-commit-config.yaml
├── .mcp.json
└── (root config files)
```

### Proposed Structure (After)

```text
tokenoverflow/
├── apps/
│   ├── api/                              # Main TokenOverflow API + MCP
│   │   ├── Cargo.toml
│   │   ├── Dockerfile
│   │   ├── config/
│   │   ├── diesel.toml
│   │   ├── migrations/
│   │   ├── src/
│   │   └── tests/
│   ├── embedding-service/                # Standalone embedding service
│   │   ├── Cargo.toml
│   │   ├── Dockerfile
│   │   ├── src/
│   │   └── tests/
│   ├── web/                              # Future: Astro + SolidJS frontend
│   │   └── .gitkeep
│   └── claude/                           # Distributable Claude Code plugin
│       ├── .claude-plugin/
│       │   └── plugin.json
│       ├── .mcp.json
│       ├── agents/
│       │   └── tokenoverflow-researcher.md
│       ├── hooks/
│       │   └── settings.json
│       ├── instructions.md
│       └── skills/
│           ├── search-tokenoverflow/SKILL.md
│           └── submit-to-tokenoverflow/SKILL.md
├── crates/                               # Shared Rust libraries (future)
│   └── .gitkeep
├── infra/
│   ├── docker/
│   │   └── diesel/Dockerfile             # Tooling container (no app code)
│   └── terraform/
│       ├── .tflint.hcl
│       ├── live/
│       │   ├── root.hcl
│       │   ├── dev/
│       │   ├── prod/
│       │   └── global/
│       └── modules/
│           ├── aws-organizations/
│           └── aws-sso/
├── scripts/
│   ├── src/
│   │   ├── includes.sh
│   │   ├── docs.sh
│   │   └── git_hooks/
│   │       ├── cargo_coverage.sh
│   │       └── tflint.sh
│   └── tests/
│       └── test_docs.sh
├── docs/
│   ├── brief/
│   ├── design/
│   ├── plans/
│   ├── prd/
│   └── templates/
├── .claude/                              # Claude Code local project config (UNCHANGED location)
│   ├── agents/
│   │   ├── design-lead.md
│   │   ├── engineer.md
│   │   └── code-reviewer.md
│   ├── hooks/
│   │   └── tdd.sh
│   ├── rules/
│   │   └── rust/
│   │       └── code-style.md
│   └── settings.json
├── .agents/                              # OpenCode agent config (unchanged)
├── Cargo.toml                            # Workspace root (at repo root)
├── docker-compose.yml                    # Stays at root
├── .editorconfig
├── .gitignore
├── .mcp.json
├── .opentofu-version
├── .pre-commit-config.yaml
├── Brewfile
├── CLAUDE.md
└── README.md
```

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Cargo workspace root at repo root | Eliminates `--manifest-path` in every cargo command. `cargo test`, `cargo build`, `cargo clippy` work from repo root. |
| `apps/` for deployable artifacts | Each entry is something built and deployed: a binary, a service, a website. Dockerfile lives next to the code it builds, respecting locality. |
| `apps/claude/` for the distributable Claude Code plugin | Contains the plugin that external users install via `/plugin install tokenoverflow`. This is the product's Claude integration that gets distributed -- entirely separate from `.claude/` which is the local project config. |
| `.claude/` stays at repo root unchanged | `.claude/` is the local Claude Code project configuration (agents, hooks, rules, settings). It must remain at the repo root for Claude Code to discover it. It is NOT moved. |
| `crates/` for shared Rust libraries | Currently empty with `.gitkeep`. Ready for extraction when shared code emerges. Does NOT hold apps -- `embedding-service` has a `main.rs` and Dockerfile, so it is an app. |
| `infra/` for infrastructure | Terraform and tooling-only Docker images (diesel CLI) grouped together. |
| `scripts/` for shell scripts | Flat structure, no redundant `tokenoverflow/` nesting. `src/` and `tests/` mirror the existing pattern. |
| `docker-compose.yml` at root | Primary developer entry point. Moving it would add `-f` flag friction to every `docker compose` command. |

### Directory Move Mapping

| Before (Old Path) | After (New Path) |
|--------------------|-------------------|
| `src/rust/Cargo.toml` | `Cargo.toml` (repo root) |
| `src/rust/tokenoverflow/` | `apps/api/` |
| `src/rust/crates/embedding-service/` | `apps/embedding-service/` |
| `src/docker/api/Dockerfile` | `apps/api/Dockerfile` |
| `src/docker/embedding-service/Dockerfile` | `apps/embedding-service/Dockerfile` |
| `src/docker/diesel/Dockerfile` | `infra/docker/diesel/Dockerfile` |
| `src/terraform/` | `infra/terraform/` |
| `src/shell/tokenoverflow/includes.sh` | `scripts/src/includes.sh` |
| `src/shell/tokenoverflow/docs.sh` | `scripts/src/docs.sh` |
| `src/shell/tokenoverflow/git_hooks/` | `scripts/src/git_hooks/` |
| `src/shell/tests/` | `scripts/tests/` |
| `src/md/instructions.md` | `apps/claude/instructions.md` |
| `src/md/claude/.claude-plugin/` | `apps/claude/.claude-plugin/` |
| `src/md/claude/.mcp.json` | `apps/claude/.mcp.json` |
| `src/md/claude/agents/` | `apps/claude/agents/` |
| `src/md/claude/hooks/` | `apps/claude/hooks/` |
| `src/md/claude/skills/` | `apps/claude/skills/` |

---

## Interfaces

This section documents every file that contains a path reference to the old
structure and must be updated. Since this is a structural change (not a
feature), the "interfaces" are the path contracts between files.

### Build System

| File (new path) | Old Path Reference | New Path Reference |
|------------------|--------------------|--------------------|
| `Cargo.toml` (repo root) | `members = ["tokenoverflow", "crates/embedding-service"]` | `members = ["apps/api", "apps/embedding-service"]` |
| `apps/api/Cargo.toml` | Relative paths (`src/lib.rs`, `src/main.rs`, `tests/`) | No change (relative to Cargo.toml, preserved by move) |
| `apps/embedding-service/Cargo.toml` | Relative paths | No change (relative to Cargo.toml, preserved by move) |
| `apps/api/diesel.toml` | `file = "src/db/schema.rs"`, `dir = "migrations"` | No change (relative to diesel.toml, preserved by move) |

### Docker

| File (new path) | Old Reference | New Reference |
|------------------|---------------|---------------|
| `docker-compose.yml` | `context: ./src/docker/diesel` | `context: .` |
| | `volumes: ./src/rust/tokenoverflow:/volume` | `volumes: ./apps/api:/volume` |
| | `dockerfile: src/docker/embedding-service/Dockerfile` | `dockerfile: apps/embedding-service/Dockerfile` |
| | `dockerfile: src/docker/api/Dockerfile` | `dockerfile: apps/api/Dockerfile` |
| `apps/api/Dockerfile` | `COPY src/rust/Cargo.toml ./Cargo.toml` | `COPY Cargo.toml ./Cargo.toml` |
| | `COPY src/rust/Cargo.lock ./Cargo.lock` | `COPY Cargo.lock ./Cargo.lock` |
| | `COPY src/rust/tokenoverflow ./tokenoverflow` | `COPY apps/api ./apps/api` |
| | `COPY src/rust/crates ./crates` | `COPY apps/embedding-service ./apps/embedding-service` |
| | `COPY src/rust/tokenoverflow/config /app/config` | `COPY apps/api/config /app/config` |
| `apps/embedding-service/Dockerfile` | `COPY src/rust/Cargo.toml ./Cargo.toml` | `COPY Cargo.toml ./Cargo.toml` |
| | `COPY src/rust/tokenoverflow/Cargo.toml ./tokenoverflow/Cargo.toml` | `COPY apps/api/Cargo.toml ./apps/api/Cargo.toml` |
| | `COPY src/rust/crates/embedding-service/Cargo.toml ./crates/embedding-service/Cargo.toml` | `COPY apps/embedding-service/Cargo.toml ./apps/embedding-service/Cargo.toml` |
| | `mkdir -p tokenoverflow/src crates/embedding-service/src` | `mkdir -p apps/api/src apps/embedding-service/src` |
| | `echo "fn main() {}" > tokenoverflow/src/main.rs` | `echo "fn main() {}" > apps/api/src/main.rs` |
| | `echo "pub fn lib() {}" > tokenoverflow/src/lib.rs` | `echo "pub fn lib() {}" > apps/api/src/lib.rs` |
| | `echo "fn main() {}" > crates/embedding-service/src/main.rs` | `echo "fn main() {}" > apps/embedding-service/src/main.rs` |
| | `COPY src/rust/crates/embedding-service/src ./crates/embedding-service/src` | `COPY apps/embedding-service/src ./apps/embedding-service/src` |
| | `touch crates/embedding-service/src/main.rs` | `touch apps/embedding-service/src/main.rs` |
| `infra/docker/diesel/Dockerfile` | (no path references) | No change |

### Pre-commit & Git Hooks

| File (new path) | Old Reference | New Reference |
|------------------|---------------|---------------|
| `.pre-commit-config.yaml` | `entry: src/shell/tokenoverflow/git_hooks/tflint.sh` | `entry: scripts/src/git_hooks/tflint.sh` |
| | `entry: trivy fs ./src/` | `entry: trivy fs .` |
| | `entry: cargo fmt --manifest-path src/rust/tokenoverflow/Cargo.toml -- --check` | `entry: cargo fmt --manifest-path apps/api/Cargo.toml -- --check` |
| | `entry: cargo clippy --manifest-path src/rust/tokenoverflow/Cargo.toml` | `entry: cargo clippy --manifest-path apps/api/Cargo.toml` |
| | `entry: src/shell/tokenoverflow/git_hooks/cargo_coverage.sh` | `entry: scripts/src/git_hooks/cargo_coverage.sh` |
| `scripts/src/git_hooks/cargo_coverage.sh` | `MANIFEST_PATH="src/rust/Cargo.toml"` | `MANIFEST_PATH="Cargo.toml"` |
| `scripts/src/git_hooks/tflint.sh` | `CONFIG="$(pwd)/src/terraform/.tflint.hcl"` | `CONFIG="$(pwd)/infra/terraform/.tflint.hcl"` |
| | `MODULES="$(pwd)/src/terraform/modules/"` | `MODULES="$(pwd)/infra/terraform/modules/"` |

### Shell Scripts

| File (new path) | Old Reference | New Reference |
|------------------|---------------|---------------|
| `scripts/src/includes.sh` | `source "src/shell/tokenoverflow/docs.sh"` | `source "scripts/src/docs.sh"` |
| | `cd "src/terraform/live/$env"` | `cd "infra/terraform/live/$env"` |
| | `# shellcheck source=src/shell/tokenoverflow/docs.sh` | `# shellcheck source=scripts/src/docs.sh` |
| `scripts/tests/test_docs.sh` | `# shellcheck source=src/shell/tokenoverflow/docs.sh` | `# shellcheck source=scripts/src/docs.sh` |
| | `source "src/shell/tokenoverflow/docs.sh"` | `source "scripts/src/docs.sh"` |
| `scripts/src/docs.sh` | (no path references) | No change |

### Claude Code Local Config (glob patterns update only)

| File (stays in place) | Old Glob/Path Pattern | New Glob/Path Pattern |
|------------------------|----------------------|----------------------|
| `.claude/settings.json` | `Write(src/rust/**/*.rs:!**/tests/**:!**/*_test.rs)` | `Write(apps/**/*.rs:!**/tests/**:!**/*_test.rs)` |
| `.claude/rules/rust/code-style.md` | `paths: "src/api/**/*.rs"` | `paths: "apps/api/src/**/*.rs"` |

### Agent Config (OpenCode)

| File (new path) | Old Reference | New Reference |
|------------------|---------------|---------------|
| `.agents/skills/design-doc/SKILL.md` | `source src/shell/tokenoverflow/includes.sh` | `source scripts/src/includes.sh` |

### MCP Config

| File (new path) | Old Reference | New Reference |
|------------------|---------------|---------------|
| `.mcp.json` | `"--manifest-path", "src/rust/tokenoverflow/Cargo.toml"` | `"--manifest-path", "apps/api/Cargo.toml"` |

### Documentation

| File | What Changes |
|------|-------------|
| `README.md` | All path references updated (see Logic section for full rewrite) |
| `docs/terraform.md` | `src/terraform/` -> `infra/terraform/`, `src/shell/tokenoverflow/includes.sh` -> `scripts/src/includes.sh` |
| `apps/api/tests/unit/README.md` | `cargo test --manifest-path src/rust/tokenoverflow/Cargo.toml` -> `cargo test -p tokenoverflow` |
| `apps/api/tests/e2e/README.md` | `cargo test --manifest-path src/rust/tokenoverflow/Cargo.toml` -> `cargo test -p tokenoverflow` |

### .gitignore

| File | What Changes |
|------|-------------|
| `.gitignore` | Remove `Cargo.lock` from gitignore. The workspace `Cargo.lock` at repo root should be committed (best practice for applications). |

### Files That Do NOT Need Changes

| File | Reason |
|------|--------|
| `.claude/agents/design-lead.md` | No path references. File stays at `.claude/agents/`. |
| `.claude/agents/engineer.md` | No path references. File stays at `.claude/agents/`. |
| `.claude/agents/code-reviewer.md` | No path references. File stays at `.claude/agents/`. |
| `.claude/hooks/tdd.sh` | No path references. Uses `$CLAUDE_PROJECT_DIR` which resolves dynamically. File stays at `.claude/hooks/`. |
| `.editorconfig` | Uses file-type patterns, not paths |
| `.opentofu-version` | Contains only a version string |
| `Brewfile` | Contains only package names |
| `CLAUDE.md` | No path references |
| `apps/api/config/*.toml` | Configuration values only, no file paths |
| `apps/api/src/config.rs` | Uses `TOKENOVERFLOW_CONFIG_DIR` env var with relative default `"config"`. `CARGO_MANIFEST_DIR` in tests resolves to the absolute path of `apps/api/`. No code changes needed. |
| `apps/api/tests/CLAUDE.md` | No path references |
| `apps/claude/*` (all files) | These are the distributable plugin contents moved from `src/md/`. Their content has no references to the monorepo's internal paths. |
| `infra/terraform/live/*/terragrunt.hcl` | Uses `find_in_parent_folders()` and relative paths. Internal terraform structure preserved. |
| `infra/terraform/live/root.hcl` | No external path references |
| `.agents/skills/implement-design/SKILL.md` | No path references |
| `.agents/skills/validate-changes/SKILL.md` | No path references |
| `.agents/skills/code-review/SKILL.md` | No path references |

---

## Logic

This section defines the exact sequence of operations for the migration. The
engineer must follow these steps in order. Each step is atomic -- if any step
fails, the migration can be rolled back to the previous step.

**Prerequisites:** Start from a clean working tree on the `main` branch.

### Phase 1: Move directories with `git mv`

Order matters. Move leaf directories first, then parent directories, to avoid
conflicts.

#### Step 1.1: Create target directory scaffolding

```bash
mkdir -p apps/web
mkdir -p apps/claude
mkdir -p crates
mkdir -p infra/docker
mkdir -p scripts/src
mkdir -p scripts/tests
touch apps/web/.gitkeep
touch crates/.gitkeep
```

#### Step 1.2: Move the main API app

```bash
git mv src/rust/tokenoverflow apps/api
```

#### Step 1.3: Move the embedding-service app

```bash
git mv src/rust/crates/embedding-service apps/embedding-service
```

#### Step 1.4: Move the workspace Cargo.toml to repo root

```bash
git mv src/rust/Cargo.toml Cargo.toml
```

#### Step 1.5: Move Cargo.lock to repo root (un-gitignored later)

The workspace `Cargo.lock` exists at `src/rust/Cargo.lock` but is gitignored.
Copy it to the repo root (it will be un-gitignored in Phase 2).

```bash
cp src/rust/Cargo.lock Cargo.lock
```

#### Step 1.6: Move Dockerfiles next to their apps

```bash
git mv src/docker/api/Dockerfile apps/api/Dockerfile
git mv src/docker/embedding-service/Dockerfile apps/embedding-service/Dockerfile
git mv src/docker/diesel infra/docker/diesel
```

#### Step 1.7: Move terraform

```bash
git mv src/terraform infra/terraform
```

#### Step 1.8: Move shell scripts

```bash
git mv src/shell/tokenoverflow/includes.sh scripts/src/includes.sh
git mv src/shell/tokenoverflow/docs.sh scripts/src/docs.sh
git mv src/shell/tokenoverflow/git_hooks scripts/src/git_hooks
git mv src/shell/tests/test_docs.sh scripts/tests/test_docs.sh
```

#### Step 1.9: Move distributable Claude Code plugin

The `src/md/` directory contains the distributable Claude Code plugin --
the product that external users install. This is entirely separate from
`.claude/` which is the local project configuration.

```bash
git mv src/md/instructions.md apps/claude/instructions.md
git mv src/md/claude/.claude-plugin apps/claude/.claude-plugin
git mv src/md/claude/.mcp.json apps/claude/.mcp.json
git mv src/md/claude/agents apps/claude/agents
git mv src/md/claude/hooks apps/claude/hooks
git mv src/md/claude/skills apps/claude/skills
```

#### Step 1.10: Clean up empty `src/` tree

After all moves, the `src/` directory should be empty (except for build
artifacts in `target/` which are gitignored). Remove it.

```bash
# Verify nothing tracked remains
git ls-files src/
# If empty, remove the directory
rm -rf src/
```

#### Step 1.11: Delete the orphaned Cargo.lock

There is an orphaned `Cargo.lock` at `src/rust/tokenoverflow/Cargo.lock` (from
before workspace was set up). It was already inside `apps/api/` after step 1.2.
Remove it.

```bash
rm -f apps/api/Cargo.lock
```

### Phase 2: Update file contents

All path references in configuration and build files must be updated to reflect
the new layout. Each file edit below shows the exact before/after.

**Step 2.1: `Cargo.toml` (repo root)**

```toml
[workspace]
resolver = "2"
members = [
    "apps/api",
    "apps/embedding-service",
]
```

**Step 2.2: `.gitignore` -- un-gitignore Cargo.lock**

Remove the `Cargo.lock` line from `.gitignore`. Applications should commit
their lockfile to ensure reproducible builds. Then stage it.

Replace:

```gitignore
# Rust
target/
Cargo.lock
```

With:

```gitignore
# Rust
target/
```

Then:

```bash
git add Cargo.lock
```

**Step 2.3: `docker-compose.yml`**

Full updated file:

```yaml
# TokenOverflow Local Development Environment
#
# Usage:
#   docker compose up -d

services:
  postgres:
    image: pgvector/pgvector:pg17
    container_name: tokenoverflow-db
    environment:
      POSTGRES_USER: tokenoverflow
      POSTGRES_PASSWORD: localdev
      POSTGRES_DB: tokenoverflow
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U tokenoverflow"]
      interval: 5s
      timeout: 5s
      retries: 5

  migrations:
    build:
      context: .
      dockerfile: infra/docker/diesel/Dockerfile
    working_dir: /volume
    volumes:
      - ./apps/api:/volume
    environment:
      DATABASE_URL: postgres://tokenoverflow:localdev@postgres:5432/tokenoverflow
    depends_on:
      postgres:
        condition: service_healthy
    command: migration run

  pgbouncer:
    image: edoburu/pgbouncer:latest
    container_name: tokenoverflow-pgbouncer
    ports:
      - "6432:6432"
    environment:
      DATABASE_URL: postgres://tokenoverflow:localdev@postgres:5432/tokenoverflow
      POOL_MODE: transaction
      MAX_PREPARED_STATEMENTS: 500
      DEFAULT_POOL_SIZE: 20
      MAX_CLIENT_CONN: 100
      LISTEN_PORT: 6432
      # SCRAM-SHA-256 authentication with passthrough to PostgreSQL
      AUTH_TYPE: scram-sha-256
      AUTH_USER: tokenoverflow
      AUTH_QUERY: SELECT usename, passwd FROM pg_shadow WHERE usename=$1
    depends_on:
      migrations:
        condition: service_completed_successfully

  embedding-service:
    build:
      context: .
      dockerfile: apps/embedding-service/Dockerfile
    container_name: tokenoverflow-embeddings
    ports:
      - "3001:8080"
    environment:
      HOST: 0.0.0.0
      PORT: 8080
      RUST_LOG: info
    volumes:
      - embedding_model_cache:/app/.fastembed_cache
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      start_period: 60s
      retries: 3

  api:
    build:
      context: .
      dockerfile: apps/api/Dockerfile
    container_name: tokenoverflow-api
    ports:
      - "8080:8080"
    environment:
      # Override localhost with Docker service names
      TOKENOVERFLOW__DATABASE__HOST: pgbouncer
      TOKENOVERFLOW__EMBEDDING__BASE_URL: http://embedding-service:8080/v1
      # Secrets
      TOKENOVERFLOW_DATABASE_PASSWORD: localdev
      OPENAI_API_KEY: sk-mock-local-development-key
    depends_on:
      pgbouncer:
        condition: service_started
      embedding-service:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 5s
      start_period: 5s
      retries: 3

volumes:
  postgres_data:
  embedding_model_cache:
```

Changes from original:

- `migrations.build.context`: `./src/docker/diesel` -> `.`
- `migrations.build.dockerfile`: `Dockerfile` -> `infra/docker/diesel/Dockerfile`
- `migrations.volumes`: `./src/rust/tokenoverflow:/volume` ->
  `./apps/api:/volume`
- `embedding-service.build.dockerfile`:
  `src/docker/embedding-service/Dockerfile` ->
  `apps/embedding-service/Dockerfile`
- `api.build.dockerfile`: `src/docker/api/Dockerfile` ->
  `apps/api/Dockerfile`

**Step 2.4: `apps/api/Dockerfile`**

Full updated file:

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

# Copy workspace manifests and source
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/api ./apps/api
COPY apps/embedding-service ./apps/embedding-service

# Generate recipe.json for dependency caching
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Stage 3: Builder - Compile dependencies then application
# =============================================================================
FROM chef AS builder

# Copy recipe and build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source and build application
COPY Cargo.toml ./Cargo.toml
COPY Cargo.lock ./Cargo.lock
COPY apps/api ./apps/api
COPY apps/embedding-service ./apps/embedding-service
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

**Step 2.5: `apps/embedding-service/Dockerfile`**

Full updated file:

```dockerfile
# Build stage
FROM rust:1.93-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    g++ \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml ./Cargo.toml
COPY apps/api/Cargo.toml ./apps/api/Cargo.toml
COPY apps/embedding-service/Cargo.toml ./apps/embedding-service/Cargo.toml

# Create dummy source files to cache dependencies
RUN mkdir -p apps/api/src apps/embedding-service/src
RUN echo "fn main() {}" > apps/api/src/main.rs
RUN echo "pub fn lib() {}" > apps/api/src/lib.rs
RUN echo "fn main() {}" > apps/embedding-service/src/main.rs

# Build dependencies only (cache layer)
RUN cargo build --release -p embedding-service 2>/dev/null || true

# Copy actual source code
COPY apps/embedding-service/src ./apps/embedding-service/src

# Touch to invalidate cache
RUN touch apps/embedding-service/src/main.rs

# Build the binary
RUN cargo build --release -p embedding-service

# Runtime stage
FROM debian:bookworm-slim

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

**Step 2.6: `.pre-commit-config.yaml`**

Replace the `repo: local` hooks section. Only the local hooks change; all
external repo hooks remain identical.

```yaml
  - repo: local
    hooks:
      - id: tflint
        name: TFLint
        entry: scripts/src/git_hooks/tflint.sh
        language: script
        stages:
          - pre-commit
      - id: trivy
        name: Trivy Scan
        entry: trivy fs .
        language: system
        pass_filenames: false
        stages:
          - pre-commit
      - id: cargo-fmt
        name: Cargo Format
        entry: cargo fmt --manifest-path apps/api/Cargo.toml -- --check
        language: system
        types: [ rust ]
        pass_filenames: false
      - id: cargo-clippy
        name: Cargo Clippy
        entry: cargo clippy --manifest-path apps/api/Cargo.toml
        language: system
        types: [ rust ]
        pass_filenames: false
      - id: cargo-coverage
        name: Cargo Coverage (90%)
        entry: scripts/src/git_hooks/cargo_coverage.sh
        language: script
        types: [ rust ]
        pass_filenames: false
```

**Step 2.7: `scripts/src/git_hooks/cargo_coverage.sh`**

Change one line:

```
MANIFEST_PATH="src/rust/Cargo.toml"
```

To:

```
MANIFEST_PATH="Cargo.toml"
```

**Step 2.8: `scripts/src/git_hooks/tflint.sh`**

Full updated file:

```bash
#!/usr/bin/env zsh

CONFIG="$(pwd)/infra/terraform/.tflint.hcl"
MODULES="$(pwd)/infra/terraform/modules/"

tflint --config="${CONFIG}" --chdir="${MODULES}" --init
tflint --config="${CONFIG}" --chdir="${MODULES}" --recursive
```

**Step 2.9: `scripts/src/includes.sh`**

Two changes:

1. The `source` line for docs.sh:

    ```
    # shellcheck source=src/shell/tokenoverflow/docs.sh
    source "src/shell/tokenoverflow/docs.sh"
    ```

    Becomes:

    ```
    # shellcheck source=scripts/src/docs.sh
    source "scripts/src/docs.sh"
    ```

2. The `tg()` function's `cd` line:

    ```
    cd "src/terraform/live/$env" || return 1
    ```

    Becomes:

    ```
    cd "infra/terraform/live/$env" || return 1
    ```

**Step 2.10: `scripts/tests/test_docs.sh`**

Two changes in `oneTimeSetUp()`:

```
  # shellcheck source=src/shell/tokenoverflow/docs.sh
  source "src/shell/tokenoverflow/docs.sh"
```

Becomes:

```
  # shellcheck source=scripts/src/docs.sh
  source "scripts/src/docs.sh"
```

**Step 2.11: `.mcp.json`**

Full updated file:

```json
{
  "mcpServers": {
    "tokenoverflow": {
      "command": "cargo",
      "args": [
        "run",
        "--manifest-path",
        "apps/api/Cargo.toml",
        "--release"
      ],
      "env": {
        "TOKENOVERFLOW_ENV": "local"
      }
    }
  }
}
```

**Step 2.12: `.claude/settings.json` (file stays in place, update glob pattern)**

The file remains at `.claude/settings.json`. Only the Write matcher glob
pattern changes because the Rust source files moved from `src/rust/` to `apps/`.

Full updated file:

```json
{
  "enabledPlugins": {
    "rust-analyzer-lsp@claude-plugins-official": true
  },
  "permissions": {
    "defaultMode": "plan"
  },
  "alwaysThinkingEnabled": true,
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write(apps/**/*.rs:!**/tests/**:!**/*_test.rs)",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/tdd.sh"
          }
        ]
      }
    ]
  }
}
```

#### Step 2.13: `.claude/rules/rust/code-style.md`

The file remains at `.claude/rules/rust/code-style.md`. Only the paths
frontmatter changes because the Rust source files moved.

Change the paths frontmatter:

```yaml
---
paths:
    - "src/api/**/*.rs"
---
```

Becomes:

```yaml
---
paths:
    - "apps/api/src/**/*.rs"
---
```

**Step 2.14: `.agents/skills/design-doc/SKILL.md`**

Change one line:

```
3. Run `source src/shell/tokenoverflow/includes.sh` and
```

Becomes:

```
3. Run `source scripts/src/includes.sh` and
```

**Step 2.15: `apps/api/tests/unit/README.md`**

Full updated file:

```markdown
# Unit Tests

Unit tests run **without Docker**. They rely on local PostgreSQL binaries.

## What Goes Here

- Tests for individual components (routes, services, types)
- In-process API testing using `axum` with `tower::ServiceExt::oneshot`
- Database operations via local postgres binaries (no external services)

## What Does NOT Go Here

- Tests requiring the full Docker stack
- Tests that make HTTP requests to external APIs
- End-to-end flow tests

## Running

    ```bash
    cargo test -p tokenoverflow --test unit
    ```
```

**Step 2.16: `apps/api/tests/e2e/README.md`**

Full updated file:

```markdown
# Integration Tests

Integration tests require the **full Docker stack** to be running.

## What Goes Here

- End-to-end flow tests
- Tests that hit the actual API via HTTP
- Tests verifying Docker service interactions

## What Does NOT Go Here

- Unit tests for individual components
- Tests that can run without Docker

## Running

    ```bash
    # Start all services
    docker compose up -d

    # Run integration tests
    cargo test -p tokenoverflow --test integration
    ```
```

**Step 2.17: `docs/terraform.md`**

All `src/terraform/` references become `infra/terraform/`. The
`src/shell/tokenoverflow/includes.sh` reference becomes
`scripts/src/includes.sh`. The `cd src/terraform/live/global/aws-sso` becomes
`cd infra/terraform/live/global/aws-sso`.

**Step 2.18: `README.md`**

Full rewrite of the README. Every path reference is updated. The workspace
architecture diagram changes. Testing commands use `-p` package selection
instead of `--manifest-path`. Coverage commands drop `--manifest-path`. The
configuration path reference changes from `src/rust/tokenoverflow/config/` to
`apps/api/config/`. The `source` command changes from
`source src/shell/tokenoverflow/includes.sh` to
`source scripts/src/includes.sh`.

Full updated file:

```markdown
# Token Overflow

[![pre-commit](https://img.shields.io/badge/pre--commit-enabled-brightgreen?logo=pre-commit)](https://github.com/pre-commit/pre-commit)

Q&A knowledge-base for all coding AI agents.

## Local Development

### Prerequisites

Install dependencies:

    ```bash
    source scripts/src/includes.sh
    setup
    ```

Setup your environment:

    ```bash
    export PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH"
    ```

### Setup

1. Run the local stack:

       ```bash
       docker compose up -d
       ```

2. Verify the API is running:

       ```bash
       curl http://localhost:8080/health
       # Expected: {"status":"ok","database":"connected"}
       ```

## Configuration

Configuration is managed through TOML files in `apps/api/config/`:

| Environment | File               | Usage                                               |
|-------------|--------------------|-----------------------------------------------------|
| unit_test   | `unit_test.toml`   | Unit tests with mocks                               |
| local       | `local.toml`       | Local development & integration testing with Docker |
| development | `development.toml` | Cloud dev environment                               |
| production  | `production.toml`  | Cloud production environment                        |

Set the environment:

    ```bash
    export TOKENOVERFLOW_ENV=local  # or unit_test, development, production
    ```

### Secrets

Only secrets come from environment variables:

| Variable                          | Purpose           |
|-----------------------------------|-------------------|
| `TOKENOVERFLOW_DATABASE_PASSWORD` | Database password |
| `OPENAI_API_KEY`                  | OpenAI API key    |

### Local Overrides

Create `config/local.override.toml` (gitignored) for personal settings.

## Testing

### Unit Tests

Unit tests run without external dependencies (uses MockEmbedding):

    ```bash
    cargo test -p tokenoverflow --test unit
    ```

### Integration Tests

Integration tests require the API to be running:

    ```bash
    # Start all services
    docker compose up -d

    # Run integration tests (uses config/local.toml)
    cargo test -p tokenoverflow --test integration
    ```

Each test binary automatically sets `TOKENOVERFLOW_ENV`. To run integration
tests against cloud environments, override the env var:

    ```bash
    # Development
    TOKENOVERFLOW_ENV=development cargo test -p tokenoverflow --test integration

    # Production
    TOKENOVERFLOW_ENV=production cargo test -p tokenoverflow --test integration
    ```

### Embedding Service Tests

Tests for the embedding-service crate:

    ```bash
    cargo test -p embedding-service
    ```

### All Tests

    ```bash
    # Workspace-wide tests
    cargo test --workspace
    ```

### Code Coverage

The project enforces 90% line coverage. Coverage is checked automatically on
commit via pre-commit hooks.

To run coverage manually:

    ```bash
    # Requires Docker services running for integration tests
    docker compose up -d

    # Run coverage with threshold enforcement
    cargo llvm-cov --workspace --fail-under-lines 90
    ```

To generate an HTML report:

    ```bash
    cargo llvm-cov --workspace --html
    # Report at target/llvm-cov/html/index.html
    ```

## Architecture

### Monorepo Layout

    ```text
    .
    ├── apps/                     # Deployable applications & distributable artifacts
    │   ├── api/                  # Main TokenOverflow API + MCP server
    │   ├── embedding-service/    # OpenAI-compatible embedding service
    │   ├── web/                  # Future: frontend
    │   └── claude/               # Distributable Claude Code plugin
    ├── crates/                   # Shared Rust libraries (future)
    ├── infra/                    # Infrastructure
    │   ├── docker/               # Tooling containers (diesel CLI)
    │   └── terraform/            # IaC (Terragrunt + OpenTofu)
    ├── scripts/                  # Shell scripts and git hooks
    │   ├── src/
    │   └── tests/
    ├── docs/                     # Documentation
    ├── Cargo.toml                # Rust workspace root
    └── docker-compose.yml        # Local development services
    ```

### Local Development Services

| Service               | Port | Purpose                                            |
|-----------------------|------|----------------------------------------------------|
| PostgreSQL (pgvector) | 5432 | Database with vector support                       |
| PgBouncer             | 6432 | Connection pooling (transaction mode + prepared)   |
| embedding-service     | 3001 | Local embeddings using fastembed-rs (BGE-small-en) |
| API                   | 8080 | TokenOverflow API                                  |

### Embedding Service Configuration

The embedding service is selected based on environment configuration:

| TOKENOVERFLOW_ENV | Embedding Service                      |
|-------------------|----------------------------------------|
| `unit_test`       | MockEmbedding (in test code)           |
| `local`           | embedding-service (via localhost:3001) |
| `development`     | Real OpenAI API                        |
| `production`      | Real OpenAI API                        |

The embedding-service uses the
[BGE-small-en-v1.5](https://huggingface.co/BAAI/bge-small-en-v1.5) model via
[fastembed-rs](https://github.com/Anush008/fastembed-rs), providing real
semantic embeddings without external API calls. Embeddings are padded from
384 to 1536 dimensions to match the production schema.

## Agent Integration

TokenOverflow integrates with AI coding agents, so they automatically search
the knowledge base before using web search and submit solutions after solving
problems. Three ecosystems are supported: Claude Code, OpenCode, and Codex CLI.

### Claude Code

    ```
    /plugin install tokenoverflow
    ```
```

### Phase 3: Stage and verify

After all edits are complete:

```bash
git add -A
cargo build --workspace
cargo test --workspace
docker compose build
prek run --all-files
```

If all checks pass, commit:

```bash
git commit -m "refactor: restructure monorepo to apps/crates/infra/scripts layout"
```

---

## Edge Cases & Constraints

### 1. Cargo workspace root and `config/` resolution

**Risk:** The `config.rs` file defaults `TOKENOVERFLOW_CONFIG_DIR` to
`"config"`, which is relative to the working directory. When running
`cargo run -p tokenoverflow` from the repo root, the working directory is the
repo root, but `config/` lives at `apps/api/config/`.

**Mitigation:** There are three approaches (all of which already work):

1. The Dockerfile copies `apps/api/config` to `/app/config` -- production is
   unaffected.
2. For local development, set the environment variable:
   `TOKENOVERFLOW_CONFIG_DIR=apps/api/config`
3. Tests use `CARGO_MANIFEST_DIR` to compute the absolute config path, so they
   are unaffected.

The `includes.sh` setup or a `.cargo/config.toml` can set this automatically
for local development. The recommended approach is to add a
`.cargo/config.toml` at the repo root:

```toml
[env]
TOKENOVERFLOW_CONFIG_DIR = { value = "apps/api/config", relative = true }
```

This sets the env var automatically for all `cargo run` invocations from any
directory within the workspace.

### 2. `target/` directory location

**Risk:** After moving the workspace root to repo root, the `target/`
directory will be at `./target/` instead of `src/rust/target/`. Existing build
artifacts in `src/rust/target/` become orphaned.

**Mitigation:** The `.gitignore` already ignores `target/` globally. After the
migration, run `rm -rf src/` to clean up the old target directory along with
the empty `src/` tree. A fresh `cargo build` will populate `./target/`.

### 3. Pre-commit hook paths must match before hooks run

**Risk:** If `git mv` and file edits are done in separate commits, pre-commit
hooks will fail because they reference paths that no longer exist.

**Mitigation:** All moves and edits must be done in a single commit.
Pre-commit hooks should be temporarily skipped during intermediate steps
using `git commit --no-verify` only if needed for incremental work, but the
final commit must pass all hooks.

### 4. Trivy scan scope change

**Risk:** Changing `trivy fs ./src/` to `trivy fs .` will scan more files
(docs, scripts, etc.), which may be slightly slower but provides better
coverage.

**Mitigation:** This is intentional. Trivy should scan the entire repo for
secrets and vulnerabilities, not just `src/`. The performance difference is
negligible.

### 5. Cargo.lock should be committed

**Risk:** `Cargo.lock` is currently gitignored. Moving the workspace root to
repo root is a good opportunity to start committing it.

**Mitigation:** Remove `Cargo.lock` from `.gitignore`. Per Cargo documentation,
applications (as opposed to libraries) should commit their lockfile to ensure
reproducible builds. Both `apps/api` and `apps/embedding-service` are
applications.

### 6. Historical design documents contain old paths

**Risk:** Design documents like `docs/design/2026_01_31_system_design.md`
reference old paths like `src/rust/tokenoverflow/`. These are historical
records.

**Mitigation:** Do NOT update historical design documents. They are a snapshot
of the codebase at the time they were written. The new structure is documented
in the README and in this design document.

### 7. MEMORY.md in user's private Claude config

**Risk:** The user's private `MEMORY.md` at
`~/.claude/projects/-Users-berkay-projects-tokenoverflow/memory/MEMORY.md`
references old paths like `Cargo.toml: src/rust/tokenoverflow/Cargo.toml` and
`Config files: src/rust/tokenoverflow/config/`.

**Mitigation:** The user should update their `MEMORY.md` after the migration.
This is outside the scope of the codebase changes but should be called out as
a post-migration step.

### 8. `.claude/` local config vs `apps/claude/` distributable plugin

**Risk:** Confusion between `.claude/` (local Claude Code project
configuration: agents, hooks, rules, settings) and `apps/claude/` (the
distributable Claude Code plugin that external users install). These are
entirely separate concerns with no relationship.

**Mitigation:** This design document and the README monorepo layout diagram
clearly distinguish the two. `.claude/` is annotated as "Claude Code local
project config (UNCHANGED location)" and `apps/claude/` as "Distributable
Claude Code plugin". The `apps/claude/` contents have no references to the
monorepo's internal paths -- they point to the production API at
`https://api.tokenoverflow.io/mcp`.

---

## Test Plan

### Verification Checklist

The restructure is verified by confirming that every existing workflow produces
the same result as before. No new tests are needed -- the existing test suite
IS the verification.

#### 1. Cargo workspace builds

```bash
cargo build --workspace
```

**Success:** Compiles without errors. Both `tokenoverflow` and
`embedding-service` binaries are produced in `./target/`.

#### 2. All test tiers pass

```bash
# Unit tests (no Docker needed)
cargo test -p tokenoverflow --test unit
cargo test -p embedding-service --test unit

# Integration tests (Docker needed for testcontainers)
cargo test -p tokenoverflow --test integration

# E2E tests (Docker Compose stack needed)
docker compose up -d --build api
cargo test -p tokenoverflow --test e2e
```

**Success:** All tests pass with the same results as before.

#### 3. Docker Compose builds and runs

```bash
docker compose build
docker compose up -d
curl http://localhost:8080/health
```

**Success:** All services start. Health check returns
`{"status":"ok","database":"connected"}`.

#### 4. Pre-commit hooks pass

```bash
prek run --all-files
```

**Success:** All hooks pass: tflint, trivy, cargo-fmt, cargo-clippy,
cargo-coverage, markdownlint, and all standard pre-commit checks.

#### 5. MCP server starts

```bash
TOKENOVERFLOW_CONFIG_DIR=apps/api/config cargo run -p tokenoverflow --release
```

**Success:** Server starts and listens on port 8080.

#### 6. Shell scripts work

```bash
source scripts/src/includes.sh
create_doc design test-doc
# Verify file created at docs/design/YYYY_MM_DD_test-doc.md
rm docs/design/*_test-doc.md
```

**Success:** Document created successfully.

#### 7. `.claude/` config is intact

```bash
# Verify .claude/ is still a real directory (NOT a symlink)
test -d .claude && ! test -L .claude && echo "OK: real directory"
# Verify settings are readable
cat .claude/settings.json
# Verify glob pattern was updated
grep "apps/" .claude/settings.json
```

**Success:** `.claude/` is a real directory at the repo root with updated glob
patterns.

---

## Documentation Changes

### Files to update

All documentation updates are defined in the Logic section above with exact
before/after content. Summary:

| File | Change |
|------|--------|
| `README.md` | Full rewrite with new paths, new architecture diagram, new test commands |
| `docs/terraform.md` | All `src/terraform/` -> `infra/terraform/`, shell script path |
| `apps/api/tests/unit/README.md` | Test command uses `-p tokenoverflow` |
| `apps/api/tests/e2e/README.md` | Test command uses `-p tokenoverflow` |

### Files NOT to update

Historical design documents (`docs/design/2026_01_31_system_design.md`, etc.)
are NOT updated. They are historical records of the codebase at the time they
were written.

### Post-migration user action

The user should update their private `MEMORY.md` at
`~/.claude/projects/-Users-berkay-projects-tokenoverflow/memory/MEMORY.md` to
reflect the new paths:

- `Cargo.toml: src/rust/tokenoverflow/Cargo.toml` ->
  `Cargo.toml: apps/api/Cargo.toml`
- `Config files: src/rust/tokenoverflow/config/` ->
  `Config files: apps/api/config/`
- `MCP server: src/rust/tokenoverflow/src/mcp/` ->
  `MCP server: apps/api/src/mcp/`

---

## Development Environment Changes

### New file: `.cargo/config.toml`

Create a new file at the repo root to set `TOKENOVERFLOW_CONFIG_DIR`
automatically for local development:

```toml
[env]
TOKENOVERFLOW_CONFIG_DIR = { value = "apps/api/config", relative = true }
```

This ensures `cargo run -p tokenoverflow` finds the config files regardless
of the working directory.

### Brewfile

No changes needed. All tools remain the same.

### Setup flow

The `source scripts/src/includes.sh && setup` command continues to work. The
only change is the source path itself, which is documented in the README.

### Environment variables

No new environment variables are introduced. `TOKENOVERFLOW_CONFIG_DIR` already
existed and is now set automatically via `.cargo/config.toml`.

---

## Tasks

Each task is a self-contained unit of work. The engineer should execute them
in order. Each task has a clear success criterion.

### Task 1: Create directory scaffolding

**What:** Create empty target directories on the `main` branch.

**Steps:**

1. Ensure clean working tree on `main`
2. `mkdir -p apps/web apps/claude crates infra/docker scripts/src scripts/tests`
3. `touch apps/web/.gitkeep crates/.gitkeep`
4. `git add apps/web/.gitkeep crates/.gitkeep`

**Success:** New directories exist with `.gitkeep` files staged.

### Task 2: Move Rust apps and workspace

**What:** Move the Cargo workspace root, the API app, and the embedding service
to their new locations.

**Steps:**

1. `git mv src/rust/tokenoverflow apps/api`
2. `git mv src/rust/crates/embedding-service apps/embedding-service`
3. `git mv src/rust/Cargo.toml Cargo.toml`
4. `cp src/rust/Cargo.lock Cargo.lock`
5. Delete orphaned lockfile: `rm -f apps/api/Cargo.lock`
6. Update `Cargo.toml` workspace members to
   `["apps/api", "apps/embedding-service"]`
7. Remove `Cargo.lock` from `.gitignore`
8. `git add Cargo.lock`
9. Create `.cargo/config.toml` with `TOKENOVERFLOW_CONFIG_DIR` setting

**Verify:** `cargo build --workspace` succeeds.

**Success:** Both crates compile from the repo root without `--manifest-path`.

### Task 3: Move Dockerfiles next to their apps

**What:** Co-locate Dockerfiles with the code they build. Move the diesel
tooling container to infra.

**Steps:**

1. `git mv src/docker/api/Dockerfile apps/api/Dockerfile`
2. `git mv src/docker/embedding-service/Dockerfile apps/embedding-service/Dockerfile`
3. `git mv src/docker/diesel infra/docker/diesel`
4. Update `apps/api/Dockerfile` with new COPY paths (see Logic step 2.4)
5. Update `apps/embedding-service/Dockerfile` with new COPY paths (see Logic
   step 2.5)
6. Update `docker-compose.yml` with new build contexts and dockerfile paths
   (see Logic step 2.3)

**Verify:** `docker compose build` succeeds.

**Success:** All four Docker images build successfully.

### Task 4: Move infrastructure

**What:** Move terraform to `infra/terraform/`.

**Steps:**

1. `git mv src/terraform infra/terraform`
2. Update `scripts/src/git_hooks/tflint.sh` paths (see Logic step 2.8)

**Verify:** Internal terraform relative paths (`../../../modules/`) still
resolve correctly because the internal structure is preserved.

**Success:** `tflint` hook runs without errors (if terraform is initialized).

### Task 5: Move shell scripts

**What:** Move scripts from the nested `src/shell/` to flat `scripts/`.

**Steps:**

1. `git mv src/shell/tokenoverflow/includes.sh scripts/src/includes.sh`
2. `git mv src/shell/tokenoverflow/docs.sh scripts/src/docs.sh`
3. `git mv src/shell/tokenoverflow/git_hooks scripts/src/git_hooks`
4. `git mv src/shell/tests/test_docs.sh scripts/tests/test_docs.sh`
5. Update `scripts/src/includes.sh` source path and tg() cd path (see Logic
   step 2.9)
6. Update `scripts/tests/test_docs.sh` source path (see Logic step 2.10)
7. Update `scripts/src/git_hooks/cargo_coverage.sh` manifest path (see Logic
   step 2.7)

**Verify:** `source scripts/src/includes.sh && create_doc design test-verify`
succeeds. Delete the test doc afterwards.

**Success:** Shell functions work with new paths.

### Task 6: Move distributable Claude Code plugin to apps/claude/

**What:** Move the contents of `src/md/` (the distributable Claude Code plugin)
to `apps/claude/`. This is the plugin that external users install -- it is
entirely separate from `.claude/` which is the local project config.

**Steps:**

1. `git mv src/md/instructions.md apps/claude/instructions.md`
2. `git mv src/md/claude/.claude-plugin apps/claude/.claude-plugin`
3. `git mv src/md/claude/.mcp.json apps/claude/.mcp.json`
4. `git mv src/md/claude/agents apps/claude/agents`
5. `git mv src/md/claude/hooks apps/claude/hooks`
6. `git mv src/md/claude/skills apps/claude/skills`

**Verify:** All files are present under `apps/claude/` and `src/md/` is empty.

**Success:** `apps/claude/.claude-plugin/plugin.json` exists and contains the
plugin metadata. `apps/claude/.mcp.json` points to
`https://api.tokenoverflow.io/mcp`.

### Task 7: Update `.claude/` glob patterns

**What:** Update glob patterns in `.claude/settings.json` and
`.claude/rules/rust/code-style.md` to reflect the new Rust source locations.
The files themselves do NOT move -- only their internal path patterns change.

**Steps:**

1. Update `.claude/settings.json` Write matcher from
   `src/rust/**/*.rs` to `apps/**/*.rs` (see Logic step 2.12)
2. Update `.claude/rules/rust/code-style.md` paths from
   `src/api/**/*.rs` to `apps/api/src/**/*.rs` (see Logic step 2.13)

**Verify:** `cat .claude/settings.json | grep apps/` shows the updated pattern.

**Success:** Glob patterns reference the new Rust source locations.

### Task 8: Update remaining config files and documentation

**What:** Update all remaining files that reference old paths.

**Steps:**

1. Update `.pre-commit-config.yaml` (see Logic step 2.6)
2. Update `.mcp.json` (see Logic step 2.11)
3. Update `.agents/skills/design-doc/SKILL.md` (see Logic step 2.14)
4. Rewrite `README.md` (see Logic step 2.18)
5. Update `docs/terraform.md` (see Logic step 2.17)
6. Update `apps/api/tests/unit/README.md` (see Logic step 2.15)
7. Update `apps/api/tests/e2e/README.md` (see Logic step 2.16)

**Success:** All path references in config files and docs point to new
locations.

### Task 9: Clean up and verify

**What:** Remove the empty `src/` tree and run the full verification suite.

**Steps:**

1. Verify nothing tracked remains: `git ls-files src/`
2. Remove empty tree: `rm -rf src/`
3. Stage everything: `git add -A`
4. Run full build: `cargo build --workspace`
5. Run all tests: `cargo test --workspace` (unit tests only; integration
   and e2e require Docker)
6. Run Docker build: `docker compose build`
7. Run Docker stack: `docker compose up -d`
8. Verify health: `curl http://localhost:8080/health`
9. Run integration tests: `cargo test -p tokenoverflow --test integration`
10. Run e2e tests:
    `docker compose up -d --build api &&`
    `cargo test -p tokenoverflow --test e2e`
11. Run pre-commit: `prek run --all-files`
12. If all pass, commit:
    `git commit -m "refactor: restructure monorepo to apps/crates/infra/scripts layout"`

**Success:** All builds, tests, and hooks pass. The restructure is complete.
