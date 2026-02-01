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
COPY apps/embedding_service ./apps/embedding_service

# Replace workspace manifest with single-member workspace
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/embedding_service"]\n' > Cargo.toml

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
COPY apps/embedding_service ./apps/embedding_service

# Replace workspace manifest with single-member workspace
RUN printf '[workspace]\nresolver = "2"\nmembers = ["apps/embedding_service"]\n' > Cargo.toml

# Build the binary
RUN cargo build --release -p embedding_service

# =============================================================================
# Stage 4: Runtime - Minimal production image
# =============================================================================
FROM debian:trixie-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary
COPY --from=builder /app/target/release/embedding_service /app/embedding_service

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

CMD ["/app/embedding_service"]
