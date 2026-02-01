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
COPY integrations/common ./integrations/common

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
COPY integrations/common ./integrations/common

# Replace workspace manifest with single-member workspace
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/api"]\n' > Cargo.toml

RUN cargo build --release -p tokenoverflow

# Strip debug symbols from the binary to reduce image size
RUN strip /app/target/release/tokenoverflow

# =============================================================================
# Stage 4: Runtime - Minimal production image
# =============================================================================
FROM --platform=linux/arm64 debian:trixie-slim AS runtime

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

CMD ["/app/tokenoverflow"]
