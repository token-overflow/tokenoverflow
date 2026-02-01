# Design: GitHub Actions Terraform CD

## Architecture Overview

### Goal

Automate Terraform/Terragrunt deployments via GitHub Actions. The workflow
plans on pull requests (with PR comment output) and applies on merge to main.
Manual dispatch is supported for ad-hoc runs.

### Scope

This design covers:

- GitHub Actions workflow for Terragrunt plan/apply
- OIDC-based authentication from GitHub Actions to AWS (no static credentials)
- New `github_oidc` Terraform module and live configs
- Change detection to only run affected environments
- PR comment with plan output
- Terragrunt/OpenTofu plugin caching
- Modifications to `root.hcl` and `env.hcl` files to remove hardcoded `profile`
- Updates to `scripts/src/includes.sh` for local dev compatibility
- Manual setup steps (GitHub Environments, secrets)

This design does NOT cover:

- Application (Rust/Lambda) CI/CD
- Terraform module tests
- Slack/email notifications
- Pull request approval gates beyond GitHub Environment protection rules

### Environments

Each Terragrunt environment maps to an AWS account and a GitHub Environment:

| Terragrunt env | AWS Account ID   | GitHub Environment | Scope                                  |
|----------------|------------------|--------------------|----------------------------------------|
| `global`       | `058170691494`   | `global`           | org, sso, github_oidc                  |
| `dev`          | `871610744185`   | `dev`              | (empty, ignored for now)               |
| `prod`         | `591120835062`   | `prod`             | vpc, nat, bastion, rds, lambda, api_gw |

### Workflow Triggers

| Trigger              | Action               | Target               |
|----------------------|----------------------|----------------------|
| PR (terraform files) | `terragrunt plan`    | Affected envs only   |
| Merge to main        | `terragrunt apply`   | Affected envs only   |
| `workflow_dispatch`  | `terragrunt apply`   | All envs (sequential)|

### Change Detection Logic

The workflow uses `dorny/paths-filter@v3` to determine which environments need
to run based on changed files:

| Changed path                        | Environments triggered |
|-------------------------------------|------------------------|
| `infra/terraform/live/global/**`    | global                 |
| `infra/terraform/live/prod/**`      | prod                   |
| `infra/terraform/live/dev/**`       | dev (ignored for now)  |
| `infra/terraform/modules/**`        | global + prod          |
| `infra/terraform/live/root.hcl`     | global + prod          |

### Execution Order

Environments run sequentially in dependency order:

```
global -> dev (skipped) -> prod
```

Plan jobs for different environments can run in parallel on PRs since they are
read-only. Apply jobs must be sequential to prevent cross-environment race
conditions.

### Authentication Flow

```
GitHub Actions Runner
  |
  |  OIDC token (JWT with sub claim)
  v
aws-actions/configure-aws-credentials@v4
  |
  |  sts:AssumeRoleWithWebIdentity
  v
IAM Role (per account)
  |  trust policy: token.actions.githubusercontent.com
  |  condition: repo:ozturkberkay/tokenoverflow:environment:<env>
  v
Temporary AWS credentials (env vars)
  |
  v
Terragrunt / OpenTofu (uses AWS credential chain)
```

### Concurrency Control

```yaml
concurrency:
  group: terraform-${{ github.ref }}
  cancel-in-progress: false
```

This prevents concurrent workflow runs on the same branch. `cancel-in-progress:
false` ensures that an in-progress apply is never cancelled mid-run.

### Directory Structure (New Files)

```text
.github/
  workflows/
    terraform_cd.yml
infra/terraform/
  modules/
    github_oidc/
      main.tf
      variables.tf
      outputs.tf
  live/
    global/
      github_oidc/
        terragrunt.hcl
    prod/
      github_oidc/
        terragrunt.hcl
```

---

## Interfaces

### New Files

#### `.github/workflows/terraform_cd.yml`

Full workflow file:

