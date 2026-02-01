# Design: aws-vpc-networking

## Architecture Overview

### Goal

Provision an enterprise-grade, 3-tier AWS VPC for the production environment
using the `terraform-aws-modules/vpc/aws` community module, orchestrated by
Terragrunt. The directory structure must support adding a dev VPC later without
code duplication.

### Scope

This design covers:

- Production VPC with 3-tier subnet architecture (public, private, isolated)
- Terragrunt unit and module configuration
- CIDR addressing plan
- Provider migration from `opentofu/aws` to `hashicorp/aws`

This design does NOT cover:

- Dev VPC (deferred, but directory structure supports it)
- NAT Gateways (deferred, but the design accommodates adding them later)
- Internet Gateway (deferred)
- VPC Flow Logs (deferred)
- VPC Peering or Transit Gateway
- Security Groups (separate concern, will be defined per-service)

### CIDR Addressing Plan

Each environment gets a `/16` block, giving 65,536 IP addresses per VPC. The
blocks are non-overlapping to allow future VPC peering between environments.

| Environment | VPC CIDR       | Status    |
|-------------|----------------|-----------|
| prod        | `10.0.0.0/16`  | In scope  |
| dev         | `10.1.0.0/16`  | Deferred  |

#### Subnet Layout (prod: `10.0.0.0/16`)

Subnets are carved from the VPC CIDR using `/24` blocks (254 usable IPs each).
Public subnets use a smaller `/24` because they will only host load balancers
and NAT Gateways (when added). Private and isolated subnets also use `/24`,
which is sufficient for the current workload. The addressing leaves large gaps
for future expansion.

| Tier     | Purpose                          | AZ           | CIDR             | Usable IPs |
|----------|----------------------------------|--------------|------------------|------------|
| Public   | ALB, NAT GW (future), IGW        | us-east-1a   | `10.0.0.0/24`    | 251        |
| Public   | ALB, NAT GW (future), IGW        | us-east-1b   | `10.0.1.0/24`    | 251        |
| Private  | App servers (ECS, EC2, Lambda)    | us-east-1a   | `10.0.10.0/24`   | 251        |
| Private  | App servers (ECS, EC2, Lambda)    | us-east-1b   | `10.0.11.0/24`   | 251        |
| Isolated | Databases (RDS, ElastiCache)      | us-east-1a   | `10.0.20.0/24`   | 251        |
| Isolated | Databases (RDS, ElastiCache)      | us-east-1b   | `10.0.21.0/24`   | 251        |

The numbering scheme uses `0-1` for public, `10-11` for private, and `20-21`
for isolated. This leaves room for `2-9`, `12-19`, and `22+` for future AZs or
additional subnet tiers.

### Network Topology

```text
                    Internet
                       |
                   (no IGW yet)
                       |
              +--------+--------+
              |    VPC 10.0.0.0/16    |
              |                       |
  +-----------+-----------+-----------+-----------+
  |                       |                       |
  | Tier 1: Public        | Tier 1: Public        |
  | 10.0.0.0/24           | 10.0.1.0/24           |
  | us-east-1a            | us-east-1b            |
  | (ALB, NGW future)     | (ALB, NGW future)     |
  +--------+--------------+--------+--------------+
           |                       |
  +--------+--------------+--------+--------------+
  |                       |                       |
  | Tier 2: Private       | Tier 2: Private       |
  | 10.0.10.0/24          | 10.0.11.0/24          |
  | us-east-1a            | us-east-1b            |
  | (App servers)         | (App servers)         |
  +--------+--------------+--------+--------------+
           |                       |
  +--------+--------------+--------+--------------+
  |                       |                       |
  | Tier 3: Isolated      | Tier 3: Isolated      |
  | 10.0.20.0/24          | 10.0.21.0/24          |
  | us-east-1a            | us-east-1b            |
  | (RDS, ElastiCache)    | (RDS, ElastiCache)    |
  +-----------------------+-----------------------+
```

