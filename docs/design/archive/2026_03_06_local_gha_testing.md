# Design: Local GitHub Actions Testing with act

## Architecture Overview

[nektos/act](https://github.com/nektos/act) (v0.2.84) runs GitHub Actions
workflows locally by spinning up Docker containers that emulate GitHub's runner
environment. It parses workflow YAML, resolves job dependencies, and executes
steps inside containers — providing fast feedback without pushing to GitHub.

### What We're Solving

Both workflows (`terraform.yml` and `deploy_api.yml`) require a push-to-GitHub
round-trip to validate changes. A typical iteration:

1. Edit workflow YAML
2. Commit and push
3. Wait for GitHub to pick up the job (~10-30s)
4. Wait for runner provisioning (~10-60s)
5. Wait for execution (~1-5 min)
6. Read logs, find the error, repeat

With `act`, steps 2-4 are eliminated. Feedback drops from minutes to seconds
for syntax/structure issues and to under a minute for executable steps.

### Scope

| Workflow | Testable Locally | Requires Cloud |
|----------|-----------------|----------------|
| `terraform.yml` — Detect Changes | Yes | No |
| `terraform.yml` — Plan/Apply | Structure only | OIDC + AWS + Terragrunt provider init |
| `deploy_api.yml` — Build steps | Yes (Rust build, cargo-lambda) | No |
| `deploy_api.yml` — Deploy steps | Structure only | OIDC + SSM + S3 + Lambda |

The design splits each workflow into two categories:

1. **Structurally testable** — validate YAML syntax, job/step ordering,
   conditional expressions, environment variable propagation, and action
   resolution. This catches the most common iteration bugs.
2. **Executable locally** — steps that can actually run end-to-end without
   cloud credentials (e.g., `cargo lambda build`, path filtering).

Cloud-dependent steps (OIDC auth, SSM tunnels, S3 uploads, Lambda deploys,
Terragrunt apply) are skipped during local runs using act's event file
mechanism.

### Component Overview

```text
Developer Machine (macOS / Apple Silicon)
├── act CLI (Homebrew)
│   ├── Reads .actrc for default flags
│   ├── Reads .act.secrets for local secrets
│   └── Reads .github/workflows/*.yml
├── Docker (OrbStack) — already installed
│   └── Runner containers (ghcr.io/catthehacker/ubuntu:act-24.04)
├── .github/act/
│   ├── event_push_main.json      # Simulated push event
│   ├── event_pr.json             # Simulated PR event
│   └── event_dispatch.json       # Simulated workflow_dispatch event
└── scripts/src/includes.sh
    └── act_*() helper functions
```

### Decision 1: Runner Image Strategy

The workflows use `ubuntu-slim` (Terraform) and `ubuntu-24.04-arm` (Deploy
API). Neither exists as a Docker image — act needs a mapping.

Available images from
[catthehacker/docker_images](https://github.com/catthehacker/docker_images):

| Variant | Example Tag | User | Description |
|---------|-------------|------|-------------|
| `act` | `ghcr.io/catthehacker/ubuntu:act-24.04` | root | Medium — compatible with most actions, small size |
| `runner` | `ghcr.io/catthehacker/ubuntu:runner-24.04` | runner | Same as `act` but runs as non-root `runner` user |
| `full` | `ghcr.io/catthehacker/ubuntu:full-24.04` | root | Near-complete GitHub runner filesystem dump |

| Approach | Image | Size | Pre-installed Tools | Pros | Cons |
|----------|-------|------|---------------------|------|------|
| **A. Micro** | `node:16-bookworm-slim` | ~50 MB | Node.js only | Fastest pull; minimal disk usage | Most actions fail due to missing tools (no git, curl, etc.) |
| **B. Medium (act)** | `ghcr.io/catthehacker/ubuntu:act-24.04` | ~600 MB | Node.js, git, curl, basic tools | Best balance — compatible with most actions; close to real runner behavior | Missing some tools (no Rust, no uv); installed by workflow steps |
| **C. Full** | `ghcr.io/catthehacker/ubuntu:full-24.04` | ~12 GB | Nearly everything GitHub runners have | Maximum compatibility | Massive download; slow first pull; overkill for workflow validation |
| **D. Self-hosted** | `-self-hosted` | 0 MB (runs on host) | Whatever is on the developer's machine | No Docker overhead; native ARM64 | Less isolation; host OS differences vs CI; side effects on host |

**Recommendation: B (Medium `act` images).** The `act-24.04` image provides the
closest behavior to actual GitHub runners without the massive download of full
images. It includes enough tooling for most actions to resolve and run. Steps
that need Rust or cargo-lambda install them explicitly (via
`dtolnay/rust-toolchain` and `astral-sh/setup-uv` actions), so the base image
doesn't need them pre-installed. Micro images are too stripped-down — even basic
actions like `actions/checkout` need `git`. Full images are unnecessarily large.
Self-hosted mode loses the isolation that makes act useful for catching
CI-specific issues.

We use the `act` variant (root user) rather than `runner` (non-root) to avoid
permission issues with action installers that expect root access.

On Apple Silicon, act runs amd64 containers via Rosetta/QEMU emulation
(OrbStack handles this transparently). The `--container-architecture
linux/amd64` flag is set in `.actrc` to ensure consistent behavior.

### Decision 2: AWS Credential Handling

Both workflows use `aws-actions/configure-aws-credentials` with OIDC
(`id-token: write`). Act does **not** support OIDC — there is no local token
issuer.

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **A. Skip OIDC steps entirely** | Use act's event file (`{"act": true}`) + `if: ${{ !github.event.act }}` guards on AWS steps | Simple; no credentials needed; tests everything except cloud steps | Requires adding conditional guards to workflow YAML |
| **B. Pass local AWS credentials** | Mount `~/.aws` or pass `AWS_*` env vars into the container | Can actually run AWS commands locally | Dangerous — could accidentally run Terragrunt apply or Lambda deploy against prod; credentials leak into containers |
| **C. Replace OIDC action conditionally** | Use act's `--replace` flag or a local action override | Transparent to workflow; no YAML changes | `--replace` is fragile; adds complexity; still risks running cloud commands |

**Recommendation: A (Skip OIDC steps entirely).** The primary value of local
testing is catching structural and build errors — not re-running cloud
deployments. Skipping AWS steps is the safest approach and avoids any risk of
accidental cloud mutations. The conditional guards (`if: ${{
!github.event.act }}`) are a well-documented act pattern and are harmless in
real CI (the `act` field is never present in real GitHub events).

### Decision 3: Configuration File Location

| Approach | Location | Pros | Cons |
|----------|----------|------|------|
| **A. Project root** | `.actrc`, `.act.secrets` | Standard act convention; auto-discovered | Secrets file needs `.gitignore` entry |
| **B. Subdirectory** | `.github/act/.actrc`, `.github/act/.secrets` | Grouped with workflow files | Non-standard; requires `-C` flag or symlink |

**Recommendation: A (Project root).** Act auto-discovers `.actrc` in the
working directory. Placing it at the project root follows the tool's convention
and requires zero extra flags. The `.act.secrets` file is added to
`.gitignore`.

## Interfaces

### New Files

| File | Purpose | Tracked in Git |
|------|---------|----------------|
| `.actrc` | Default act CLI flags | Yes |
| `.act.secrets` | Local secrets (GITHUB_TOKEN, etc.) | No (gitignored) |
| `.act.secrets.example` | Template showing required secret keys | Yes |
| `.github/act/event_push_main.json` | Simulated `push` event for `main` branch | Yes |
| `.github/act/event_pr.json` | Simulated `pull_request` event | Yes |
| `.github/act/event_dispatch.json` | Simulated `workflow_dispatch` event | Yes |

### `.actrc`

```text
--container-architecture linux/amd64
-P ubuntu-slim=ghcr.io/catthehacker/ubuntu:act-24.04
-P ubuntu-24.04-arm=ghcr.io/catthehacker/ubuntu:act-24.04
-P ubuntu-latest=ghcr.io/catthehacker/ubuntu:act-24.04
```

All runners map to the same medium `act-24.04` image. The
`--container-architecture linux/amd64` flag ensures consistent behavior on
Apple Silicon.

### `.act.secrets.example`

```text
GITHUB_TOKEN=
```

Only `GITHUB_TOKEN` is needed (for actions that call the GitHub API). Generate
one with `gh auth token` or use a PAT.

### Event Files

**`.github/act/event_push_main.json`**

```json
{
  "act": true,
  "ref": "refs/heads/main",
  "head_ref": "",
  "base_ref": ""
}
```

**`.github/act/event_pr.json`**

```json
{
  "act": true,
  "action": "opened",
  "pull_request": {
    "head": { "ref": "feature-branch" },
    "base": { "ref": "main" }
  }
}
```

**`.github/act/event_dispatch.json`**

```json
{
  "act": true
}
```

The `"act": true` field enables workflows to detect local runs via
`${{ github.event.act }}`.

### Workflow Modifications

Both workflows need conditional guards on cloud-dependent steps. The guards
use the `act` field from the event payload.

**Pattern:**

```yaml
- name: Configure AWS credentials
  if: ${{ !github.event.act }}
  uses: aws-actions/configure-aws-credentials@...
```

**Steps to guard in `terraform.yml`:**

| Step | Reason |
|------|--------|
| Configure AWS credentials | OIDC not supported |
| Cache OpenTofu providers | `actions/cache` has limited act support |
| Plan (terragrunt-action) | Requires AWS credentials + provider init |
| Apply (terragrunt-action) | Requires AWS credentials |

**Steps to guard in `deploy_api.yml`:**

| Step | Reason |
|------|--------|
| Configure AWS credentials | OIDC not supported |
| Install Session Manager plugin | Not needed without SSM tunnel |
| Establish SSM tunnel | Requires AWS + bastion |
| Run database migrations | Requires tunnel + RDS |
| Upload to S3 | Requires AWS |
| Update Lambda function | Requires AWS |

The remaining steps (Checkout, Install Rust, Install uv, Install cargo-lambda,
Build Lambda artifact) run normally, validating the build pipeline locally.

For `terraform.yml`, the `detect-changes` job (using `dorny/paths-filter`) runs
normally — it only needs git history, not cloud access.

### Shell Helper Functions

Added to `scripts/src/includes.sh`:

```bash
function act_terraform() {
  local event="${1:-push}"
  act "$event" \
    -W .github/workflows/terraform.yml \
    -e ".github/act/event_${event}_main.json" \
    --secret-file .act.secrets
}

function act_deploy() {
  local event="${1:-push}"
  act "$event" \
    -W .github/workflows/deploy_api.yml \
    -e ".github/act/event_${event}_main.json" \
    --secret-file .act.secrets
}

function act_job() {
  local workflow="$1"
  local job="$2"
  local event="${3:-push}"
  act "$event" \
    -W ".github/workflows/${workflow}.yml" \
    -j "$job" \
    -e ".github/act/event_${event}_main.json" \
    --secret-file .act.secrets
}
```

Usage:

```bash
source scripts/src/includes.sh

# Run all terraform workflow jobs (push event)
act_terraform push

# Run all deploy_api workflow jobs (push event)
act_deploy push

# Run a specific job
act_job terraform detect-changes push
act_job deploy_api deploy push

# Run with workflow_dispatch event
act_terraform dispatch
```

## Logic

### Workflow Guard Implementation

The `if: ${{ !github.event.act }}` guard is added to each cloud-dependent step.
This is a no-op in real GitHub Actions because `github.event.act` is always
`undefined` (falsy), so the condition evaluates to `true` and the step runs
normally. In local act runs, the event file sets `"act": true`, making the
condition `false` and skipping the step.

**`terraform.yml` — modified steps (plan-global job shown, same pattern for
plan-prod, apply-global, apply-prod):**

```yaml
plan-global:
  steps:
    - name: Checkout
      uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd

    - name: Configure AWS credentials
      if: ${{ !github.event.act }}
      uses: aws-actions/configure-aws-credentials@8df5847569e6427dd6c4fb1cf565c83acfa8afa7
      with:
        role-to-assume: ${{ secrets.AWS_IAM_ROLE_ARN }}
        aws-region: ${{ env.AWS_REGION }}

    - name: Create plugin cache directory
      run: mkdir -p "$TF_PLUGIN_CACHE_DIR"

    - name: Cache OpenTofu providers
      if: ${{ !github.event.act }}
      uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830
      with:
        # ... unchanged ...

    - name: Plan
      if: ${{ !github.event.act }}
      uses: gruntwork-io/terragrunt-action@5e86476ca61eaf74adb9c0525745f29f921f2199
      with:
        # ... unchanged ...
```

**`deploy_api.yml` — modified steps:**

```yaml
deploy:
  steps:
    - name: Checkout
      uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd

    - name: Configure AWS credentials
      if: ${{ !github.event.act }}
      uses: aws-actions/configure-aws-credentials@8df5847569e6427dd6c4fb1cf565c83acfa8afa7
      with:
        role-to-assume: ${{ secrets.AWS_IAM_ROLE_ARN }}
        aws-region: ${{ env.AWS_REGION }}

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@efa25f7f19611383d5b0ccf2d1c8914531636bf9
      with:
        toolchain: stable

    - name: Install uv
      uses: astral-sh/setup-uv@803947b9bd8e9f986429fa0c5a41c367cd732b41

    - name: Install cargo-lambda
      run: uv tool install cargo-lambda

    - name: Build Lambda artifact
      run: cargo lambda build -p tokenoverflow --release --arm64 --output-format zip --features bundled-libs --compiler cargo

    - name: Install Session Manager plugin
      if: ${{ !github.event.act }}
      uses: ankurk91/install-session-manager-plugin-action@bf762f2baff16807788bb3e3199da1a77f0b6666

    - name: Establish SSM tunnel
      if: ${{ !github.event.act }}
      run: |
        # ... unchanged ...

    - name: Run database migrations
      if: ${{ !github.event.act }}
      env:
        TOKENOVERFLOW_ENV: production
        TOKENOVERFLOW__DATABASE__HOST: localhost
      run: |
        # ... unchanged ...

    - name: Upload to S3
      if: ${{ !github.event.act }}
      run: |
        # ... unchanged ...

    - name: Update Lambda function
      if: ${{ !github.event.act }}
      run: |
        # ... unchanged ...
```

### act Execution Flow

```text
Developer runs: act_deploy push

1. act reads .actrc
   → --container-architecture linux/amd64
   → -P ubuntu-24.04-arm=catthehacker/ubuntu:act-22.04

2. act parses .github/workflows/deploy_api.yml
   → Resolves job graph (single job: deploy)
   → Evaluates trigger (push on main — matches event file)

3. act pulls ghcr.io/catthehacker/ubuntu:act-24.04 (cached after first run)

4. act creates container and runs steps:
   [x] Checkout                      → runs (clones repo into container)
   [ ] Configure AWS credentials     → SKIPPED (github.event.act == true)
   [x] Install Rust toolchain        → runs (downloads rustup + stable)
   [x] Install uv                    → runs
   [x] Install cargo-lambda          → runs
   [x] Build Lambda artifact         → runs (full cargo build)
   [ ] Install Session Manager       → SKIPPED
   [ ] Establish SSM tunnel          → SKIPPED
   [ ] Run database migrations       → SKIPPED
   [ ] Upload to S3                  → SKIPPED
   [ ] Update Lambda function        → SKIPPED

5. act reports success/failure
```

## Edge Cases & Constraints

### Apple Silicon + amd64 Emulation

The `--container-architecture linux/amd64` flag forces amd64 containers on
ARM64 hosts. OrbStack's Rosetta translation handles this efficiently, but some
steps (especially Rust compilation) will be slower than native. This is
acceptable because:

- The Terraform workflow has no heavy compilation steps locally (Terragrunt
  steps are skipped).
- The Deploy API workflow's `cargo lambda build` is the only heavy step. For
  quick structural validation, developers can run with `--job detect-changes` or
  list mode (`act -l`) to skip execution entirely.

### act's Unsupported Features

The following GitHub Actions features used by our workflows are **not supported
by act**:

| Feature | Used In | Impact |
|---------|---------|--------|
| `concurrency` | Both workflows | Ignored — no concurrent run protection locally |
| `job.permissions` | Both workflows | Ignored — no OIDC token scoping |
| `job.environment` | Both workflows | Ignored — environment-scoped secrets not available |
| OIDC (`id-token: write`) | Both workflows | Not implemented — why we skip AWS steps |
| `actions/cache` | Terraform | Limited support — cache doesn't persist across runs |

None of these impact structural validation. The `concurrency` and `permissions`
fields are passthrough configuration that doesn't affect step execution logic.

### First Run Performance

The first `act` invocation pulls the Docker image (~600 MB for medium) and
downloads actions. Subsequent runs use Docker's layer cache and act's action
cache. Expected timings:

| Run | Terraform (detect-changes) | Deploy API (build) |
|-----|----------------------------|---------------------|
| First | ~2 min (image pull + action download) | ~5-8 min (image + Rust toolchain + build) |
| Subsequent | ~5 sec | ~2-3 min (cached image, incremental build not available in container) |

### Docker-in-Docker Actions

`gruntwork-io/terragrunt-action` is a Docker-based action (it runs Terragrunt
inside its own container). Act supports Docker-based actions but requires Docker
socket access. This works with OrbStack but the step is skipped anyway (guarded
by `!github.event.act`), so this is not a concern in practice.

### Secrets File Safety

`.act.secrets` may contain a `GITHUB_TOKEN`. It is gitignored to prevent
accidental commits. The `.act.secrets.example` file documents required keys
without values.

## Test Plan

### Validation After Implementation

| Test | Command | Success Criteria |
|------|---------|------------------|
| act lists terraform jobs | `act -l -W .github/workflows/terraform.yml` | Shows all 5 jobs without errors |
| act lists deploy jobs | `act -l -W .github/workflows/deploy_api.yml` | Shows the deploy job without errors |
| Terraform detect-changes runs | `act_terraform push` | `detect-changes` job completes; plan/apply jobs are skipped (guarded) |
| Deploy API build runs | `act_deploy push` | Checkout, Rust install, uv install, cargo-lambda install, and build steps complete; AWS steps are skipped |
| Guards are no-ops in CI | Push a no-op change to a PR | Both workflows run identically to before (all steps execute, none skipped) |
| Event file triggers correct workflow | `act pull_request -W .github/workflows/terraform.yml -e .github/act/event_pr.json` | Workflow triggers on PR event |
| Helper functions work | `source scripts/src/includes.sh && act_job deploy_api deploy push` | Runs the specified job |

### Regression Safety

The `if: ${{ !github.event.act }}` condition must be verified to be a no-op in
real GitHub Actions. This is guaranteed because:

1. GitHub never sets an `act` field in the event payload.
2. `github.event.act` evaluates to `undefined` → falsy.
3. `!undefined` → `true` → step runs.

This is the same pattern recommended by act's official documentation.

## Documentation Changes

Add a **Local Workflow Testing** subsection to the **Local Development** section
of `README.md`:

````markdown
### Local Workflow Testing

Test GitHub Actions workflows locally using [act](https://github.com/nektos/act):

```bash
source scripts/src/includes.sh

# List all jobs in a workflow
act -l -W .github/workflows/terraform.yml

# Run the Terraform workflow (push event)
act_terraform push

# Run the Deploy API workflow (push event)
act_deploy push

# Run a specific job
act_job terraform detect-changes push

# Run with a PR event
act_terraform pull_request
```

Cloud-dependent steps (AWS auth, Terraform plan/apply, S3 upload, Lambda
deploy) are automatically skipped during local runs. The build and validation
steps run normally.
````

## Development Environment Changes

### Brewfile

Add `act` to the Brewfile:

```ruby
brew "act"
```

### .gitignore

Add the secrets file:

```text
# act (local GitHub Actions testing)
.act.secrets
```

### Setup

No changes to `setup()` in `includes.sh` — `act` is installed via
`brew bundle install` (already part of `setup_brew()`). The helper functions
are available after `source scripts/src/includes.sh`.

## Tasks

### Task 1: Add act to the development toolchain

**Scope:** Modify `Brewfile`, `.gitignore`, and create `.actrc`,
`.act.secrets.example`.

**Requirements:**
- Add `brew "act"` to `Brewfile`
- Create `.actrc` at project root with runner mappings and container
  architecture flag
- Create `.act.secrets.example` with `GITHUB_TOKEN=` placeholder
- Add `.act.secrets` to `.gitignore`

**Success criteria:**
- `brew bundle install --file=Brewfile` installs act
- `act --version` returns v0.2.84 or later
- `.act.secrets` is not tracked by git

---

### Task 2: Create event files

**Scope:** Create `.github/act/event_push_main.json`,
`.github/act/event_pr.json`, `.github/act/event_dispatch.json`.

**Requirements:**
- Each file must include `"act": true`
- Push event must set `ref` to `refs/heads/main`
- PR event must include `pull_request` object with `head` and `base` refs
- Dispatch event needs only the `act` flag

**Success criteria:**
- `act -l -W .github/workflows/terraform.yml -e .github/act/event_push_main.json`
  lists all jobs without parse errors

---

### Task 3: Add act guards to terraform.yml

**Scope:** Modify `.github/workflows/terraform.yml`.

**Requirements:**
- Add `if: ${{ !github.event.act }}` to all AWS credential, cache, and
  Terragrunt action steps across all jobs (plan-global, plan-prod,
  apply-global, apply-prod)
- Do NOT guard: Checkout, Detect Changes job steps, `mkdir` for cache dir
- Preserve all existing logic, conditions, and formatting

**Success criteria:**
- `act push -W .github/workflows/terraform.yml -e .github/act/event_push_main.json`
  completes with guarded steps showing as skipped
- Push the unchanged workflow to a PR — all steps run normally in CI

---

### Task 4: Add act guards to deploy_api.yml

**Scope:** Modify `.github/workflows/deploy_api.yml`.

**Requirements:**
- Add `if: ${{ !github.event.act }}` to: Configure AWS credentials, Install
  Session Manager plugin, Establish SSM tunnel, Run database migrations,
  Upload to S3, Update Lambda function
- Do NOT guard: Checkout, Install Rust toolchain, Install uv, Install
  cargo-lambda, Build Lambda artifact
- Preserve all existing logic and formatting

**Success criteria:**
- `act push -W .github/workflows/deploy_api.yml -e .github/act/event_push_main.json`
  completes with build steps passing and AWS steps skipped
- Push the unchanged workflow to a PR — all steps run normally in CI

---

### Task 5: Add helper functions and update documentation

**Scope:** Modify `scripts/src/includes.sh` and `README.md`.

**Requirements:**
- Add `act_terraform()`, `act_deploy()`, and `act_job()` functions to
  `scripts/src/includes.sh`
- Functions must use `-W`, `-e`, and `--secret-file` flags
- Add "Local Workflow Testing" subsection to README.md under "Local
  Development"
- Document available commands and what gets skipped

**Success criteria:**
- `source scripts/src/includes.sh && act_terraform push` runs the Terraform
  workflow locally
- README accurately describes the local testing workflow