```yaml
name: Terraform CD

on:
  push:
    branches: [main]
    paths:
      - "infra/terraform/**"
  pull_request:
    branches: [main]
    paths:
      - "infra/terraform/**"
  workflow_dispatch:

concurrency:
  group: terraform-${{ github.ref }}
  cancel-in-progress: false

permissions:
  id-token: write
  contents: read
  pull-requests: write

env:
  TOFU_VERSION: "1.11.5"
  TERRAGRUNT_VERSION: "0.99.4"
  TF_PLUGIN_CACHE_DIR: ${{ github.workspace }}/.terraform-plugin-cache

jobs:
  detect-changes:
    name: Detect Changes
    runs-on: ubuntu-latest
    outputs:
      global: ${{ steps.filter.outputs.global }}
      prod: ${{ steps.filter.outputs.prod }}
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Detect changed paths
        uses: dorny/paths-filter@v3
        id: filter
        with:
          filters: |
            global:
              - "infra/terraform/live/global/**"
              - "infra/terraform/modules/**"
              - "infra/terraform/live/root.hcl"
            prod:
              - "infra/terraform/live/prod/**"
              - "infra/terraform/modules/**"
              - "infra/terraform/live/root.hcl"

  plan-global:
    name: Plan (global)
    needs: detect-changes
    if: >-
      needs.detect-changes.outputs.global == 'true'
      || github.event_name == 'workflow_dispatch'
    runs-on: ubuntu-latest
    environment: global
    env:
      AWS_REGION: us-east-1
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: ${{ secrets.AWS_IAM_ROLE_ARN }}
          aws-region: ${{ env.AWS_REGION }}

      - name: Setup OpenTofu
        uses: opentofu/setup-opentofu@v1
        with:
          tofu-version: ${{ env.TOFU_VERSION }}

      - name: Setup Terragrunt
        run: |
          curl -fsSL "https://github.com/gruntwork-io/terragrunt/releases/download/v${TERRAGRUNT_VERSION}/terragrunt_linux_amd64" \
            -o /usr/local/bin/terragrunt
          chmod +x /usr/local/bin/terragrunt

      - name: Create plugin cache directory
        run: mkdir -p "$TF_PLUGIN_CACHE_DIR"

      - name: Cache Terraform plugins
        uses: actions/cache@v4
        with:
          path: ${{ env.TF_PLUGIN_CACHE_DIR }}
          key: ${{ runner.os }}-tf-plugins-global-${{ hashFiles('infra/terraform/live/global/**/.terraform.lock.hcl') }}
          restore-keys: |
            ${{ runner.os }}-tf-plugins-global-

      - name: Terragrunt plan
        id: plan
        working-directory: infra/terraform/live/global
        run: |
          terragrunt run --all plan -no-color -detailed-exitcode 2>&1 \
            | tee "${{ runner.temp }}/plan-global.txt" \
            || true

      - name: Post plan to PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const raw = fs.readFileSync('${{ runner.temp }}/plan-global.txt', 'utf8');
            const plan = raw.length > 60000 ? raw.substring(0, 60000) + '\n... (truncated)' : raw;
            const marker = '<!-- tf-plan-global -->';
            const body = `${marker}\n## Terraform Plan \u2014 global\n\n\`\`\`\n${plan}\n\`\`\``;

            const { data: comments } = await github.rest.issues.listComments({
              owner: context.repo.owner,
              repo: context.repo.repo,
              issue_number: context.issue.number,
            });

            const existing = comments.find(c => c.body.includes(marker));
            if (existing) {
              await github.rest.issues.updateComment({
                owner: context.repo.owner,
                repo: context.repo.repo,
                comment_id: existing.id,
                body,
              });
            } else {
              await github.rest.issues.createComment({
                owner: context.repo.owner,
                repo: context.repo.repo,
                issue_number: context.issue.number,
                body,
              });
            }

  plan-prod:
    name: Plan (prod)
    needs: detect-changes
    if: >-
      needs.detect-changes.outputs.prod == 'true'
      || github.event_name == 'workflow_dispatch'
    runs-on: ubuntu-latest
    environment: prod
    env:
      AWS_REGION: us-east-1
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: ${{ secrets.AWS_IAM_ROLE_ARN }}
          aws-region: ${{ env.AWS_REGION }}

      - name: Setup OpenTofu
        uses: opentofu/setup-opentofu@v1
        with:
          tofu-version: ${{ env.TOFU_VERSION }}

      - name: Setup Terragrunt
        run: |
          curl -fsSL "https://github.com/gruntwork-io/terragrunt/releases/download/v${TERRAGRUNT_VERSION}/terragrunt_linux_amd64" \
            -o /usr/local/bin/terragrunt
          chmod +x /usr/local/bin/terragrunt

      - name: Create plugin cache directory
        run: mkdir -p "$TF_PLUGIN_CACHE_DIR"

      - name: Cache Terraform plugins
        uses: actions/cache@v4
        with:
          path: ${{ env.TF_PLUGIN_CACHE_DIR }}
          key: ${{ runner.os }}-tf-plugins-prod-${{ hashFiles('infra/terraform/live/prod/**/.terraform.lock.hcl') }}
          restore-keys: |
            ${{ runner.os }}-tf-plugins-prod-

      - name: Terragrunt plan
        id: plan
        working-directory: infra/terraform/live/prod
        run: |
          terragrunt run --all plan -no-color -detailed-exitcode 2>&1 \
            | tee "${{ runner.temp }}/plan-prod.txt" \
            || true

      - name: Post plan to PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const raw = fs.readFileSync('${{ runner.temp }}/plan-prod.txt', 'utf8');
            const plan = raw.length > 60000 ? raw.substring(0, 60000) + '\n... (truncated)' : raw;
            const marker = '<!-- tf-plan-prod -->';
            const body = `${marker}\n## Terraform Plan \u2014 prod\n\n\`\`\`\n${plan}\n\`\`\``;

            const { data: comments } = await github.rest.issues.listComments({
              owner: context.repo.owner,
              repo: context.repo.repo,
              issue_number: context.issue.number,
            });

            const existing = comments.find(c => c.body.includes(marker));
            if (existing) {
              await github.rest.issues.updateComment({
                owner: context.repo.owner,
                repo: context.repo.repo,
                comment_id: existing.id,
                body,
              });
            } else {
              await github.rest.issues.createComment({
                owner: context.repo.owner,
                repo: context.repo.repo,
                issue_number: context.issue.number,
                body,
              });
            }

  apply-global:
    name: Apply (global)
    needs: [detect-changes, plan-global]
    if: >-
      github.event_name != 'pull_request'
      && (needs.detect-changes.outputs.global == 'true'
          || github.event_name == 'workflow_dispatch')
    runs-on: ubuntu-latest
    environment: global
    env:
      AWS_REGION: us-east-1
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: ${{ secrets.AWS_IAM_ROLE_ARN }}
          aws-region: ${{ env.AWS_REGION }}

      - name: Setup OpenTofu
        uses: opentofu/setup-opentofu@v1
        with:
          tofu-version: ${{ env.TOFU_VERSION }}

      - name: Setup Terragrunt
        run: |
          curl -fsSL "https://github.com/gruntwork-io/terragrunt/releases/download/v${TERRAGRUNT_VERSION}/terragrunt_linux_amd64" \
            -o /usr/local/bin/terragrunt
          chmod +x /usr/local/bin/terragrunt

      - name: Create plugin cache directory
        run: mkdir -p "$TF_PLUGIN_CACHE_DIR"

      - name: Cache Terraform plugins
        uses: actions/cache@v4
        with:
          path: ${{ env.TF_PLUGIN_CACHE_DIR }}
          key: ${{ runner.os }}-tf-plugins-global-${{ hashFiles('infra/terraform/live/global/**/.terraform.lock.hcl') }}
          restore-keys: |
            ${{ runner.os }}-tf-plugins-global-

      - name: Terragrunt apply
        working-directory: infra/terraform/live/global
        run: terragrunt run --all apply -auto-approve -no-color

  apply-prod:
    name: Apply (prod)
    needs: [detect-changes, plan-prod, apply-global]
    if: >-
      github.event_name != 'pull_request'
      && (needs.detect-changes.outputs.prod == 'true'
          || github.event_name == 'workflow_dispatch')
      && always()
      && (needs.apply-global.result == 'success'
          || needs.apply-global.result == 'skipped')
    runs-on: ubuntu-latest
    environment: prod
    env:
      AWS_REGION: us-east-1
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: ${{ secrets.AWS_IAM_ROLE_ARN }}
          aws-region: ${{ env.AWS_REGION }}

      - name: Setup OpenTofu
        uses: opentofu/setup-opentofu@v1
        with:
          tofu-version: ${{ env.TOFU_VERSION }}

      - name: Setup Terragrunt
        run: |
          curl -fsSL "https://github.com/gruntwork-io/terragrunt/releases/download/v${TERRAGRUNT_VERSION}/terragrunt_linux_amd64" \
            -o /usr/local/bin/terragrunt
          chmod +x /usr/local/bin/terragrunt

      - name: Create plugin cache directory
        run: mkdir -p "$TF_PLUGIN_CACHE_DIR"

      - name: Cache Terraform plugins
        uses: actions/cache@v4
        with:
          path: ${{ env.TF_PLUGIN_CACHE_DIR }}
          key: ${{ runner.os }}-tf-plugins-prod-${{ hashFiles('infra/terraform/live/prod/**/.terraform.lock.hcl') }}
          restore-keys: |
            ${{ runner.os }}-tf-plugins-prod-

      - name: Terragrunt apply
        working-directory: infra/terraform/live/prod
        run: terragrunt run --all apply -auto-approve -no-color
