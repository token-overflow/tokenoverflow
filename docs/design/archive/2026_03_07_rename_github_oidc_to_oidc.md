# Design: Rename github_oidc Terraform Module to oidc

## Architecture Overview

This is a pure rename/refactoring of the existing `github_oidc` Terraform module
and its live Terragrunt configurations. No resources are created, modified, or
destroyed. The change is cosmetic at the file/directory level and requires
careful state migration to avoid Terraform seeing the rename as a destroy + create.

### Current Structure

```text
infra/terraform/
├── modules/
│   └── github_oidc/
│       ├── main.tf          # OIDC provider + IAM role + policy attachment
│       ├── variables.tf
│       └── outputs.tf
└── live/
    ├── global/
    │   └── github_oidc/
    │       └── terragrunt.hcl   # source = "../../../modules/github_oidc"
    └── prod/
        └── github_oidc/
            └── terragrunt.hcl   # source = "../../../modules/github_oidc"
```

### Target Structure

```text
infra/terraform/
├── modules/
│   └── oidc/
│       ├── github.tf        # renamed from main.tf
│       ├── variables.tf
│       └── outputs.tf
└── live/
    ├── global/
    │   └── oidc/
    │       └── terragrunt.hcl   # source = "../../../modules/oidc"
    └── prod/
        └── oidc/
            └── terragrunt.hcl   # source = "../../../modules/oidc"
```

### State Key Impact

The S3 backend key is derived from the directory path in `root.hcl`:

```hcl
key = "${trimprefix(path_relative_to_include(), "${local.env_name}/")}/tofu.tfstate"
```

| Environment | S3 Bucket                            | Current Key                  | New Key              |
|-------------|--------------------------------------|------------------------------|----------------------|
| global      | tokenoverflow-terraform-backend-global | `github_oidc/tofu.tfstate` | `oidc/tofu.tfstate` |
| prod        | tokenoverflow-terraform-backend-prod   | `github_oidc/tofu.tfstate` | `oidc/tofu.tfstate` |

Both the state file and its lock file (`*.tflock`) must be moved.

## Interfaces

No interface changes. The module's inputs (`github_repo`, `env_name`) and
outputs (`role_arn`, `oidc_provider_arn`) remain identical. No other modules
reference `github_oidc` as a dependency.

## Logic

No logic changes. The Terraform resources, variables, and outputs are unchanged.
Only file and directory names change.

## Edge Cases & Constraints

| Concern | Mitigation |
|---------|------------|
| Renaming the directory changes the S3 state key, so Terraform would initialize a fresh empty state and plan to create all resources from scratch | Migrate state objects in S3 **before** running `terragrunt init` in the renamed directories |
| S3 native lock files (`*.tflock`) also need to be moved alongside the state | Include lock file in the S3 move operation |
| `.terragrunt-cache` directories contain stale references to the old module path | Delete cache directories after rename |
| Other modules might depend on `github_oidc` outputs via `dependency` blocks | Verified: no dependencies exist |
| CI/CD pipelines might reference the old path | Verified: the Terraform GHA workflow runs Terragrunt commands dynamically; no hardcoded `github_oidc` paths |

## Test Plan

1. After state migration + local rename, run `terragrunt plan` in both
   `live/global/oidc/` and `live/prod/oidc/` environments.
2. **Success criteria:** Both plans show `No changes. Infrastructure is up-to-date.`
3. If plan shows any resource additions or deletions, **stop immediately** and
   restore from the old state key.

## Documentation Changes

None required. The module is not referenced in README.md or any documentation
files.

## Development Environment Changes

None required.

## Tasks

### Task 1: Back Up Current State (Safety Net)

Pull a local backup of both state files before making any changes.

```bash
# From infra/terraform/live/global/github_oidc/
terragrunt state pull > /tmp/oidc_state_global.json

# From infra/terraform/live/prod/github_oidc/
terragrunt state pull > /tmp/oidc_state_prod.json
```

**Success criteria:** Both JSON files are non-empty and contain valid state.

### Task 2: Move S3 State Objects

Move the state files and lock files in S3 to the new key path.

