# Design: Deploy API GitHub Action

## Architecture Overview

A GitHub Actions workflow that automates API deployment on every merge to
`main` that touches API-related paths. The pipeline builds the Lambda artifact,
runs Diesel database migrations through the bastion host, and deploys the new
code to the `api` Lambda function.

### Trigger Conditions

| Trigger              | Condition                                             |
|----------------------|-------------------------------------------------------|
| `push` to `main`     | Paths: `apps/api/**`, `Cargo.toml`, `Cargo.lock`     |
| `workflow_dispatch`  | Manual trigger (any branch, no path filter)           |

Path filter excludes markdown files (`!**/*.md`) to avoid unnecessary deploys
when only documentation changes.

### Pipeline Flow

```text
┌─────────────────────────────────────────────────────────────────────┐
│                   GitHub Actions Runner (ubuntu-slim)                │
│                         Environment: prod                           │
│                                                                     │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────┐  ┌───────────┐ │
│  │ 1. Build     │  │ 2. Migrate    │  │ 3. S3    │  │ 4. Update │ │
│  │ cargo-lambda │─▶│ diesel CLI    │─▶│ Upload   │─▶│ Lambda    │ │
│  │ (ARM64 ZIP)  │  │ via SSM       │  │          │  │           │ │
│  └──────────────┘  └──────┬────────┘  └──────────┘  └───────────┘ │
│                           │ SSM Port Forward                       │
└───────────────────────────┼─────────────────────────────────────────┘
                            │
                   ┌────────▼─────────┐
                   │  Bastion (SSM)   │
                   │  10.0.10.x       │
                   └────────┬─────────┘
                            │
                   ┌────────▼─────────┐
                   │  PgBouncer       │
                   │  10.0.10.200     │
                   │  port 6432       │
                   └────────┬─────────┘
                            │
                   ┌────────▼─────────┐
                   │  RDS PostgreSQL  │
                   │  port 5432       │
                   └──────────────────┘
```

### Decision 1: Pipeline Structure

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **A. Single job** | All steps sequentially in one job | Simple; shared filesystem; no artifact passing | Serial execution; longer wall time |
| **B. Multi-job** | build + migrate in parallel, then deploy | Faster wall time; clear separation | Artifact upload/download overhead; shared state via SSM tunnel is awkward across jobs |

**Recommendation: A (Single job).** The build (~3 min) and migrate (~10 sec)
are not long enough to justify the complexity of multi-job coordination. A
single job keeps the workflow simple and ensures ordering guarantees (build
→ migrate → deploy) without conditional logic.

**Single-job step order:**

1. Build Lambda artifact (`cargo lambda build`)
2. Establish SSM tunnel through bastion → PgBouncer
3. Run embedded migrations via the built binary
4. Upload ZIP to S3
5. Update Lambda function code
6. Wait for Lambda update to complete

This order ensures: if the build fails, the database is never touched. If
migrations fail, the Lambda is never updated.

### Decision 2: Migration Connection Target

| Approach | Target | Port | Pros | Cons |
|----------|--------|------|------|------|
| **A. Via PgBouncer** | 10.0.10.200 | 6432 | Consistent with app path; validates PgBouncer config | Session-level DDL side-effects in transaction pool mode |
| **B. Direct to RDS** | RDS endpoint | 5432 | Full session semantics; safer for DDL | Bypasses production connection path |

**Recommendation: A (Via PgBouncer).** Diesel migrations run each file as a
single transaction, which is compatible with PgBouncer's `transaction` pool
mode. The database-level `search_path = api, public` (set in the initial
migration) applies to all new connections, so schema resolution works correctly
regardless of pool mode. If a future migration requires session-level
semantics, the tunnel target can be changed to the RDS endpoint without
workflow changes (the bastion SG already allows port 5432 to RDS).

### Decision 3: Migration Strategy

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **A. Embedded migrations** | `diesel_migrations` crate with `embed_migrations!()`. Binary gets a `--migrate` flag. | Single source of truth for config; no extra tooling; no `DATABASE_URL` construction | Requires a code change to the API binary |
| **B. `diesel_cli` + cache** | Install diesel_cli from source, cache binary | Standard approach; no code changes | Needs `libpq-dev`, cache management, hardcoded `DATABASE_URL` in workflow |
| **C. Run SQL via `psql`** | Execute migration files directly | No Rust tooling | Loses Diesel migration tracking (`__diesel_schema_migrations`) |