```

#### `infra/terraform/modules/github_oidc/main.tf`

```hcl
resource "aws_iam_openid_connect_provider" "github" {
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = ["ffffffffffffffffffffffffffffffffffffffff"]
}

resource "aws_iam_role" "github-actions" {
  name = "github-actions-terraform"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Federated = aws_iam_openid_connect_provider.github.arn
        }
        Action = "sts:AssumeRoleWithWebIdentity"
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" = "repo:${var.github_repo}:*"
          }
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "admin" {
  role       = aws_iam_role.github-actions.name
  policy_arn = "arn:aws:iam::aws:policy/AdministratorAccess"
}
```

#### `infra/terraform/modules/github_oidc/variables.tf`

```hcl
variable "github_repo" {
  description = "GitHub repository in owner/repo format."
  type        = string
}
```

#### `infra/terraform/modules/github_oidc/outputs.tf`

```hcl
output "role_arn" {
  description = "ARN of the IAM role for GitHub Actions."
  value       = aws_iam_role.github-actions.arn
}

output "oidc_provider_arn" {
  description = "ARN of the OIDC provider."
  value       = aws_iam_openid_connect_provider.github.arn
}
```

#### `infra/terraform/live/global/github_oidc/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/github_oidc"
}

inputs = {
  github_repo = "ozturkberkay/tokenoverflow"
}
```

#### `infra/terraform/live/prod/github_oidc/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/github_oidc"
}