```bash
# Global
aws s3 mv \
  s3://tokenoverflow-terraform-backend-global/github_oidc/tofu.tfstate \
  s3://tokenoverflow-terraform-backend-global/oidc/tofu.tfstate

aws s3 mv \
  s3://tokenoverflow-terraform-backend-global/github_oidc/tofu.tfstate.tflock \
  s3://tokenoverflow-terraform-backend-global/oidc/tofu.tfstate.tflock

# Prod
aws s3 mv \
  s3://tokenoverflow-terraform-backend-prod/github_oidc/tofu.tfstate \
  s3://tokenoverflow-terraform-backend-prod/oidc/tofu.tfstate

aws s3 mv \
  s3://tokenoverflow-terraform-backend-prod/github_oidc/tofu.tfstate.tflock \
  s3://tokenoverflow-terraform-backend-prod/oidc/tofu.tfstate.tflock
```

**Note:** The lock file (`.tflock`) may not exist if no lock is currently held.
It is safe to ignore "not found" errors for the lock file move.

**Success criteria:** State files exist at the new S3 key paths.

### Task 3: Rename Local Directories and Files

```bash
# Rename the module directory
mv infra/terraform/modules/github_oidc infra/terraform/modules/oidc

# Rename main.tf to github.tf inside the module
mv infra/terraform/modules/oidc/main.tf infra/terraform/modules/oidc/github.tf

# Rename the live directories
mv infra/terraform/live/global/github_oidc infra/terraform/live/global/oidc
mv infra/terraform/live/prod/github_oidc infra/terraform/live/prod/oidc
```

**Success criteria:** Old directories no longer exist; new directories contain
all expected files.

### Task 4: Update Terragrunt Source Paths

Update the `source` attribute in both `terragrunt.hcl` files:

**`infra/terraform/live/global/oidc/terragrunt.hcl`:**

```hcl
terraform {
  source = "../../../modules/oidc"
}
```

**`infra/terraform/live/prod/oidc/terragrunt.hcl`:**

```hcl
terraform {
  source = "../../../modules/oidc"
}
```

**Success criteria:** Both files reference `modules/oidc`.

### Task 5: Clear Terragrunt Cache

```bash
rm -rf infra/terraform/live/global/oidc/.terragrunt-cache
rm -rf infra/terraform/live/prod/oidc/.terragrunt-cache
```

**Success criteria:** No `.terragrunt-cache` directories in the renamed
locations.

### Task 6: Validate with Terragrunt Plan

```bash
# From infra/terraform/live/global/oidc/
terragrunt plan
# Expected: No changes.

# From infra/terraform/live/prod/oidc/
terragrunt plan
# Expected: No changes.
```

**Success criteria:** Both plans report no changes.

### Task 7: Clean Up Old Lock Files

Remove the `.terraform.lock.hcl` files from the old location (they were copied
into the renamed directories and will be regenerated by `terragrunt init`):

```bash
rm -f infra/terraform/live/global/oidc/.terraform.lock.hcl
rm -f infra/terraform/live/prod/oidc/.terraform.lock.hcl
```

These will be recreated during `terragrunt init` (which runs as part of
`terragrunt plan`).

**Success criteria:** Lock files are regenerated after running plan.

### Rollback Plan

If `terragrunt plan` shows unexpected changes:

```bash
# 1. Move S3 state back
aws s3 mv \
  s3://tokenoverflow-terraform-backend-global/oidc/tofu.tfstate \
  s3://tokenoverflow-terraform-backend-global/github_oidc/tofu.tfstate

aws s3 mv \
  s3://tokenoverflow-terraform-backend-prod/oidc/tofu.tfstate \
  s3://tokenoverflow-terraform-backend-prod/github_oidc/tofu.tfstate

# 2. Revert local changes
git checkout -- infra/terraform/

# 3. OR restore from backup
cd infra/terraform/live/global/github_oidc && terragrunt state push /tmp/oidc_state_global.json
cd infra/terraform/live/prod/github_oidc && terragrunt state push /tmp/oidc_state_prod.json
```