**Recommendation: A (Embedded migrations).** Using `ubuntu-24.04-arm` as the
runner means the built binary runs natively — no cross-architecture issues. The
binary reuses the app's config loading (`production.toml` + `TOKENOVERFLOW_*`
env vars), eliminating hardcoded connection details in the workflow. This also
removes the need for `diesel_cli`, `libpq-dev`, and cache management.

### Runner

`ubuntu-24.04-arm` — ARM64 runner matches the Lambda target architecture
(`aarch64`). This enables:

1. **Native builds** — `cargo lambda build --arm64` compiles natively instead
   of cross-compiling via Zig, which is faster and avoids Zig toolchain issues
   with C dependencies (`pq-sys`, `openssl-sys`).
2. **Run the binary locally** — The built `bootstrap` binary can be executed
   directly on the runner to run embedded migrations.

### Concurrency & Safety

```yaml
concurrency:
  group: deploy-api
  cancel-in-progress: false
```

Only one deployment runs at a time. Queued runs wait rather than cancelling
in-progress deploys, preventing partial deployments.

### Authentication

Uses the existing GitHub OIDC → AWS IAM role (`github-actions-terraform` with
`AdministratorAccess`) via the `prod` GitHub environment. The role's trust
policy allows `repo:{owner}/{repo}:environment:*`.

## Interfaces

### Workflow File

**Path:** `.github/workflows/deploy_api.yml`

**Triggers:**

```yaml
on:
  push:
    branches: [main]
    paths:
      - "apps/api/**"
      - "Cargo.toml"
      - "Cargo.lock"
      - "!**/*.md"
  workflow_dispatch:
```

The `workflow_dispatch` trigger has no path filter by design — manual runs
always execute the full pipeline regardless of which files changed.

**Permissions:**

```yaml
permissions:
  id-token: write   # OIDC token for AWS
  contents: read    # Checkout
```

**Environment:** `prod` (required by OIDC trust policy:
`repo:{owner}/{repo}:environment:*`).

### GitHub Secrets & Variables

| Name | Type | Source | Purpose |
|------|------|--------|---------|
| `AWS_IAM_ROLE_ARN` | Secret | GitHub environment `prod` | OIDC role ARN (`github-actions-terraform`) |

No additional secrets are needed. The database password is fetched at runtime
from AWS SSM Parameter Store (`/tokenoverflow/prod/database-password`) using
the OIDC role's `AdministratorAccess` permissions.

### AWS Service Interfaces

| Service | Action | Resource | Purpose |
|---------|--------|----------|---------|
| SSM | `ssm:GetParameter` | `/tokenoverflow/prod/database-password` | Retrieve DB password for migration |
| SSM | `ssm:StartSession` | Bastion EC2 instance | Port forward tunnel |
| SSM | `ssmmessages:*` | `*` | SSM data channel for port forwarding |
| AutoScaling | `autoscaling:DescribeAutoScalingGroups` | ASG `bastion` | Discover bastion instance ID |
| S3 | `s3:PutObject` | `tokenoverflow-lambda-prod/api/*` | Upload Lambda ZIP |
| Lambda | `lambda:UpdateFunctionCode` | Function `api` | Deploy new code |
| Lambda | `lambda:GetFunction` | Function `api` | Wait for update (`aws lambda wait function-updated`) |

All covered by the existing `AdministratorAccess` policy on the OIDC role.

### SSM Port Forward Tunnel

The runner establishes a port-forwarding session through the bastion to reach
PgBouncer in the private VPC:

```text
Runner localhost:6432
  → SSM Session (encrypted, over HTTPS)
    → Bastion EC2 (10.0.10.x, private subnet)
      → PgBouncer ENI (10.0.10.200:6432)
        → RDS (database subnet, port 5432)
```

**Command:** Reuses `scripts/src/rds_tunnel.sh` directly. The script targets
PgBouncer (`10.0.10.200:6432`), discovers the bastion instance ID from the
`bastion` ASG, and relies on the environment for AWS credentials (no
`--profile` flag — works with both local `AWS_PROFILE` and CI OIDC).

The workflow runs the script in the background and waits for the tunnel:

```bash
scripts/src/rds_tunnel.sh &

for i in {1..30}; do
  nc -z localhost 6432 2>/dev/null && exit 0
  sleep 1
done

echo "Tunnel failed to establish"
exit 1
```