inputs = {
  github_repo = "ozturkberkay/tokenoverflow"
}
```

### Modified Files

#### `infra/terraform/live/root.hcl`

Remove `aws_profile` local, remove `profile` from both `remote_state` backend
config and the generated `provider.tf`. The AWS credential chain handles
authentication: locally via `AWS_PROFILE` env var, in CI via OIDC-injected env
vars.

```hcl
locals {
  aws_region     = "us-east-1"
  env_vars       = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  env_name       = local.env_vars.locals.env_name
  backend_bucket = local.env_vars.locals.backend_bucket
}

remote_state {
  backend = "s3"
  generate = {
    path      = "backend.tf"
    if_exists = "overwrite"
  }
  config = {
    bucket       = local.backend_bucket
    key          = "${trimprefix(path_relative_to_include(), "${local.env_name}/")}/tofu.tfstate"
    region       = local.aws_region
    encrypt      = true
    use_lockfile = true
  }
}

generate "providers" {
  path      = "provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "aws" {
  region = "${local.aws_region}"
}

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "6.33.0"
    }
  }
}
EOF
}
```

#### `infra/terraform/live/global/env.hcl`

Remove `aws_profile`:

```hcl
locals {
  env_name       = "global"
  backend_bucket = "tokenoverflow-terraform-backend-${local.env_name}"
}
```

#### `infra/terraform/live/dev/env.hcl`

Remove `aws_profile`:

```hcl
locals {
  env_name       = "dev"
  backend_bucket = "tokenoverflow-terraform-backend-${local.env_name}"
}
```

#### `infra/terraform/live/prod/env.hcl`

Remove `aws_profile`:

```hcl
locals {
  env_name       = "prod"
  backend_bucket = "tokenoverflow-terraform-backend-${local.env_name}"
}
```

#### `scripts/src/includes.sh` (`tg()` function)

Update the `tg()` function to export `AWS_PROFILE` instead of relying on the
profile in `root.hcl`. The `tg()` function already sets `AWS_PROFILE` on line
49, so the only change needed is removing the `TF_VAR_ssh_public_key` export
(handled by another agent) and ensuring `AWS_PROFILE` is the sole auth
mechanism:

```bash
function tg() {
  local action="$1"
  local env="$2"

  if [ -z "$action" ] || [ -z "$env" ]; then
    echo "Usage: tg <plan|apply> <global/dev/prod>" >&2
    return 1
  fi

  case "$env" in
    global) aws_profile="tokenoverflow-root-admin" ;;
    dev) aws_profile="tokenoverflow-dev-admin" ;;
    prod) aws_profile="tokenoverflow-prod-admin" ;;
    *)
      echo "Unknown environment: $env (expected: global/dev/prod)" >&2
      return 1
      ;;
  esac

  if ! aws sts get-caller-identity --profile "$aws_profile" >/dev/null 2>&1; then
    echo "AWS SSO session expired or missing — logging in..."
    aws sso login --profile "$aws_profile" || return 1
  fi

  export AWS_PROFILE=$aws_profile

  cd "infra/terraform/live/$env" || return 1
  terragrunt run --all "$action"
  cd - >/dev/null || return 1
}
```

---

## Logic

### Phase 1: Create the `github_oidc` Terraform module

**Step 1.1:** Create `infra/terraform/modules/github_oidc/` with `main.tf`,
`variables.tf`, and `outputs.tf` as defined in the Interfaces section.

**Step 1.2:** Create live configs for both accounts:
- `infra/terraform/live/global/github_oidc/terragrunt.hcl`
- `infra/terraform/live/prod/github_oidc/terragrunt.hcl`

Both pass `github_repo = "ozturkberkay/tokenoverflow"` as input.

### Phase 2: Bootstrap OIDC (manual, one-time)

This is a chicken-and-egg problem: the OIDC resources must exist before CI can
authenticate. These steps must be run manually using the current SSO profiles
before the `root.hcl` profile removal.

**Step 2.1:** Apply OIDC in the root account:

```bash
aws sso login --profile tokenoverflow-root-admin
cd infra/terraform/live/global/github_oidc
terragrunt init --backend-bootstrap
terragrunt apply
```

Note the `role_arn` output (e.g.,
`arn:aws:iam::058170691494:role/github-actions-terraform`).

**Step 2.2:** Apply OIDC in the prod account:

```bash
aws sso login --profile tokenoverflow-prod-admin
cd infra/terraform/live/prod/github_oidc
terragrunt init --backend-bootstrap
terragrunt apply
```

Note the `role_arn` output (e.g.,
`arn:aws:iam::591120835062:role/github-actions-terraform`).

### Phase 3: Remove `profile` from `root.hcl` and `env.hcl` files

**Step 3.1:** Apply the changes to `root.hcl`, `global/env.hcl`,
`dev/env.hcl`, and `prod/env.hcl` as defined in the Interfaces section.

**Step 3.2:** Re-initialize all existing live configs to pick up the backend
change (profile removal). This requires the `-reconfigure` flag since the
backend config changed:

```bash
# Global
export AWS_PROFILE=tokenoverflow-root-admin
cd infra/terraform/live/global
terragrunt run --all init -reconfigure

