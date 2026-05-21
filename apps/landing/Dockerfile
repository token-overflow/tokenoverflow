# =============================================================================
# Stage 1: Builder - Build the Astro static site with Bun
# =============================================================================
FROM oven/bun:1.3.13 AS builder
WORKDIR /app

# Workspace manifests + lockfile for deterministic resolution via bunfig.toml.
# The sibling app's package.json is copied as a stub so bun's workspace
# globber resolves every member the lockfile references; otherwise
# --frozen-lockfile reads the absent member as drift and rejects the install.
COPY package.json bun.lock bunfig.toml turbo.json ./
COPY apps/landing ./apps/landing
COPY apps/web/package.json ./apps/web/package.json
COPY packages ./packages

# Isolated-linker install per bunfig.toml.
RUN bun install --frozen-lockfile

# Build shared packages then the landing app via Turborepo.
RUN bun run build --filter=@tokenoverflow/landing

# =============================================================================
# Stage 2: Runtime - Static Web Server (SWS) serving the static dist
# =============================================================================
# SWS is a single statically-linked Rust binary (Alpine + musl), giving us a
# ~3-5MB runtime layer vs ~37MB for nginx:alpine. Brotli + gzip + HTTP/2 are
# built in, config is a single TOML file, and the container already runs as a
# non-root `sws` user.
FROM joseluisq/static-web-server:2-alpine AS runtime

# The image's default public dir is /home/sws/public (symlinked to /var/public)
# and the `sws` user owns it. Writing straight to /public avoids touching the
# symlink and matches the root path configured in static-web-server.toml.
USER root
RUN mkdir -p /public /etc/sws && chown -R sws:sws /public /etc/sws
USER sws:sws

COPY --from=builder --chown=sws:sws /app/apps/landing/dist /public
COPY --chown=sws:sws apps/landing/static-web-server.toml /etc/sws/config.toml

EXPOSE 4321

# The image's entrypoint prepends `static-web-server` when the first argument
# begins with `-`, so passing the config file flag directly here works.
CMD ["-w", "/etc/sws/config.toml"]
