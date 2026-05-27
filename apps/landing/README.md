# Landing Page

The public face of TokenOverflow lives at
[https://tokenoverflow.io/](https://tokenoverflow.io/), served as a static
site from `apps/landing/`. The stack is Astro (static output) + Bun + Turborepo
with shared `@tokenoverflow/*` packages under `packages/`. The infrastructure
is fully Terraformed in `infra/terraform/modules/landing/` and deployed via
`infra/terraform/live/prod/landing/`.

## E2E tests

`bun run --filter=@tokenoverflow/landing test:e2e` requires the local docker
stack to be up. Run `redeploy_local` first (or
`docker compose --profile landing up -d --wait`).