# Prod
export AWS_PROFILE=tokenoverflow-prod-admin
cd infra/terraform/live/prod
terragrunt run --all init -reconfigure
```

**Step 3.3:** Verify zero drift in both environments:

```bash
source scripts/src/includes.sh
tg plan global   # Must show "No changes" for all units
tg plan prod     # Must show "No changes" for all units
```

### Phase 4: Update `scripts/src/includes.sh`

Apply the changes to the `tg()` function as defined in the Interfaces section.
Remove the `TF_VAR_ssh_public_key` export block (another agent is handling
this).

### Phase 5: Create GitHub Environments and secrets

Using the `gh` CLI:

**Step 5.1:** Create GitHub Environments:

```bash
gh api --method PUT "repos/ozturkberkay/tokenoverflow/environments/global"
gh api --method PUT "repos/ozturkberkay/tokenoverflow/environments/prod"
```

**Step 5.2:** Set the `AWS_IAM_ROLE_ARN` secret on each environment:

```bash
gh secret set AWS_IAM_ROLE_ARN \
  --env global \
  --body "arn:aws:iam::058170691494:role/github-actions-terraform"

gh secret set AWS_IAM_ROLE_ARN \
  --env prod \
  --body "arn:aws:iam::591120835062:role/github-actions-terraform"