**Prerequisites on the runner:**
- AWS Session Manager plugin (`session-manager-plugin`) must be installed
- `nc` (netcat) for port readiness check (pre-installed on Ubuntu runners)

### Migration Execution

Migrations run via the built API binary's `--migrate` flag. The binary uses the
same config loading as the app: `production.toml` + `TOKENOVERFLOW_*` env var
overrides. Only two overrides are needed in CI:

| Env Var | Value | Why |
|---------|-------|-----|
| `TOKENOVERFLOW_DATABASE_PASSWORD` | From SSM at runtime | Password is never in config files |
| `TOKENOVERFLOW__DATABASE__HOST` | `localhost` | Override PgBouncer IP to tunnel endpoint |

All other connection details (user, port, database name, search_path) come
from `production.toml` and the database-level `ALTER DATABASE ... SET
search_path` — same as the running Lambda.

## Logic

### Complete Workflow

```yaml
name: Deploy API

on:
  push:
    branches: [main]
    paths:
      - "apps/api/**"
      - "Cargo.toml"
      - "Cargo.lock"
      - "!**/*.md"
  workflow_dispatch:

concurrency:
  group: deploy-api
  cancel-in-progress: false

permissions:
  id-token: write
  contents: read

env:
  AWS_REGION: us-east-1

jobs:
  deploy:
    name: Deploy
    runs-on: ubuntu-24.04-arm
    environment: prod
    steps:
      - name: Checkout
        uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd  # v6.0.2

      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@8df5847569e6427dd6c4fb1cf565c83acfa8afa7  # v6.0.0
        with:
          role-to-assume: ${{ secrets.AWS_IAM_ROLE_ARN }}
          aws-region: ${{ env.AWS_REGION }}

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@631a55b12751854ce901bb631d5902ceb48146f7  # stable
        with:
          toolchain: stable

      - name: Install cargo-lambda
        run: uv tool install cargo-lambda

      - name: Install Session Manager plugin
        uses: ankurk91/install-session-manager-plugin-action@v1

      - name: Build Lambda artifact
        run: cargo lambda build -p tokenoverflow --release --arm64 --output-format zip

      - name: Establish SSM tunnel
        run: |
          scripts/src/rds_tunnel.sh &

          for i in {1..30}; do
            nc -z localhost 6432 2>/dev/null && exit 0
            sleep 1
          done

          echo "Tunnel failed to establish"
          exit 1

      - name: Run database migrations
        env:
          TOKENOVERFLOW_ENV: production
          TOKENOVERFLOW__DATABASE__HOST: localhost
        run: |
          TOKENOVERFLOW_DATABASE_PASSWORD=$(aws ssm get-parameter \
            --name /tokenoverflow/prod/database-password \
            --with-decryption \
            --query Parameter.Value \
            --output text)
          export TOKENOVERFLOW_DATABASE_PASSWORD

          ./target/lambda/tokenoverflow/bootstrap --migrate

      - name: Upload to S3
        run: |
          SHA=$(shasum -a 256 target/lambda/tokenoverflow/bootstrap.zip | cut -c1-12)
          echo "LAMBDA_S3_KEY=api/${SHA}.zip" >> "$GITHUB_ENV"

          aws s3 cp target/lambda/tokenoverflow/bootstrap.zip \
            "s3://tokenoverflow-lambda-prod/api/${SHA}.zip"

      - name: Update Lambda function
        run: |
          aws lambda update-function-code \
            --function-name api \
            --s3-bucket tokenoverflow-lambda-prod \
            --s3-key "$LAMBDA_S3_KEY" \
            --architectures arm64

          aws lambda wait function-updated \
            --function-name api
```

**Note:** `ankurk91/install-session-manager-plugin-action` is a new action.
Its exact commit hash must be pinned during implementation per
`.github/workflows/AGENTS.md`.

### Step-by-Step Explanation