**Connectivity rules:**

- Public subnets: Will have a route to IGW (when added). Currently no route.
- Private subnets: Will have a route to NAT GW (when added) for outbound
  internet. Currently no route.
- Isolated subnets: Never get a route to the internet. Only accessible from
  private subnets via security groups.

### Provider Compatibility Decision

The existing infrastructure uses `opentofu/aws` (v6.21.0) as the provider
source in `root.hcl`. The `terraform-aws-modules/vpc/aws` v6.6.0 module
declares `hashicorp/aws >= 6.28` in its `required_providers`.

OpenTofu treats `opentofu/aws` and `hashicorp/aws` as two completely different
providers because they have different namespaces. When a root module declares
`opentofu/aws` but a child module declares `hashicorp/aws`, the child module
gets an unconfigured provider instance -- the root's region, profile, and
assume_role settings are not inherited. This leads to authentication failures.

Per the OpenTofu team (GitHub issue #1189, closed as "not planned"), there is
no provider source remapping feature. Their official recommendation is: "Use
`hashicorp/aws` consistently. There is generally no particular reason to use
`opentofu/aws` right now."

When you write `hashicorp/aws` in an OpenTofu config, it resolves to
`registry.opentofu.org/hashicorp/aws` -- it still downloads from the OpenTofu
registry, not HashiCorp's registry. The provider binary is identical.

**Decision:** Switch the provider source in `root.hcl` from `opentofu/aws` to
`hashicorp/aws` and bump the version to `6.33.0` (latest as of 2026-02-19).

| Approach | Provider conflict? | Community module compatible? | Recommended by OpenTofu? |
|----------|--------------------|------------------------------|--------------------------|
| Keep `opentofu/aws` | Yes -- modules declare `hashicorp/aws` | No -- requires duplicate provider blocks | No |
| Switch to `hashicorp/aws` | No | Yes | Yes |

This switch requires a one-time `state replace-provider` migration for
existing units (`aws-organizations`, `aws-sso`) to update the provider
reference in their state files. See Logic Phase 1 for exact steps.

### Module Strategy: Wrapper Module vs Direct Registry

Two approaches exist for integrating the community VPC module with Terragrunt.

**Option A: Wrapper module (local `modules/vpc/` that calls the community module)**

```text
infra/terraform/
  modules/vpc/
    main.tf          # module "vpc" { source = "terraform-aws-modules/vpc/aws" }
    variables.tf     # env_name, vpc_cidr, etc.
    outputs.tf
  live/prod/vpc/
    terragrunt.hcl   # source = "../../../modules/vpc", inputs = { ... }
```

#### Option B: Direct registry reference from the Terragrunt unit

```text
infra/terraform/
  live/prod/vpc/
    terragrunt.hcl   # source = "tfr:///terraform-aws-modules/vpc/aws?version=6.6.0"
                     # inputs = { all VPC variables directly }
```

| Criteria | Option A: Wrapper | Option B: Direct |
|----------|-------------------|------------------|
| Consistency with existing pattern | Matches `aws-organizations`, `aws-sso` | Different pattern |
| Ability to add custom resources | Can add outputs, data sources, locals | Cannot -- limited to module inputs |
| Future extensibility | Can add IGW, NAT GW, Flow Logs resources alongside | Would need to switch to wrapper later |
| Code to maintain | More files (main.tf, variables.tf, outputs.tf) | Less files (just terragrunt.hcl) |
| Version pinning | In main.tf `version = "6.6.0"` | In terragrunt.hcl `?version=6.6.0` |

**Decision:** Option A (wrapper module). This is consistent with the existing
project pattern where all Terragrunt units reference local modules. It also
provides a natural place to add custom resources (like VPC endpoints, flow logs,
or NAT Gateways) alongside the community module in future iterations without
restructuring.

### Directory Structure (After)

```text
infra/terraform/
  modules/
    aws-organizations/       # existing
    aws-sso/                 # existing
    vpc/                     # new
      main.tf
      variables.tf
      outputs.tf
  live/
    root.hcl                 # shared config (provider, backend)
    global/
      env.hcl
      aws-organizations/
        terragrunt.hcl       # existing
      aws-sso/
        terragrunt.hcl       # existing
    prod/
      env.hcl
      vpc/
        terragrunt.hcl       # new
    dev/
      env.hcl
      vpc/                   # future (not created now, but structure supports it)
        terragrunt.hcl
```

---

## Interfaces

This section documents every file that will be created or modified, and the
contracts between them.

### New Files

#### `infra/terraform/modules/vpc/variables.tf`

```hcl
variable "env_name" {
  description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
  type        = string
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC."
  type        = string
}

variable "azs" {
  description = "List of availability zones."
  type        = list(string)
}

variable "public_subnets" {
  description = "List of CIDR blocks for public subnets (one per AZ)."
  type        = list(string)
}

variable "private_subnets" {
  description = "List of CIDR blocks for private subnets (one per AZ)."
  type        = list(string)
}

variable "database_subnets" {
  description = "List of CIDR blocks for isolated/database subnets (one per AZ)."
  type        = list(string)
}
```

#### `infra/terraform/modules/vpc/main.tf`

```hcl
module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "6.6.0"

  name = var.env_name
  cidr = var.vpc_cidr
  azs  = var.azs

  # Tier 1: Public subnets
  public_subnets = var.public_subnets
  public_subnet_names = [
    for i, az in var.azs : "${var.env_name}-public-${az}"
  ]

  # Tier 2: Private subnets
  private_subnets = var.private_subnets
  private_subnet_names = [
    for i, az in var.azs : "${var.env_name}-private-${az}"
  ]

  # Tier 3: Isolated subnets (databases)
  database_subnets = var.database_subnets
  database_subnet_names = [
    for i, az in var.azs : "${var.env_name}-isolated-${az}"
  ]

  # Database subnet group for RDS
  create_database_subnet_group       = true
  database_subnet_group_name         = var.env_name
  create_database_subnet_route_table = true

  # No internet access for database subnets (isolated)
  create_database_internet_gateway_route = false
  create_database_nat_gateway_route      = false

  # No NAT Gateway (deferred)
  enable_nat_gateway = false

  # DNS
  enable_dns_hostnames = true
  enable_dns_support   = true

  # Tags
  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }

  public_subnet_tags = {
    Tier = "public"
  }

  private_subnet_tags = {
    Tier = "private"
  }

  database_subnet_tags = {
    Tier = "isolated"
  }
}
```

#### `infra/terraform/modules/vpc/outputs.tf`

```hcl
output "vpc_id" {
  description = "The ID of the VPC."
  value       = module.vpc.vpc_id
}

output "vpc_cidr_block" {
  description = "The CIDR block of the VPC."
  value       = module.vpc.vpc_cidr_block
}

output "public_subnet_ids" {
  description = "List of public subnet IDs."
  value       = module.vpc.public_subnets
}

output "private_subnet_ids" {
  description = "List of private subnet IDs."
  value       = module.vpc.private_subnets
}

output "database_subnet_ids" {
  description = "List of database (isolated) subnet IDs."
  value       = module.vpc.database_subnets
}

output "database_subnet_group_name" {
  description = "Name of the database subnet group."
  value       = module.vpc.database_subnet_group_name
}

output "public_route_table_ids" {
  description = "List of public route table IDs."
  value       = module.vpc.public_route_table_ids
}

output "private_route_table_ids" {
  description = "List of private route table IDs."
  value       = module.vpc.private_route_table_ids
}

output "database_route_table_ids" {
  description = "List of database route table IDs."
  value       = module.vpc.database_route_table_ids
}
```

#### `infra/terraform/live/prod/vpc/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/vpc"
}