```

**Step 5.3:** Enable OIDC token permissions on the repository. The workflow
already declares `permissions.id-token: write` at the job level; this just
needs the repository to not restrict it. By default, GitHub allows this for
public repositories. For private repos, ensure the org-level setting allows
OIDC token creation.

### Phase 6: Create the workflow file

Create `.github/workflows/terraform_cd.yml` as defined in the Interfaces
section.

### Phase 7: End-to-end verification

**Step 7.1:** Push the workflow and all changes on a feature branch. Create a
PR. Verify that:

- The `Detect Changes` job correctly identifies the changed environments
- Plan jobs run for affected environments
- Plan output is posted as a PR comment (one per environment, with
  `<!-- tf-plan-global -->` / `<!-- tf-plan-prod -->` markers)
- No apply jobs run on the PR

**Step 7.2:** Merge the PR. Verify that:

- Apply jobs run for affected environments
- Global applies before prod
- Both complete successfully

**Step 7.3:** Trigger a manual `workflow_dispatch` run. Verify that:

- All environments are planned and applied
- Global completes before prod starts

---

## Edge Cases & Constraints

### 1. Chicken-and-egg bootstrap

The OIDC resources must be created manually before CI can work. Phase 2 handles
this by applying the `github_oidc` module using the existing SSO profiles
*before* removing `profile` from `root.hcl`.

### 2. Backend re-initialization after profile removal

Removing `profile` from the `remote_state` backend config changes the backend
configuration. Terragrunt will prompt for re-initialization. Phase 3 handles
this with `terragrunt run --all init -reconfigure` using the `AWS_PROFILE` env
var for authentication.

### 3. `dorny/paths-filter` on first push to a new branch

On the first push to a new branch, `dorny/paths-filter` compares against
`HEAD~1` (not the base branch) for `push` events. For PRs, it compares against
the base branch. This means the first push to main after merging a PR correctly
detects only the files changed in that merge commit.

### 4. Plan output size

GitHub PR comments have a 65536 character limit. The workflow truncates plan
output at 60000 characters to leave room for the markdown wrapper. Large
`terragrunt run --all plan` outputs may be truncated. If this becomes an issue,
the plan can be uploaded as a workflow artifact instead.

### 5. Concurrent applies

The `concurrency` group prevents two workflow runs from applying at the same
time. If a second merge to main happens while the first is applying,
`cancel-in-progress: false` ensures the first apply completes. The second
workflow run queues and starts after the first finishes.

### 6. OIDC trust policy scope

The trust policy uses `StringLike` with `repo:ozturkberkay/tokenoverflow:*` to
allow any branch and any GitHub Environment within this repository. This is
broader than environment-scoped locking (`environment:prod`) but simpler to
manage. The GitHub Environment on each job acts as the authorization boundary.
If tighter scoping is needed later, the `Condition` in the trust policy can be
narrowed to `environment:global` and `environment:prod` respectively.

### 7. `AdministratorAccess` policy

The IAM role uses `AdministratorAccess` because Terragrunt manages a wide range
of AWS resources (VPCs, RDS, Lambda, API Gateway, IAM, S3, etc.). A
least-privilege policy would need to enumerate all resource types and actions
across all modules, which is maintenance-heavy and fragile. This is acceptable
for a single-developer project. For a team, consider creating a scoped IAM
policy.

### 8. Plugin cache concurrency

`TF_PLUGIN_CACHE_DIR` can cause issues with concurrent provider downloads when
`terragrunt run --all` runs multiple modules in parallel. OpenTofu 1.11+ has
improved locking for the plugin cache. If issues arise, add
`--terragrunt-parallelism 1` to the init step.

---

## Test Plan

### Pre-merge verification (on PR)

| Step | Verification | Success criteria |
|---|---|---|
| Change detection | Push a PR that only changes `live/prod/` files | Only the `prod` plan job runs, `global` is skipped |
| Change detection (modules) | Push a PR that changes `modules/` | Both `global` and `prod` plan jobs run |
| Plan output | Check the PR comments | One comment per environment with plan output |
| Comment update | Push another commit to the same PR | Existing comments are updated (not duplicated) |
| No apply on PR | Check workflow jobs | Apply jobs do not run |

### Post-merge verification

| Step | Verification | Success criteria |
|---|---|---|
| Sequential apply | Check job timeline | `apply-global` completes before `apply-prod` starts |
| Successful apply | Check job status | Both apply jobs complete with green checkmarks |
| State consistency | Run `tg plan global` and `tg plan prod` locally | Both show "No changes" |

### Manual dispatch verification

| Step | Verification | Success criteria |
|---|---|---|
| Trigger | Click "Run workflow" on the Actions tab | Workflow starts |
| All envs run | Check job status | Both global and prod plan+apply run |

---

## Documentation Changes

### `README.md`

Add a CI/CD section after the Deployment section:

```markdown
## CI/CD