1. **Checkout** — Clones the repository.
2. **Configure AWS credentials** — Assumes the `github-actions-terraform` IAM
   role via OIDC. Sets `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and
   `AWS_SESSION_TOKEN` as environment variables for all subsequent steps.
3. **Install Rust toolchain** — Installs the stable Rust toolchain (`rustc`,
   `cargo`).
4. **Install cargo-lambda** — Installs via `uv tool install` (uv is
   pre-installed on ubuntu-24.04 runners). The PyPI package is published by
   the cargo-lambda authors and downloads a pre-built binary.
5. **Install Session Manager plugin** — Installs the AWS CLI Session Manager
   plugin, required for `aws ssm start-session` in the tunnel step.
6. **Build Lambda artifact** — Builds the API for ARM64 Linux. On the
   `ubuntu-24.04-arm` runner this is a native build (no cross-compilation).
   Static linking via `pq-sys` bundled + `openssl-sys` vendored produces a
   self-contained `bootstrap` binary. Output:
   `target/lambda/tokenoverflow/bootstrap.zip`.
7. **Establish SSM tunnel** — Runs `rds_tunnel.sh` in background. Polls
   `localhost:6432` with `nc` until PgBouncer is reachable (30 second timeout).
   Background process persists across subsequent steps.
8. **Run database migrations** — Retrieves the database password from SSM,
   sets the env vars the app's config loader expects, and runs the built binary
   with `--migrate`. The binary loads `production.toml`, overrides host to
   `localhost` (tunnel), connects to PgBouncer, and runs pending migrations.
9. **Upload to S3** — Computes a 12-character SHA256 prefix of the ZIP and
   uploads to `s3://tokenoverflow-lambda-prod/api/{SHA}.zip`. Stores the key
   in `GITHUB_ENV` for the next step.
10. **Update Lambda function** — Points the `api` Lambda at the new S3 key and
    waits for the update to reach `Successful` state via
    `aws lambda wait function-updated`.

## Edge Cases & Constraints

### Bastion Instance Unavailable

The `rds_tunnel.sh` script exits with an error if no in-service instance is
found in the `bastion` ASG:

```bash
if [ -z "$INSTANCE_ID" ] || [ "$INSTANCE_ID" = "None" ]; then
  echo "Error: No bastion instance found in ASG 'bastion'." >&2
  exit 1
fi
```

This can happen during ASG instance replacement (spot interruption, capacity
rebalancing). The ASG is configured with `min_size = 1`, so recovery is
automatic. Re-running the workflow resolves this.

### SSM Tunnel Timeout

The tunnel wait loop gives 30 seconds for the SSM session to establish. If the
bastion is slow to respond (e.g., SSM agent starting after a fresh launch), the
step fails. This is a safe failure — no database or Lambda changes have been
made at this point.

### Migration Failure

If `diesel migration run` fails (SQL error, network issue), the workflow stops
before the Lambda deploy step. Diesel runs each migration inside a transaction,
so a failed migration is rolled back — the database remains in its
pre-migration state.

**Important:** Migrations must be backwards-compatible with the currently
deployed Lambda code. The deploy order (migrate then deploy) means the old
Lambda code continues to serve traffic with the new schema until the Lambda
update completes. Breaking schema changes require a two-phase deployment:

1. Deploy a Lambda version that supports both old and new schema.
2. Run the migration.
3. Deploy the final Lambda version that drops old schema support.

### Lambda Update In Progress

`aws lambda wait function-updated` blocks until the function reaches the
`Successful` state. If another update is already in progress,
`update-function-code` fails with `ResourceConflictException`. The workflow's
`concurrency` group prevents this from happening within CI, but a manual CLI
deploy could conflict. Re-running the workflow resolves it.

### PgBouncer Transaction Mode and DDL

PgBouncer runs in `transaction` pool mode. `diesel_migrations` runs each
migration inside a transaction, so PgBouncer keeps a stable server connection
for the duration of each migration file. The database-level default
`search_path = api, public` (set by `ALTER DATABASE` in the initial migration)
applies to all new connections automatically, so schema resolution works
correctly.

If a future migration requires session-level semantics that span multiple
transactions, the tunnel target can be changed to the RDS endpoint
(`main.ccp4e4gum1b0.us-east-1.rds.amazonaws.com:5432`) without workflow
changes — the bastion SG already allows port 5432 to RDS.

### Identical Artifact Re-deploy

If a merge contains no code changes to the API binary (e.g., only migration
files changed), the build produces the same ZIP. The SHA-based S3 key means
the upload overwrites the same object (S3 versioning preserves previous
versions). The Lambda `update-function-code` call is idempotent — pointing to
the same S3 key is a no-op from a runtime perspective.

### `--migrate` Flag and the Lambda Runtime