inputs = {
  env_name = "prod"
  vpc_cidr = "10.0.0.0/16"
  azs      = ["us-east-1a", "us-east-1b"]

  public_subnets   = ["10.0.0.0/24", "10.0.1.0/24"]
  private_subnets  = ["10.0.10.0/24", "10.0.11.0/24"]
  database_subnets = ["10.0.20.0/24", "10.0.21.0/24"]
}
```

### Modified Files

#### `infra/terraform/live/root.hcl`

**Change:** Update the provider source from `opentofu/aws` to `hashicorp/aws`
and bump the version from `6.21.0` to `6.33.0`. Only the `generate "providers"`
block changes. All other content (locals, remote_state) remains unchanged.

Full updated file:

```hcl
locals {
  aws_region     = "us-east-1"
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  aws_profile    = local.env_vars.locals.aws_profile
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
    profile      = local.aws_profile
    encrypt      = true
    use_lockfile = true
  }
}

generate "providers" {
  path      = "provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "aws" {
  region  = "${local.aws_region}"
  profile = "${local.aws_profile}"
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

### Files NOT Modified

| File | Reason |
|------|--------|
| `live/global/env.hcl` | No changes needed |
| `live/prod/env.hcl` | No changes needed |
| `live/dev/env.hcl` | No changes needed |
| `live/global/aws-organizations/terragrunt.hcl` | No changes needed (state migration handled by CLI) |
| `live/global/aws-sso/terragrunt.hcl` | No changes needed (state migration handled by CLI) |
| `.tflint.hcl` | No changes needed |

---

## Logic

This section defines the exact sequence of operations to implement the VPC
infrastructure. Steps are ordered so each can be verified independently.

### Phase 1: Migrate provider source from opentofu/aws to hashicorp/aws

**What:** Switch all existing infrastructure from `opentofu/aws` to
`hashicorp/aws`, migrate state references, re-initialize, and verify zero
drift.

**Step 1.1:** Edit `infra/terraform/live/root.hcl`.

Replace the `generate "providers"` block. Change `opentofu/aws` to
`hashicorp/aws` and `6.21.0` to `6.33.0`. See Interfaces section for full
file content.

**Step 1.2:** Migrate provider references in existing state files.

The state files for `aws-organizations` and `aws-sso` currently track resources
under `registry.opentofu.org/opentofu/aws`. This must be updated to
`registry.opentofu.org/hashicorp/aws` before re-initializing, otherwise OpenTofu
will try to destroy and recreate all resources under the old provider.

```bash
aws sso login --profile tokenoverflow-root-admin

cd infra/terraform/live/global/aws-organizations
terragrunt state replace-provider \
  'registry.opentofu.org/opentofu/aws' \
  'registry.opentofu.org/hashicorp/aws'

cd ../aws-sso
terragrunt state replace-provider \
  'registry.opentofu.org/opentofu/aws' \
  'registry.opentofu.org/hashicorp/aws'
```

Each command will prompt for confirmation and automatically create a state
backup before making changes.

**Step 1.3:** Re-initialize existing units to download the new provider.

```bash
cd infra/terraform/live/global/aws-organizations
terragrunt init -upgrade

cd ../aws-sso
terragrunt init -upgrade
```

This downloads `hashicorp/aws` v6.33.0 and updates the `.terraform.lock.hcl`
files. The lock file entries will change from:

```
provider "registry.opentofu.org/opentofu/aws" {
  version = "6.21.0"
```

To:

```
provider "registry.opentofu.org/hashicorp/aws" {
  version = "6.33.0"
```

**Step 1.4:** Verify zero drift on existing infrastructure.

```bash
source scripts/src/includes.sh
tg plan global
```

The plan must show "No changes" for both `aws-organizations` and `aws-sso`.
If there is any drift, stop and investigate before proceeding. Do NOT continue
to Phase 2 until this step passes.

**Step 1.5:** Commit the provider migration.

```bash
git add infra/terraform/live/root.hcl
git add infra/terraform/live/global/aws-organizations/.terraform.lock.hcl
git add infra/terraform/live/global/aws-sso/.terraform.lock.hcl
git commit -m "infra: migrate provider from opentofu/aws to hashicorp/aws 6.33.0"
```

### Phase 2: Create the VPC wrapper module

**Step 2.1:** Create the module directory:

```bash
mkdir -p infra/terraform/modules/vpc
```

**Step 2.2:** Create `infra/terraform/modules/vpc/variables.tf` with the
content defined in the Interfaces section.

**Step 2.3:** Create `infra/terraform/modules/vpc/main.tf` with the content
defined in the Interfaces section.

**Step 2.4:** Create `infra/terraform/modules/vpc/outputs.tf` with the
content defined in the Interfaces section.

**Step 2.5:** Validate with TFLint:

```bash
cd infra/terraform/modules/vpc
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

### Phase 3: Create the prod Terragrunt unit

**Step 3.1:** Create the unit directory:

```bash
mkdir -p infra/terraform/live/prod/vpc
```

**Step 3.2:** Create `infra/terraform/live/prod/vpc/terragrunt.hcl` with the
content defined in the Interfaces section.

### Phase 4: Deploy the prod VPC

**Step 4.1:** Log in to the prod AWS account:

```bash
aws sso login --profile tokenoverflow-prod-admin
```

**Step 4.2:** Initialize the prod VPC unit:

```bash
cd infra/terraform/live/prod/vpc
terragrunt init --backend-bootstrap
```

This creates the S3 state backend bucket for prod
(`tokenoverflow-terraform-backend-prod`) if it does not exist, and downloads
the community VPC module.

**Step 4.3:** Review the plan:

```bash
terragrunt plan
```

Expected resources to be created (approximately 20 resources):

- 1 `aws_vpc`
- 6 `aws_subnet` (2 public, 2 private, 2 database)
- 3 `aws_route_table` (1 public, 1 private, 1 database)
- 6 `aws_route_table_association`
- 1 `aws_db_subnet_group`
- 1 `aws_internet_gateway` (created by the module, routes only in public RT)
- 1 `aws_route` (public RT -> IGW for `0.0.0.0/0`)
- Tags on all resources

**Step 4.4:** Apply:

```bash
terragrunt apply
```

**Step 4.5:** Verify outputs:

```bash
terragrunt output
```

Confirm that `vpc_id`, `public_subnet_ids`, `private_subnet_ids`,
`database_subnet_ids`, and `database_subnet_group_name` all return valid
values.

**Step 4.6:** Commit:

```bash
git add infra/terraform/modules/vpc/
git add infra/terraform/live/prod/vpc/
git commit -m "infra: add prod VPC with 3-tier subnet architecture"
```

---

## Edge Cases & Constraints

### 1. State migration for provider switch

**Risk:** The `state replace-provider` command modifies the state file. If done
incorrectly, OpenTofu could attempt to destroy and recreate all existing
resources.

**Mitigation:** The `state replace-provider` command automatically creates a
backup of the state before making changes. The command is interactive and
requires confirmation. After migration, `tg plan global` must show zero
changes before any further work proceeds.

### 2. Provider version bump from 6.21.0 to 6.33.0

**Risk:** Bumping the AWS provider across 12 minor versions could introduce
breaking changes in existing `aws-organizations` and `aws-sso` resources.

**Mitigation:** The AWS provider follows semantic versioning within the v6.x
line. Minor version bumps are backward compatible. The verification step
(`tg plan global` showing zero changes) catches any unexpected drift. If the
plan shows changes, the version can be bumped incrementally to isolate the
issue.

### 3. The community VPC module creates an Internet Gateway

**Risk:** Even with `enable_nat_gateway = false`, the
`terraform-aws-modules/vpc/aws` module creates an Internet Gateway when
`public_subnets` is non-empty. It also creates a route in the public subnet
route table pointing `0.0.0.0/0` to the IGW.

**Mitigation:** This is expected and acceptable. The IGW is a prerequisite for
future ALB deployment in public subnets. It has no cost. No inbound traffic can
reach any resource until security groups explicitly allow it.

### 4. Database subnets have no route to the internet

**Risk:** Services in isolated subnets cannot reach the internet (by design).
RDS instances in these subnets cannot reach external services.

**Mitigation:** This is intentional. RDS managed by AWS handles patching via
internal AWS infrastructure, not the public internet. If future services in
isolated subnets need to reach AWS APIs (e.g., S3, Secrets Manager), VPC
Endpoints should be added as a separate design.

### 5. S3 state backend bucket for prod

**Risk:** The prod state backend bucket
(`tokenoverflow-terraform-backend-prod`) may not exist yet since no prod
Terragrunt units have been deployed before.

**Mitigation:** Terragrunt auto-creates the S3 backend bucket when
`--backend-bootstrap` is passed to `terragrunt init`. This is the same
mechanism used for the `global` environment.

### 6. No NAT Gateway means private subnets have no outbound internet

**Risk:** Application servers in private subnets cannot reach the internet for
pulling container images, calling external APIs, etc.

**Mitigation:** This is known and accepted as a deferred concern. When NAT
Gateways are added later, only two lines change in `modules/vpc/main.tf`:

```hcl
enable_nat_gateway     = true
one_nat_gateway_per_az = true   # or single_nat_gateway = true for cost savings
```

No structural changes to Terragrunt or the directory layout are required.

### 7. AWS account profile must be configured

**Risk:** The `tokenoverflow-prod-admin` AWS SSO profile must be configured
locally before running Terragrunt against the prod stack.

**Mitigation:** Log in via AWS SSO before running any prod commands:

```bash
aws sso login --profile tokenoverflow-prod-admin
```

### 8. Lock files are gitignored

**Risk:** The `.terraform.lock.hcl` files are not explicitly gitignored, but
the `.terragrunt-cache/` directory is. The lock files at
`live/global/aws-organizations/.terraform.lock.hcl` and
`live/global/aws-sso/.terraform.lock.hcl` are committed and will be updated
by the provider migration.

**Mitigation:** These lock files should be committed. They ensure reproducible
provider versions across team members. The `git add` step in Phase 1 includes
them explicitly.

---

## Test Plan

### Verification Checklist

Infrastructure changes are verified through plan output inspection and
post-apply validation. There are no application-level tests since this is
purely a networking infrastructure change.

#### 1. Provider state migration succeeds

```bash
cd infra/terraform/live/global/aws-organizations
terragrunt state replace-provider \
  'registry.opentofu.org/opentofu/aws' \
  'registry.opentofu.org/hashicorp/aws'

cd ../aws-sso
terragrunt state replace-provider \
  'registry.opentofu.org/opentofu/aws' \
  'registry.opentofu.org/hashicorp/aws'
```

**Success:** Both commands complete without errors. State backups are created.

#### 2. Provider migration does not affect existing infrastructure

```bash
source scripts/src/includes.sh
tg plan global
```

**Success:** Plan shows "No changes. Your infrastructure matches the
configuration." for both `aws-organizations` and `aws-sso`.

#### 3. TFLint passes on the VPC module

```bash
cd infra/terraform/modules/vpc
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

**Success:** No errors or warnings.

#### 4. VPC plan creates expected resources

```bash
cd infra/terraform/live/prod/vpc
terragrunt plan
```

**Success:** Plan shows creation of:

- 1 `aws_vpc`
- 6 `aws_subnet` (2 per tier)
- 3 `aws_route_table`
- 6 `aws_route_table_association`
- 1 `aws_db_subnet_group`
- 1 `aws_internet_gateway`
- 1 `aws_route` (public `0.0.0.0/0` -> IGW)
- Resource names contain `prod`
- Tags include `Environment = prod`

#### 5. Post-apply: outputs return valid values

```bash
cd infra/terraform/live/prod/vpc
terragrunt output vpc_id
terragrunt output public_subnet_ids
terragrunt output private_subnet_ids
terragrunt output database_subnet_ids
terragrunt output database_subnet_group_name
```

**Success:** All outputs return non-empty values.

#### 6. Post-apply: route table isolation verification

Verify through the AWS CLI that database subnets have no internet route:

```bash
# Get the VPC ID
VPC_ID=$(cd infra/terraform/live/prod/vpc && terragrunt output -raw vpc_id)

# List all route tables for the VPC
aws ec2 describe-route-tables \
  --filters "Name=vpc-id,Values=$VPC_ID" \
  --query 'RouteTables[*].{ID:RouteTableId,Routes:Routes[*].{Dest:DestinationCidrBlock,Target:GatewayId},Tags:Tags[?Key==`Name`].Value|[0]}' \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --output table
```

**Success:**

- Public route table has `0.0.0.0/0 -> igw-xxx`
- Private route table has only `10.0.0.0/16 -> local`
- Database route table has only `10.0.0.0/16 -> local`

---

## Documentation Changes

### Files to Update

| File | Change |
|------|--------|
| `infra/terraform/README.md` | Add VPC section. Update provider source mention. |

### Content to Add to `infra/terraform/README.md`

Add the following after the existing "Run Stack" section:

```markdown
## Provider

All modules use `hashicorp/aws` as the provider source. This resolves to the
OpenTofu registry (`registry.opentofu.org/hashicorp/aws`) and is the
recommended approach for compatibility with community modules.

## VPC

The VPC module (`modules/vpc/`) provisions a 3-tier network architecture using
the `terraform-aws-modules/vpc/aws` community module:

| Tier     | Subnets         | Purpose                      |
|----------|-----------------|------------------------------|
| Public   | 2 (one per AZ)  | ALB, NAT GW (future)         |
| Private  | 2 (one per AZ)  | Application servers           |
| Isolated | 2 (one per AZ)  | Databases (no internet route) |

### CIDR Plan

| Environment | VPC CIDR      |
|-------------|---------------|
| prod        | 10.0.0.0/16   |
| dev         | 10.1.0.0/16   |

### Deploy

    ```shell
    $ source "${PROJECTS}/tokenoverflow/scripts/src/includes.sh"
    $ tg plan prod
    $ tg apply prod
    ```
```

### Files NOT Updated

Historical design documents are not updated. They are a snapshot of the
codebase at the time they were written.

---

## Development Environment Changes

### Brewfile

No changes needed. `tofuenv`, `terragrunt`, and `tflint` are already
installed.

### Environment Variables

No new environment variables are introduced.

### Setup Flow

No changes. The `source scripts/src/includes.sh && setup` command continues
to work. The `tg` helper function already supports the `prod` environment.

---

## Tasks

### Task 1: Migrate provider from opentofu/aws to hashicorp/aws

**What:** Update `root.hcl`, run `state replace-provider` on existing units,
re-initialize, and verify zero drift.

**Steps:**

1. Edit `infra/terraform/live/root.hcl`: change `source = "opentofu/aws"` to
   `source = "hashicorp/aws"` and `version = "6.21.0"` to
   `version = "6.33.0"` in the `generate "providers"` block. See Interfaces
   section for the full file content.
2. Log in: `aws sso login --profile tokenoverflow-root-admin`
3. Migrate state for aws-organizations:

   ```bash
   cd infra/terraform/live/global/aws-organizations
   terragrunt state replace-provider \
     'registry.opentofu.org/opentofu/aws' \
     'registry.opentofu.org/hashicorp/aws'
   ```

4. Migrate state for aws-sso:

   ```bash
   cd infra/terraform/live/global/aws-sso
   terragrunt state replace-provider \
     'registry.opentofu.org/opentofu/aws' \
     'registry.opentofu.org/hashicorp/aws'
   ```

5. Re-initialize both units:

   ```bash
   cd infra/terraform/live/global/aws-organizations && terragrunt init -upgrade
   cd ../aws-sso && terragrunt init -upgrade
   ```

6. Verify zero-diff plan: `tg plan global`
7. Commit:

   ```bash
   git add infra/terraform/live/root.hcl
   git add infra/terraform/live/global/aws-organizations/.terraform.lock.hcl
   git add infra/terraform/live/global/aws-sso/.terraform.lock.hcl
   git commit -m "infra: migrate provider from opentofu/aws to hashicorp/aws 6.33.0"
   ```

**Success:** `tg plan global` shows no changes for both `aws-organizations`
and `aws-sso` units. Lock files show `registry.opentofu.org/hashicorp/aws`
version `6.33.0`.

### Task 2: Create the VPC wrapper module

**What:** Create `infra/terraform/modules/vpc/` with `main.tf`,
`variables.tf`, and `outputs.tf`.

**Steps:**

1. `mkdir -p infra/terraform/modules/vpc`
2. Create `variables.tf` with the content from the Interfaces section
3. Create `main.tf` with the content from the Interfaces section
4. Create `outputs.tf` with the content from the Interfaces section
5. Run TFLint:

   ```bash
   cd infra/terraform/modules/vpc
   tflint --config="$(pwd)/../../.tflint.hcl" --init
   tflint --config="$(pwd)/../../.tflint.hcl"
   ```

**Success:** TFLint passes with no errors. The three files follow the same
patterns as existing modules (`aws-organizations`, `aws-sso`).

### Task 3: Create the prod Terragrunt unit

**What:** Create `infra/terraform/live/prod/vpc/terragrunt.hcl` with
prod-specific inputs.

**Steps:**

1. `mkdir -p infra/terraform/live/prod/vpc`
2. Create `terragrunt.hcl` with the content from the Interfaces section

**Success:** File exists and follows the same pattern as existing units
(`aws-organizations/terragrunt.hcl`, `aws-sso/terragrunt.hcl`).

### Task 4: Deploy the prod VPC

**What:** Initialize, plan, and apply the VPC infrastructure.

**Steps:**

1. Log in: `aws sso login --profile tokenoverflow-prod-admin`
2. Initialize:

   ```bash
   cd infra/terraform/live/prod/vpc
   terragrunt init --backend-bootstrap
   ```

3. Plan: `terragrunt plan` -- review output against the expected resource list
   in Logic Phase 4
4. Apply: `terragrunt apply`
5. Verify outputs: `terragrunt output`
6. Verify route table isolation via AWS CLI (see Test Plan section 6)
7. Commit:

   ```bash
   git add infra/terraform/modules/vpc/
   git add infra/terraform/live/prod/vpc/
   git commit -m "infra: add prod VPC with 3-tier subnet architecture"
   ```

**Success:** All outputs return valid values. Public route table has IGW
route. Database route table has only the local VPC route.

### Task 5: Update documentation

**What:** Update `infra/terraform/README.md` with VPC and provider
information.

**Steps:**

1. Add Provider section to `infra/terraform/README.md` explaining the
   `hashicorp/aws` source on OpenTofu
2. Add VPC section with tier table, CIDR plan, and deploy commands (see
   Documentation Changes section)
3. Commit:

   ```bash
   git add infra/terraform/README.md
   git commit -m "docs: add VPC networking documentation to terraform README"
   ```

**Success:** README accurately describes the provider source, VPC layout, CIDR
plan, and deploy workflow.