### Terraform CD

Infrastructure changes are automatically deployed via GitHub Actions:

- **Pull requests:** `terragrunt plan` runs for affected environments. Output
  is posted as a PR comment.
- **Merge to main:** `terragrunt apply` runs sequentially
  (global -> prod).
- **Manual trigger:** Available via the Actions tab for full-stack apply.

Authentication uses OIDC (no static AWS credentials). Each environment maps to
a GitHub Environment with its own IAM role.

| Environment | AWS Account | GitHub Environment |
|---|---|---|
| global | 058170691494 | global |
| prod | 591120835062 | prod |
```

---

## Development Environment Changes

### Profile removal impact

After removing `profile` from `root.hcl`, developers must set `AWS_PROFILE`
before running Terragrunt directly. The `tg()` helper function handles this
automatically. If running Terragrunt manually:

```bash
export AWS_PROFILE=tokenoverflow-prod-admin
cd infra/terraform/live/prod
terragrunt run --all plan
```

### No new tools

No new Homebrew formulas or development tools are required.

---

## Tasks

### Task 1: Create `github_oidc` Terraform module

**Scope:** Create `infra/terraform/modules/github_oidc/` with `main.tf`,
`variables.tf`, and `outputs.tf`.

**Requirements:**

- `main.tf`: OIDC provider for `token.actions.githubusercontent.com`, IAM role
  `github-actions-terraform` with OIDC trust policy scoped to
  `repo:ozturkberkay/tokenoverflow:*`, `AdministratorAccess` policy attachment
- `variables.tf`: Single variable `github_repo` (string)
- `outputs.tf`: `role_arn` and `oidc_provider_arn`
- Follow existing module conventions (kebab-case for resource names)

**Success criteria:** Files exist and follow the existing module patterns.

---

### Task 2: Create `github_oidc` Terragrunt live configs

**Scope:** Create `infra/terraform/live/global/github_oidc/terragrunt.hcl` and
`infra/terraform/live/prod/github_oidc/terragrunt.hcl`.

**Requirements:**

- Both include `root.hcl`, source the `github_oidc` module, and pass
  `github_repo = "ozturkberkay/tokenoverflow"`
- Follow existing unit patterns (see `live/global/org/terragrunt.hcl`)

**Success criteria:** Files exist and match the structure of existing units.

---

### Task 3: Bootstrap OIDC (manual)

**Scope:** Apply the `github_oidc` module in both AWS accounts using SSO.

**Requirements:**

- Apply in root account (058170691494) using `tokenoverflow-root-admin` profile
- Apply in prod account (591120835062) using `tokenoverflow-prod-admin` profile
- Record the `role_arn` output from each account

**Success criteria:** Both `terragrunt apply` succeed. IAM roles and OIDC
providers exist in both accounts.

---

### Task 4: Remove `profile` from `root.hcl` and `env.hcl` files

**Scope:** Modify `root.hcl`, `global/env.hcl`, `dev/env.hcl`,
`prod/env.hcl`.

**Requirements:**

- Remove `aws_profile` local from `root.hcl`
- Remove `profile` from `remote_state` backend config
- Remove `profile` from generated `provider.tf`
- Remove `aws_profile` local from all `env.hcl` files
- Re-initialize all live configs with `-reconfigure`
- Verify zero drift: `tg plan global` and `tg plan prod` show no changes

**Success criteria:** All plans show "No changes" after re-initialization.

---

### Task 5: Update `scripts/src/includes.sh`

**Scope:** Modify the `tg()` function.

**Requirements:**

- Remove the `TF_VAR_ssh_public_key` export block (lines 50-53)
- Keep the `AWS_PROFILE` export (this is now the only auth mechanism)

**Success criteria:** `tg plan prod` works correctly using `AWS_PROFILE`.

---

### Task 6: Set up GitHub Environments and secrets

**Scope:** Create GitHub Environments and set secrets using `gh` CLI.

**Requirements:**

- Create `global` and `prod` GitHub Environments
- Set `AWS_IAM_ROLE_ARN` secret on each environment with the role ARN from
  Task 3

**Success criteria:** `gh api repos/ozturkberkay/tokenoverflow/environments`
shows both environments. Secrets are set.

---

### Task 7: Create the GitHub Actions workflow

**Scope:** Create `.github/workflows/terraform_cd.yml`.

**Requirements:**

- Full workflow as defined in the Interfaces section
- Change detection with `dorny/paths-filter@v3`
- Plan on PR with PR comments, apply on merge to main
- Sequential execution: global before prod
- Plugin caching with `actions/cache@v4`
- Manual dispatch support
- Concurrency control

**Success criteria:** Workflow file passes `actionlint` (if available) and
YAML is valid.

---

### Task 8: Update documentation

**Scope:** Update `README.md` with CI/CD section.

**Requirements:**

- Add CI/CD section as defined in Documentation Changes
- Document the profile removal impact for local development

**Success criteria:** README accurately describes the new CI/CD workflow.

---

### Task 9: End-to-end verification

**Scope:** Push all changes on a feature branch, create a PR, verify plan,
merge, verify apply.

**Requirements:**

- PR triggers plan jobs for affected environments
- Plan output appears as PR comments
- No apply on PR
- Merge triggers sequential apply (global -> prod)
- Manual dispatch works

**Success criteria:** Full workflow runs successfully end-to-end.
