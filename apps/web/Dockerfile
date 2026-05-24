# =============================================================================
# Stage 1: Builder - Build the TanStack Start app with Bun
# =============================================================================
FROM oven/bun:1.3.13 AS builder
WORKDIR /app

# Workspace manifests + lockfile for deterministic resolution via bunfig.toml.
# The sibling app's package.json is copied as a stub so bun's workspace
# globber resolves every member the lockfile references; otherwise
# --frozen-lockfile reads the absent member as drift and rejects the install.
COPY package.json bun.lock bunfig.toml turbo.json ./
COPY apps/web ./apps/web
COPY apps/landing/package.json ./apps/landing/package.json
COPY packages ./packages

# The committed OpenAPI spec is the only API artifact the web build needs.
# turbo's `gen:api-client` runs hey-api against it without a Rust toolchain.
COPY apps/api/openapi.json ./apps/api/openapi.json

# Isolated-linker install per bunfig.toml.
RUN bun install --frozen-lockfile

# `node-server` preset emits a Node-runnable .output/server/index.mjs whose
# runtime contract matches the prod aws_lambda preset; the only difference is
# how the request loop is driven.
ENV NITRO_PRESET=node-server
# `bun run build:web` routes through turbo so `gen:api-client` runs before
# the vite build; that ordering is what plugs the generated SDK in.
RUN bun run build:web

# =============================================================================
# Stage 2: Runtime - Node 22 runs the prebuilt server bundle
# =============================================================================
FROM node:22-slim AS runtime
WORKDIR /app

# Copy the prebuilt server output. The bundle is self-contained so there is
# no need to ship node_modules.
COPY --from=builder /app/apps/web/.output ./.output

EXPOSE 3000

CMD ["node", ".output/server/index.mjs"]