The `--migrate` flag must run migrations and exit before the normal server
startup logic. In CI, `AWS_LAMBDA_RUNTIME_API` is not set, so the binary would
fall into the TCP listener path after migrating if not explicitly exited. The
implementation must exit after migrations complete.

## Test Plan

### Initial Validation

| Step | Method | Success Criteria |
|------|--------|------------------|
| Path trigger works | Push a commit touching `apps/api/` to `main` | Workflow starts automatically |
| Path filter excludes non-API changes | Push a commit touching only `docs/` | Workflow does not start |
| Manual trigger works | Run via Actions UI → `workflow_dispatch` | Workflow completes successfully |
| Build succeeds | Check "Build Lambda artifact" step log | Exits 0, log shows `bootstrap.zip` produced |
| SSM tunnel establishes | Check "Establish SSM tunnel" step log | Exits 0 within 30 seconds |
| Migrations run | Check "Run database migrations" step log | Exits 0 |
| S3 upload succeeds | `aws s3 ls s3://tokenoverflow-lambda-prod/api/` | New `{SHA}.zip` object present |
| Lambda updates | Check "Update Lambda function" step log | `wait function-updated` completes |
| Health check passes | `curl https://api.tokenoverflow.io/health` | Returns `{"status":"ok","database":"connected"}` |

### Rollback Verification

| Step | Method | Success Criteria |
|------|--------|------------------|
| Re-deploy previous version | Trigger `workflow_dispatch` from an older commit | Lambda serves the older code; health check passes |

## Documentation Changes

Update the **Deployment** section of `README.md` to document the CI/CD
workflow:

```markdown
## Deployment

Merges to `main` that change `apps/api/**`, `Cargo.toml`, or `Cargo.lock`
automatically trigger the **Deploy API** workflow
(`.github/workflows/deploy_api.yml`). The workflow:

1. Builds the Lambda artifact (`cargo lambda build`)
2. Runs database migrations via SSM tunnel to PgBouncer
3. Uploads the artifact to S3 and updates the Lambda function

Manual deploys can be triggered via `workflow_dispatch` in the GitHub Actions
UI.
```

Keep the existing manual deployment commands (`cargo lambda build`, `aws s3 cp`,
`aws lambda update-function-code`) below the CI/CD section as a reference.

## Development Environment Changes

None. The workflow runs entirely in CI. Local development is unchanged
(`docker compose up -d`).

## Tasks

### Task 1: Add embedded migrations to the API binary

**Scope:** Modify `apps/api/Cargo.toml` and `apps/api/src/main.rs`.

**Requirements:**
- Add `diesel_migrations` crate to `apps/api/Cargo.toml`
- Use `embed_migrations!("migrations")` to embed all migration SQL files
- Add `--migrate` CLI flag handling in `main.rs`:
    - Parse `std::env::args()` for `--migrate`
    - Load config (same `Config::load()` as the server)
    - Establish a single `PgConnection` using the loaded config
    - Run `conn.run_pending_migrations(MIGRATIONS)`
    - Log the result and exit
- The migration connection must use the same config loading as the app
  (`production.toml` + `TOKENOVERFLOW_*` env var overrides)
- Do NOT change the existing server startup path — `--migrate` is a separate
  branch that exits after completion

**Success criteria:**
- `cargo test -p tokenoverflow --test unit` passes
- `docker compose up -d` then
  `./target/debug/tokenoverflow --migrate` runs with no pending migrations
  (idempotent against the local stack)

---

### Task 2: Create the Deploy API workflow

**Scope:** Create `.github/workflows/deploy_api.yml`.

**Requirements:**
- Full workflow YAML as specified in the Logic section
- Pin all new action commit hashes per `.github/workflows/AGENTS.md`
- Reuse existing action hashes for `actions/checkout`,
  `aws-actions/configure-aws-credentials`
- Use `ubuntu-24.04-arm` runner
- Use `prod` GitHub environment
- Install cargo-lambda via `uv tool install cargo-lambda`

**Success criteria:** `workflow_dispatch` on `main` completes all steps. Health
check at `https://api.tokenoverflow.io/health` returns
`{"status":"ok","database":"connected"}`.

---

### Task 3: Update README.md

**Scope:** Update the Deployment section of `README.md`.

**Requirements:**
- Document the CI/CD workflow trigger and steps
- Keep existing manual deployment commands as a reference
- Mention `workflow_dispatch` for manual re-deploys

**Success criteria:** Deployment section accurately describes both automated
and manual deployment paths.
