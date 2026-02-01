# Design: rds-postgresql

## Architecture Overview

### Goal

Provision an AWS RDS PostgreSQL instance in the production VPC's tier 3 isolated
subnets using the `terraform-aws-modules/rds/aws` community module (v7.0.0),
orchestrated by Terragrunt. The configuration must follow the same DRY wrapper
module pattern established by the VPC module, use the `password_wo` write-only
attribute so credentials never land in Terraform state, and keep costs minimal
for an early-stage product.

### Scope

This design covers:

- OpenTofu upgrade from 1.10.7 to 1.11.5 (required for write-only attributes)
- RDS PostgreSQL instance (`db.t4g.micro`, single-AZ) in the prod VPC
- Security group restricting access to specific private subnet CIDRs (tier 2)
- Write-only password via `password_wo` (never stored in state)
- gp3 storage (20GB allocated, 100GB max autoscaling)
- Encryption at rest using AWS managed KMS key
- Terragrunt module and unit configuration
- Database and identifier naming decisions

This design does NOT cover:

- Application-level database users (handled by migrations/application)
- PgBouncer / connection pooling in production (separate design)
- Read replicas (not needed at MVP scale)
- Custom parameter groups (use defaults, tune later if needed)
- Dev environment RDS (deferred, but directory structure supports it)
- pgvector extension setup (handled by application migrations)
- Monitoring and alarms (deferred)
- Automatic password rotation (explicitly not wanted)

### OpenTofu Upgrade

The `password_wo` write-only attribute and the community RDS module v7.0.0 both
require OpenTofu >= 1.11.0. The project currently pins OpenTofu 1.10.7 in
`.opentofu-version`.

| Current | Target | Why |
|---------|--------|-----|
| 1.10.7 | 1.11.5 | Latest stable release (2026-02-12). Required for write-only attributes (`password_wo`). Required by `terraform-aws-modules/rds/aws` v7.0.0 (`required_version >= 1.11`). |

**Impact on existing units:** OpenTofu 1.11.x is backward compatible with
1.10.x configurations. The existing units (`aws-organizations`, `aws-sso`,
`vpc`) do not use any features removed in 1.11. The upgrade requires:

1. Update `.opentofu-version` from `1.10.7` to `1.11.5`
2. Run `tofuenv install` to install the new version
3. Re-initialize existing units (`terragrunt init -upgrade`) to update lock
   files
4. Verify zero drift on existing infrastructure (`tg plan global` and
   `tg plan prod`)

The generated `provider.tf` in `root.hcl` does not have a
`required_version` constraint, so no changes to `root.hcl` are needed.

### Database Name Decision

The user wants a generic database name with hyphens (`main-prod`) that does
not repeat the project name. However, hyphens in PostgreSQL database names
have a specific technical implication that must be understood.

#### Hyphens in PostgreSQL Database Names: Technical Analysis

PostgreSQL identifiers (database names, table names, column names) follow
these rules:

- **Unquoted identifiers** can only contain letters, digits, and underscores.
  They are case-folded to lowercase.
- **Quoted identifiers** (wrapped in double quotes) can contain any character
  except the null byte. They are case-sensitive.

A database named `main-prod` (with a hyphen) is a **quoted identifier**. This
means:

| Context | `main_prod` (underscore) | `main-prod` (hyphen) |
|---------|-------------------------|---------------------|
| `CREATE DATABASE` | `CREATE DATABASE main_prod;` | `CREATE DATABASE "main-prod";` |
| `psql -d` | `psql -d main_prod` | `psql -d main-prod` (works -- psql passes it as a connection parameter, not SQL) |
| Connection URI | `postgres://user:pass@host/main_prod` | `postgres://user:pass@host/main-prod` (works -- hyphens are valid URI path characters per RFC 3986) |
| `\c` in psql | `\c main_prod` | `\c "main-prod"` (must quote) |
| Diesel `database_url` | Works | Works -- Diesel uses the URI for connections, not raw SQL. `diesel setup` uses `push_identifier()` which properly quotes. |
| Raw SQL `CREATE DATABASE` | No quoting needed | Must double-quote: `"main-prod"` |
| Application code | No quoting needed in connection strings | No quoting needed in connection strings |

**Key finding:** Hyphens work fine in connection URIs and with Diesel ORM.
The application code at `apps/api/src/config.rs` constructs the database URL
as `postgres://user:pass@host:port/dbname` -- the database name goes in the
URI path where hyphens are valid characters. Diesel's
`AsyncDieselConnectionManager::new(database_url)` parses this URI directly.

The only place hyphens cause friction is in raw SQL (like `psql` interactive
sessions or hand-written `CREATE DATABASE` statements), where the name must
be double-quoted. However, the database is created by RDS (via `db_name`
parameter), not by manual SQL, and application queries never reference the
database name in SQL statements -- they connect to it via the URI.

The project's integration test code at `apps/api/tests/integration/test_db.rs`
uses raw `CREATE DATABASE` without quoting, but those test databases are named
`test_0`, `test_1`, etc. -- they never use the production database name.

**Decision: `main-prod` (with hyphens).** Hyphens work without issues in
connection URIs, Diesel, and any PostgreSQL client library that connects via
URI. The RDS `db_name` parameter handles creation. The only trade-off is needing
double quotes in interactive `psql` commands (`\c "main-prod"`), which is minor.
PostgreSQL identifiers with hyphens must be double-quoted in SQL, but since the
database name is never used inside application SQL statements (only in
connection URIs), this has no practical impact on the codebase.

**Config file impact:** The following files will need to be updated (as a
separate application config change, not part of the infrastructure tasks):

| File | Current `database.name` | New `database.name` |
|------|------------------------|---------------------|
| `apps/api/config/production.toml` | `tokenoverflow` | `main-prod` |
| `apps/api/config/development.toml` | `tokenoverflow` | `main-dev` |
| `apps/api/config/local.toml` | `tokenoverflow` | No change (local dev is not RDS) |
| `apps/api/config/unit_test.toml` | N/A | No change |
| `docker-compose.yml` | `tokenoverflow` | No change (local dev is not RDS) |

Local development and unit tests keep `tokenoverflow` as the database name
because changing them would also require updating the Docker Compose
`POSTGRES_DB` and all local connection strings for no benefit.

### RDS Instance Identifier Decision

The RDS instance identifier is the AWS resource name visible in the console and
used in endpoints.

| Option | Identifier | Resulting Endpoint |
|--------|-----------|-------------------|
| A. `tokenoverflow` | No environment suffix | `tokenoverflow.xxxx.us-east-1.rds.amazonaws.com` |
| B. `tokenoverflow-prod` | Includes environment | `tokenoverflow-prod.xxxx.us-east-1.rds.amazonaws.com` |
| C. `tokenoverflow-db-prod` | Includes resource type and environment | `tokenoverflow-db-prod.xxxx.us-east-1.rds.amazonaws.com` |

**Decision: Option B (`tokenoverflow-prod`).** The environment suffix prevents
mistakes when switching between AWS accounts. Even though environments are in
separate AWS accounts, it is easy to have the wrong AWS profile active. Seeing
`-prod` in the endpoint string provides an immediate visual confirmation.

The identifier is constructed as `"tokenoverflow-${var.env_name}"` in the
module, so for dev it would become `tokenoverflow-dev`.

### Master Username Decision

| Option | Username | Notes |
|--------|----------|-------|
| A. `tokenoverflow` | Matches local dev | Consistent. Used as the "admin" account. |
| B. `postgres` | PostgreSQL default | Common, but generic and a known target for brute force. |
| C. `admin` | AWS default suggestion | Generic. Known target. |

**Decision: Option A (`tokenoverflow`).** Matches the local development
configuration (`POSTGRES_USER: tokenoverflow` in `docker-compose.yml` and
`database.user = "tokenoverflow"` in config files). Using a non-default
username adds a trivial layer of security-through-obscurity on top of the
real security controls (isolated subnet, security group).

### Credential Management Decision

The user does not want Secrets Manager (cost constraint), does not need
automatic password rotation, and does not want any sensitive data in Terraform
state.

Three approaches were evaluated:

| Approach | How It Works | Password in State? | Monthly Cost | Rotation |
|----------|-------------|-------------------|-------------|----------|
| A. `password_wo` (write-only) | User provides password as a variable at apply time. OpenTofu sends it to AWS but never writes it to state or plan files. | No | $0 | Manual: update password, increment `password_wo_version`, apply |
| B. `random_password` + SSM Parameter Store | Terraform generates password, stores in SSM SecureString, passes to RDS | Yes (`random_password` result in state) | $0 | Manual: taint + apply |
| C. RDS-managed Secrets Manager | RDS creates and auto-rotates via `manage_master_user_password = true` | No | ~$0.40/month | Automatic |

**Option B is eliminated** because the user does not want sensitive data in
Terraform state.

**Option C is eliminated** because the user does not want Secrets Manager
due to cost.

**Decision: Option A (`password_wo` write-only attribute).**

How it works:

1. The module declares a `password_wo` variable (marked `ephemeral` and
   `sensitive`).
2. The user provides the password at apply time via a Terragrunt variable
   (environment variable, `-var` flag, or `*.tfvars` file).
3. OpenTofu sends the password to the AWS API to configure the RDS instance.
4. OpenTofu then discards the value -- it is never written to the state file
   or plan file.
5. To verify the password works, the user connects to the database using the
   password they provided.

**How the user provides the password:**

The recommended approach is via the `TF_VAR_password_wo` environment variable:

```bash
export TF_VAR_password_wo='<the-password>'
terragrunt apply
```

This keeps the password out of command history (no `-var` on the command line)
and out of committed files (no `.tfvars` file). The user is responsible for
storing the password securely on their end (e.g., in a password manager).

**How to rotate the password:**

```bash
export TF_VAR_password_wo='<new-password>'
export TF_VAR_password_wo_version=2    # increment from previous value
terragrunt apply
```

The `password_wo_version` is a numeric trigger. OpenTofu does not store the
password value, so it cannot detect when the password changes. Incrementing
the version tells OpenTofu to re-apply the password.

### Encryption Decision

| Option | Key Type | Monthly Cost | Notes |
|--------|----------|-------------|-------|
| A. AWS managed key (`aws/rds`) | AWS-managed | $0 | Default. AWS manages lifecycle. Cannot be shared cross-account. |
| B. Customer managed KMS key (CMK) | Customer-managed | ~$1/month + API calls | Full control. Can share cross-account. Required for some compliance. |

**Decision: Option A (AWS managed key, `aws/rds`).** The task specifies "AWS
managed keys (KMS)" which is the default `aws/rds` key. This key is free,
requires no management, and provides AES-256 encryption at rest. A customer
managed key (CMK) is only needed if cross-account key sharing or custom key
policies are required, neither of which applies here.

### Network Architecture

```text
              VPC 10.0.0.0/16
              +------------------------------------------+
              |                                          |
              |  Tier 2: Private Subnets                 |
              |  +----------------+  +----------------+  |
              |  | 10.0.10.0/24   |  | 10.0.11.0/24   |  |
              |  | us-east-1a     |  | us-east-1b     |  |
              |  | (App servers)  |  | (App servers)  |  |
              |  +-------+--------+  +-------+--------+  |
              |          |                   |            |
              |          | SG: port 5432     |            |
              |          | (per-subnet CIDR) |            |
              |          |                   |            |
              |  Tier 3: Isolated Subnets (DB)           |
              |  +----------------+  +----------------+  |
              |  | 10.0.20.0/24   |  | 10.0.21.0/24   |  |
              |  | us-east-1a     |  | us-east-1b     |  |
              |  |  [RDS Primary] |  | (standby slot) |  |
              |  +----------------+  +----------------+  |
              |                                          |
              +------------------------------------------+
```

**Connectivity rules:**

- The RDS security group allows inbound TCP on port 5432 only from the
  specific private subnet CIDRs (`10.0.10.0/24` and `10.0.11.0/24`). One
  ingress rule is created per CIDR using `for_each`.
- No inbound from public subnets, isolated subnets, or the internet.
- The database subnet group (`prod`) was already created by the VPC module and
  spans both isolated subnets across two AZs.
- Even though multi-AZ is disabled (single-AZ deployment), the subnet group
  must still contain subnets in at least two AZs (AWS requirement). RDS will
  place the instance in one of them.

### PostgreSQL Engine Version

RDS PostgreSQL 17 is the latest major version available on AWS RDS and is
supported on `db.t4g.micro`. The community RDS module example also uses
PostgreSQL 17. The local development environment uses `pgvector/pgvector:pg17`,
so PostgreSQL 17 maintains parity between local and production.

The specific minor version will be set to the latest available at deploy time
using `engine_version = "17"` (RDS resolves to the latest 17.x automatically).

### Module Strategy

Consistent with the VPC design, a wrapper module pattern will be used:

```text
infra/terraform/
  modules/
    rds/                          # new wrapper module
      main.tf                    # SG + community RDS module
      variables.tf
      outputs.tf
  live/
    prod/
      rds/                       # new Terragrunt unit
        terragrunt.hcl
```

The wrapper module encapsulates:
1. The security group for RDS (ingress from private subnets on port 5432)
2. The community RDS module call with all required parameters
3. Outputs needed by downstream consumers (endpoint, port, identifier)

This keeps the Terragrunt unit simple (just inputs) and provides a natural
place to add future resources (parameter groups, option groups, CloudWatch
alarms) without restructuring.

### Terragrunt Dependency

The RDS unit depends on the VPC unit to obtain:
- `vpc_id` (for the security group)
- `database_subnet_group_name` (for RDS placement)

This is expressed as a Terragrunt `dependency` block.

### Directory Structure (After)

```text
infra/terraform/
  modules/
    aws-organizations/       # existing
    aws-sso/                 # existing
    vpc/                     # existing
    rds/                     # new
      main.tf
      variables.tf
      outputs.tf
  live/
    root.hcl                 # existing (unchanged)
    global/
      env.hcl                # existing (unchanged)
      aws-organizations/     # existing (unchanged)
      aws-sso/               # existing (unchanged)
    prod/
      env.hcl                # existing (unchanged)
      vpc/
        terragrunt.hcl       # existing (unchanged)
      rds/
        terragrunt.hcl       # new
    dev/
      env.hcl                # existing (unchanged)
```

---

## Interfaces

This section documents every file that will be created or modified, and the
contracts between them.

### New Files

#### `infra/terraform/modules/rds/variables.tf`

```hcl
variable "env_name" {
  description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
  type        = string
}

variable "project_name" {
  description = "Project name. Used as prefix for the RDS instance identifier."
  type        = string
  default     = "tokenoverflow"
}

variable "vpc_id" {
  description = "ID of the VPC where the RDS security group will be created."
  type        = string
}

variable "database_subnet_group_name" {
  description = "Name of the database subnet group for RDS placement."
  type        = string
}

variable "private_subnet_cidrs" {
  description = "CIDR blocks of private subnets allowed to connect to RDS."
  type        = list(string)
}

variable "engine_version" {
  description = "PostgreSQL engine version (e.g., '17'). RDS resolves to the latest minor version."
  type        = string
  default     = "17"
}

variable "instance_class" {
  description = "RDS instance class (e.g., 'db.t4g.micro')."
  type        = string
  default     = "db.t4g.micro"
}

variable "allocated_storage" {
  description = "Initial allocated storage in GB."
  type        = number
  default     = 20
}

variable "max_allocated_storage" {
  description = "Maximum storage in GB for autoscaling. Set to 0 to disable."
  type        = number
  default     = 100
}

variable "db_name" {
  description = "Name of the initial database to create (e.g., 'main-prod')."
  type        = string
}

variable "username" {
  description = "Master username for the database."
  type        = string
  default     = "tokenoverflow"
}

variable "multi_az" {
  description = "Enable Multi-AZ deployment."
  type        = bool
  default     = false
}

variable "password_wo" {
  description = "Master password for the database. Write-only: never stored in state."
  type        = string
  sensitive   = true
  ephemeral   = true
}

variable "password_wo_version" {
  description = "Increment to trigger a password update. OpenTofu cannot detect password changes since the value is not stored."
  type        = number
  default     = 1
}
```

#### `infra/terraform/modules/rds/main.tf`

```hcl
locals {
  identifier = "${var.project_name}-${var.env_name}"
}

# ---------- Security Group ----------

resource "aws_security_group" "rds" {
  name        = "${local.identifier}-rds"
  description = "Allow PostgreSQL access from private subnets only"
  vpc_id      = var.vpc_id

  tags = {
    Name        = "${local.identifier}-rds"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_vpc_security_group_ingress_rule" "postgresql" {
  for_each = toset(var.private_subnet_cidrs)

  security_group_id = aws_security_group.rds.id
  description       = "PostgreSQL from ${each.value}"
  from_port         = 5432
  to_port           = 5432
  ip_protocol       = "tcp"
  cidr_ipv4         = each.value
}

# ---------- RDS ----------

module "rds" {
  source  = "terraform-aws-modules/rds/aws"
  version = "7.0.0"

  identifier = local.identifier

  # Engine
  engine               = "postgres"
  engine_version       = var.engine_version
  family               = "postgres17"
  major_engine_version = "17"

  # Instance
  instance_class = var.instance_class
  multi_az       = var.multi_az

  # Storage
  allocated_storage     = var.allocated_storage
  max_allocated_storage = var.max_allocated_storage
  storage_type          = "gp3"
  storage_encrypted     = true
  # Uses default AWS managed key (aws/rds) - no kms_key_id needed

  # Database
  db_name  = var.db_name
  username = var.username
  port     = 5432

  # Credentials: write-only password (never stored in state)
  manage_master_user_password = false
  password_wo                 = var.password_wo
  password_wo_version         = var.password_wo_version

  # Network
  db_subnet_group_name   = var.database_subnet_group_name
  vpc_security_group_ids = [aws_security_group.rds.id]
  publicly_accessible    = false

  # Backups
  backup_retention_period          = 7
  backup_window                    = "03:00-04:00"
  maintenance_window               = "mon:04:00-mon:05:00"
  skip_final_snapshot              = false
  final_snapshot_identifier_prefix = "${local.identifier}-final"
  copy_tags_to_snapshot            = true
  deletion_protection              = true

  # Monitoring (basic, no enhanced monitoring to avoid cost)
  enabled_cloudwatch_logs_exports = ["postgresql"]

  # Parameter group: use module-created default
  create_db_parameter_group = true

  # Tags
  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
```

#### `infra/terraform/modules/rds/outputs.tf`

```hcl
output "db_instance_endpoint" {
  description = "The connection endpoint of the RDS instance (host:port)."
  value       = module.rds.db_instance_endpoint
}

output "db_instance_address" {
  description = "The hostname of the RDS instance."
  value       = module.rds.db_instance_address
}

output "db_instance_port" {
  description = "The port of the RDS instance."
  value       = module.rds.db_instance_port
}

output "db_instance_name" {
  description = "The name of the database."
  value       = module.rds.db_instance_name
}

output "db_instance_username" {
  description = "The master username."
  value       = module.rds.db_instance_username
  sensitive   = true
}

output "db_instance_identifier" {
  description = "The RDS instance identifier."
  value       = module.rds.db_instance_identifier
}

output "db_instance_arn" {
  description = "The ARN of the RDS instance."
  value       = module.rds.db_instance_arn
}

output "security_group_id" {
  description = "The ID of the RDS security group."
  value       = aws_security_group.rds.id
}
```

#### `infra/terraform/live/prod/rds/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/rds"
}

dependency "vpc" {
  config_path = "../vpc"
}

inputs = {
  env_name                   = "prod"
  vpc_id                     = dependency.vpc.outputs.vpc_id
  database_subnet_group_name = dependency.vpc.outputs.database_subnet_group_name
  private_subnet_cidrs       = ["10.0.10.0/24", "10.0.11.0/24"]
  db_name                    = "main-prod"

  # password_wo and password_wo_version are provided via environment variables:
  #   TF_VAR_password_wo=<password>
  #   TF_VAR_password_wo_version=<version>

  # All other inputs use module defaults:
  # project_name          = "tokenoverflow"
  # engine_version        = "17"
  # instance_class        = "db.t4g.micro"
  # allocated_storage     = 20
  # max_allocated_storage = 100
  # username              = "tokenoverflow"
  # multi_az              = false
}
```

### Modified Files

#### `.opentofu-version`

**Change:** Update from `1.10.7` to `1.11.5`.

```
1.11.5
```

#### `apps/api/config/production.toml`

**Change:** Update `database.name` from `tokenoverflow` to `main-prod` and
`database.host` placeholder to match the new instance identifier pattern.

```toml
[database]
host = "tokenoverflow-prod.xxxx.us-east-1.rds.amazonaws.com"
port = 5432
user = "tokenoverflow"
name = "main-prod"
```

The actual `host` value will be filled in after `terragrunt apply` using the
`db_instance_address` output. This config change is an application concern and
is listed here for completeness but is NOT part of the infrastructure tasks.

#### `apps/api/config/development.toml`

**Change:** Update `database.name` from `tokenoverflow` to `main-dev`.

```toml
[database]
host = "tokenoverflow-dev.xxxx.us-east-1.rds.amazonaws.com"
port = 5432
user = "tokenoverflow"
name = "main-dev"
```

Same note as above -- this is an application config change, not an
infrastructure task.

### Files NOT Modified

| File | Reason |
|------|--------|
| `live/root.hcl` | No changes needed. The generated `provider.tf` has no `required_version`, so OpenTofu 1.11.5 works without modification. Provider version `hashicorp/aws` 6.33.0 satisfies the RDS module v7.0.0 requirement of `>= 6.27`. |
| `live/prod/env.hcl` | No changes needed |
| `live/prod/vpc/terragrunt.hcl` | No changes needed |
| `modules/vpc/outputs.tf` | Already exports `vpc_id`, `database_subnet_group_name` |
| `modules/vpc/main.tf` | Already creates database subnet group |
| `apps/api/config/local.toml` | Local dev keeps `database.name = "tokenoverflow"` (not RDS) |
| `apps/api/config/unit_test.toml` | Not affected |
| `docker-compose.yml` | Local dev keeps `POSTGRES_DB: tokenoverflow` (not RDS) |

---

## Logic

This section defines the exact sequence of operations to implement the RDS
infrastructure.

### Phase 1: Upgrade OpenTofu to 1.11.5

**Step 1.1:** Update `.opentofu-version`:

```bash
echo "1.11.5" > .opentofu-version
```

**Step 1.2:** Install and activate the new version:

```bash
tofuenv install
tofuenv use 1.11.5
tofu version  # confirm: OpenTofu v1.11.5
```

**Step 1.3:** Re-initialize existing global units:

```bash
aws sso login --profile tokenoverflow-root-admin
cd infra/terraform/live/global/aws-organizations
terragrunt init -upgrade
cd ../aws-sso
terragrunt init -upgrade
```

**Step 1.4:** Verify zero drift on existing global infrastructure:

```bash
source scripts/src/includes.sh
tg plan global
```

Must show "No changes" for both units.

**Step 1.5:** Re-initialize existing prod units:

```bash
aws sso login --profile tokenoverflow-prod-admin
cd infra/terraform/live/prod/vpc
terragrunt init -upgrade
```

**Step 1.6:** Verify zero drift on existing prod infrastructure:

```bash
tg plan prod
```

Must show "No changes" for the VPC unit.

**Step 1.7:** Commit:

```bash
git add .opentofu-version
git add infra/terraform/live/global/aws-organizations/.terraform.lock.hcl
git add infra/terraform/live/global/aws-sso/.terraform.lock.hcl
git add infra/terraform/live/prod/vpc/.terraform.lock.hcl
git commit -m "infra: upgrade OpenTofu from 1.10.7 to 1.11.5"
```

### Phase 2: Create the RDS wrapper module

**Step 2.1:** Create the module directory:

```bash
mkdir -p infra/terraform/modules/rds
```

**Step 2.2:** Create `infra/terraform/modules/rds/variables.tf` with the
content defined in the Interfaces section.

**Step 2.3:** Create `infra/terraform/modules/rds/main.tf` with the content
defined in the Interfaces section.

**Step 2.4:** Create `infra/terraform/modules/rds/outputs.tf` with the content
defined in the Interfaces section.

**Step 2.5:** Validate with TFLint:

```bash
cd infra/terraform/modules/rds
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

### Phase 3: Create the prod Terragrunt unit

**Step 3.1:** Create the unit directory:

```bash
mkdir -p infra/terraform/live/prod/rds
```

**Step 3.2:** Create `infra/terraform/live/prod/rds/terragrunt.hcl` with the
content defined in the Interfaces section.

### Phase 4: Deploy the prod RDS instance

**Step 4.1:** Log in to the prod AWS account:

```bash
aws sso login --profile tokenoverflow-prod-admin
```

**Step 4.2:** Initialize the prod RDS unit:

```bash
cd infra/terraform/live/prod/rds
terragrunt init
```

**Step 4.3:** Review the plan (password provided via environment variable):

```bash
export TF_VAR_password_wo='<chosen-password>'
terragrunt plan
```

Expected resources to be created:

- 1 `aws_security_group` (RDS security group)
- 2 `aws_vpc_security_group_ingress_rule` (one per private subnet CIDR)
- 1 `aws_db_instance` (PostgreSQL 17, db.t4g.micro)
- 1 `aws_db_parameter_group` (postgres17 family)

The plan should NOT show any changes to VPC resources (they are a dependency
only, read via Terragrunt `dependency`). The password value should NOT appear
anywhere in the plan output.

**Step 4.4:** Apply:

```bash
terragrunt apply
```

Note: RDS instance creation typically takes 5-15 minutes.

**Step 4.5:** Verify outputs:

```bash
terragrunt output db_instance_endpoint
terragrunt output db_instance_address
terragrunt output db_instance_identifier
```

**Step 4.6:** Verify password is NOT in state:

```bash
terragrunt state show 'module.rds.aws_db_instance.this[0]' | grep -i password
```

Should return no matches or show `password_wo` as `(write-only attribute)`.

**Step 4.7:** Commit:

```bash
git add infra/terraform/modules/rds/
git add infra/terraform/live/prod/rds/
git commit -m "infra: add prod RDS PostgreSQL 17 with write-only credentials"
```

### Phase 5: Update application configuration

**Step 5.1:** After deployment, update `apps/api/config/production.toml` with
the actual RDS endpoint and new database name:

```toml
[database]
host = "<actual-rds-endpoint-from-output>"
port = 5432
user = "tokenoverflow"
name = "main-prod"
```

The password is provided to the application via the
`TOKENOVERFLOW_DATABASE_PASSWORD` environment variable at deploy time. The user
manages the password on their end. This is an application-level concern.

---

## Edge Cases & Constraints

### 1. Password is never stored in Terraform state

**Benefit:** The `password_wo` attribute is a write-only field. OpenTofu sends
the value to the AWS API but never writes it to the state file or plan file.
This means that even if the state bucket is compromised, the database password
is not exposed.

**Trade-off:** Because OpenTofu does not store the password, it cannot detect
when the password changes. To update the password, you must increment
`password_wo_version` to signal that a new value should be applied.

**Trade-off:** The user is responsible for storing and managing the password
outside of Terraform. If the password is lost, the only recovery option is to
reset it via the AWS console or CLI:

```bash
aws rds modify-db-instance \
  --db-instance-identifier tokenoverflow-prod \
  --master-user-password '<new-password>' \
  --apply-immediately \
  --profile tokenoverflow-prod-admin \
  --region us-east-1
```

Then update the Terraform variable to match and increment `password_wo_version`.

### 2. OpenTofu upgrade may update lock files

**Risk:** Upgrading from 1.10.7 to 1.11.5 and running `terragrunt init
-upgrade` will update `.terraform.lock.hcl` files for existing units, even
though no functional changes are being made.

**Mitigation:** The lock file updates are expected and safe. They reflect the
new provider hashes for the upgraded OpenTofu version. The verification step
(`tg plan global` and `tg plan prod` showing zero changes) confirms that the
upgrade does not affect existing infrastructure.

### 3. Storage autoscaling can only increase, never decrease

**Risk:** Once RDS autoscales storage from 20GB to, say, 50GB, it cannot be
reduced back to 20GB. This is an AWS limitation.

**Mitigation:** The 100GB max cap prevents runaway growth. For an MVP, 20GB
initial with 100GB max is generous. If storage usage approaches the max,
investigate data retention policies or consider data archival.

### 4. Single-AZ means downtime during maintenance

**Risk:** With `multi_az = false`, database maintenance (patching, minor version
upgrades) requires a brief outage. AWS typically completes this in under 10
minutes.

**Mitigation:** The maintenance window is set to `mon:04:00-mon:05:00` UTC
(Sunday night US Eastern), which is low-traffic for a developer-focused product.
This is an accepted trade-off for cost savings. Multi-AZ can be enabled later
by changing one variable.

### 5. `deletion_protection = true` prevents accidental destruction

**Risk:** Running `terragrunt destroy` or modifying the instance identifier
could accidentally delete the database.

**Mitigation:** Deletion protection is enabled. To intentionally delete the
instance, you must first set `deletion_protection = false`, apply, then
destroy. A final snapshot is always created.

### 6. The `db.t4g.micro` instance has limited resources

**Risk:** `db.t4g.micro` provides 2 vCPUs (burstable) and 1 GB RAM. Under
sustained load, CPU credits can be exhausted, causing performance degradation.

**Mitigation:** This is an MVP. The instance class is a variable and can be
changed to `db.t4g.small` (2 GB RAM) or `db.t4g.medium` (4 GB RAM) with a
single variable change and a brief outage. Storage IOPS baseline is 3000 on
gp3, which is more than sufficient for early traffic.

### 7. Security group uses per-subnet CIDRs

**Risk:** If new private subnets are added to the VPC in the future, the
security group will not automatically allow them. The
`private_subnet_cidrs` input must be updated.

**Mitigation:** This is intentional and preferred for security. Every new
subnet that needs database access must be explicitly added to the
`private_subnet_cidrs` list in the Terragrunt unit, providing an audit trail
of which subnets can reach the database. This is a feature, not a bug.

### 8. Lock file must be committed

**Risk:** The `.terraform.lock.hcl` generated during `terragrunt init` pins
provider and module versions. If not committed, different team members may
get different versions.

**Mitigation:** The lock file at `infra/terraform/live/prod/rds/.terraform.lock.hcl`
must be committed to git, consistent with how the VPC and other units handle
lock files.

### 9. Database name change requires application config update

**Risk:** The database name changes from `tokenoverflow` to `main-prod` for
production. If the application config is not updated, the API will fail to
connect.

**Mitigation:** The application config update (`apps/api/config/production.toml`
and `apps/api/config/development.toml`) is documented in the Modified Files
section. The local development environment is unaffected (keeps `tokenoverflow`).
The config update should be made as part of the production deployment, before
the API attempts to connect to the new RDS instance.

### 10. Hyphenated database name requires double-quoting in raw SQL

**Risk:** The database name `main-prod` contains a hyphen, which means it must
be double-quoted in any raw SQL context (e.g., `\c "main-prod"` in psql, or
`CREATE DATABASE "main-prod"` in manual SQL).

**Mitigation:** The database is created automatically by RDS via the `db_name`
parameter -- no manual `CREATE DATABASE` is needed. Application code connects
via URI (`postgres://...host.../main-prod`), where hyphens are valid characters.
Diesel ORM and all standard PostgreSQL client libraries parse the database name
from the URI without issues. The only impact is interactive psql sessions,
where the user must type `\c "main-prod"` instead of `\c main-prod`. This is
a minor inconvenience.

---

## Test Plan

### Verification Checklist

Infrastructure changes are verified through plan output inspection and
post-apply validation. There are no application-level tests since this is
purely an infrastructure change.

#### 1. OpenTofu upgrade: zero drift on existing infrastructure

```bash
source scripts/src/includes.sh
tg plan global
tg plan prod
```

**Success:** All existing units show "No changes."

#### 2. TFLint passes on the RDS module

```bash
cd infra/terraform/modules/rds
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

**Success:** No errors or warnings.

#### 3. RDS plan creates expected resources

```bash
cd infra/terraform/live/prod/rds
export TF_VAR_password_wo='test-plan-password'
terragrunt plan
```

**Success:** Plan shows creation of:

- 1 `aws_security_group` named `tokenoverflow-prod-rds`
- 2 `aws_vpc_security_group_ingress_rule` (port 5432,
    one for `10.0.10.0/24`, one for `10.0.11.0/24`)
- 1 `aws_db_instance` with:
    - `identifier = "tokenoverflow-prod"`
    - `engine = "postgres"`
    - `engine_version` starting with `17`
    - `instance_class = "db.t4g.micro"`
    - `allocated_storage = 20`
    - `max_allocated_storage = 100`
    - `storage_type = "gp3"`
    - `storage_encrypted = true`
    - `multi_az = false`
    - `publicly_accessible = false`
    - `deletion_protection = true`
    - `manage_master_user_password = false`
    - `db_name = "main-prod"`
    - `username = "tokenoverflow"`
    - `password_wo` shown as `(write-only attribute)`
- 1 `aws_db_parameter_group` (postgres17 family)
- Tags include `Environment = prod`

Plan should NOT show any changes to VPC resources.
Plan should NOT display the password value anywhere.

#### 4. Post-apply: RDS endpoint contains environment suffix

After apply, verify the endpoint:

```bash
RDS_ENDPOINT=$(cd infra/terraform/live/prod/rds && terragrunt output -raw db_instance_address)
echo "RDS endpoint: $RDS_ENDPOINT"
```

**Success:** Returns a hostname matching `tokenoverflow-prod.*.us-east-1.rds.amazonaws.com`.

#### 5. Post-apply: Password is NOT in state

```bash
cd infra/terraform/live/prod/rds
terragrunt state show 'module.rds.aws_db_instance.this[0]' | grep -i password
```

**Success:** Either no output, or `password_wo` shown as `(write-only attribute)`.

#### 6. Post-apply: Security group has correct per-subnet rules

```bash
SG_ID=$(cd infra/terraform/live/prod/rds && terragrunt output -raw security_group_id)
aws ec2 describe-security-group-rules \
  --filters "Name=group-id,Values=$SG_ID" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'SecurityGroupRules[?IsEgress==`false`].{Port:FromPort,CIDR:CidrIpv4}'
```

**Success:** Shows two ingress rules:
- Port 5432, CIDR `10.0.10.0/24`
- Port 5432, CIDR `10.0.11.0/24`

#### 7. Post-apply: Instance is not publicly accessible

```bash
aws rds describe-db-instances \
  --db-instance-identifier tokenoverflow-prod \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'DBInstances[0].{PubliclyAccessible:PubliclyAccessible,StorageEncrypted:StorageEncrypted,MultiAZ:MultiAZ,DBName:DBName}'
```

**Success:**

```json
{
    "PubliclyAccessible": false,
    "StorageEncrypted": true,
    "MultiAZ": false,
    "DBName": "main-prod"
}
```

---

## Documentation Changes

### Files to Update

| File | Change |
|------|--------|
| `infra/terraform/README.md` | Add RDS section with instance details and deploy commands |

### Content to Add to `infra/terraform/README.md`

Add the following after the existing VPC section:

```markdown
## RDS PostgreSQL

The RDS module (`modules/rds/`) provisions a PostgreSQL instance with:

| Setting | Value |
|---------|-------|
| Engine | PostgreSQL 17 |
| Instance | db.t4g.micro (single-AZ) |
| Storage | 20 GB gp3 (autoscales to 100 GB) |
| Encryption | AWS managed key (aws/rds) |
| Credentials | Write-only (`password_wo`) -- never stored in state |
| Network | Isolated subnets, SG allows port 5432 from private subnet CIDRs |

### Deploy

    ```shell
    $ export TF_VAR_password_wo='<your-password>'
    $ cd infra/terraform/live/prod/rds
    $ terragrunt init
    $ terragrunt plan
    $ terragrunt apply
    ```

### Rotate Password

    ```shell
    $ export TF_VAR_password_wo='<new-password>'
    $ export TF_VAR_password_wo_version=2
    $ cd infra/terraform/live/prod/rds
    $ terragrunt apply
    ```
```

### Files NOT Updated

Historical design documents are not updated. They are a snapshot of the
codebase at the time they were written.

---

## Development Environment Changes

### Brewfile

No changes needed. `tofuenv` is already installed and handles multiple OpenTofu
versions.

### Environment Variables

No new permanent environment variables are introduced. The `TF_VAR_password_wo`
and `TF_VAR_password_wo_version` variables are only needed at apply time and
should NOT be persisted in any dotfile or committed to the repository.

### Setup Flow

The `source scripts/src/includes.sh && setup` command continues to work. The
`setup_opentofu` function already reads `.opentofu-version` and installs the
specified version, so upgrading to 1.11.5 is automatic for new engineers.

---

## Tasks

### Task 1: Upgrade OpenTofu from 1.10.7 to 1.11.5

**What:** Update `.opentofu-version`, install the new version, re-initialize
existing units, and verify zero drift.

**Steps:**

1. `echo "1.11.5" > .opentofu-version`
2. `tofuenv install && tofuenv use 1.11.5`
3. Log in to root account: `aws sso login --profile tokenoverflow-root-admin`
4. Re-initialize global units:

   ```bash
   cd infra/terraform/live/global/aws-organizations && terragrunt init -upgrade
   cd ../aws-sso && terragrunt init -upgrade
   ```

5. Verify zero drift: `tg plan global`
6. Log in to prod account: `aws sso login --profile tokenoverflow-prod-admin`
7. Re-initialize prod units:

   ```bash
   cd infra/terraform/live/prod/vpc && terragrunt init -upgrade
   ```

8. Verify zero drift: `tg plan prod`
9. Commit:

   ```bash
   git add .opentofu-version
   git add infra/terraform/live/global/aws-organizations/.terraform.lock.hcl
   git add infra/terraform/live/global/aws-sso/.terraform.lock.hcl
   git add infra/terraform/live/prod/vpc/.terraform.lock.hcl
   git commit -m "infra: upgrade OpenTofu from 1.10.7 to 1.11.5"
   ```

**Success:** `tofu version` shows 1.11.5. All existing units show zero
changes in plan output. Lock files are updated and committed.

### Task 2: Create the RDS wrapper module

**What:** Create `infra/terraform/modules/rds/` with `main.tf`,
`variables.tf`, and `outputs.tf`.

**Steps:**

1. `mkdir -p infra/terraform/modules/rds`
2. Create `variables.tf` with the content from the Interfaces section
3. Create `main.tf` with the content from the Interfaces section
4. Create `outputs.tf` with the content from the Interfaces section
5. Run TFLint:

   ```bash
   cd infra/terraform/modules/rds
   tflint --config="$(pwd)/../../.tflint.hcl" --init
   tflint --config="$(pwd)/../../.tflint.hcl"
   ```

**Success:** TFLint passes with no errors. The three files follow the same
patterns as existing modules (`vpc`, `aws-organizations`, `aws-sso`).

### Task 3: Create the prod Terragrunt unit

**What:** Create `infra/terraform/live/prod/rds/terragrunt.hcl` with
prod-specific inputs and VPC dependency.

**Steps:**

1. `mkdir -p infra/terraform/live/prod/rds`
2. Create `terragrunt.hcl` with the content from the Interfaces section

**Success:** File exists and follows the same pattern as existing units.
The `dependency "vpc"` block references `../vpc`. Inputs include
`db_name = "main-prod"` and `private_subnet_cidrs` with the two private
subnet CIDRs.

### Task 4: Deploy the prod RDS instance

**What:** Initialize, plan, and apply the RDS infrastructure.

**Steps:**

1. Log in: `aws sso login --profile tokenoverflow-prod-admin`
2. Initialize:

   ```bash
   cd infra/terraform/live/prod/rds
   terragrunt init
   ```

3. Set the password:

   ```bash
   export TF_VAR_password_wo='<chosen-password>'
   ```

4. Plan: `terragrunt plan` -- review output against the expected resource list
   in Logic Phase 4. Confirm the password does NOT appear in the plan output.
5. Apply: `terragrunt apply` (takes 5-15 minutes)
6. Verify outputs: `terragrunt output`
7. Verify password is not in state (see Test Plan section 5)
8. Verify security group rules (see Test Plan section 6)
9. Verify instance is not publicly accessible (see Test Plan section 7)
10. Commit:

    ```bash
    git add infra/terraform/modules/rds/
    git add infra/terraform/live/prod/rds/
    git commit -m "infra: add prod RDS PostgreSQL 17 with write-only credentials"
    ```

**Success:** All outputs return valid values. Password is NOT visible in
state. Security group has two ingress rules (one per private subnet CIDR) on
port 5432. Instance identifier is `tokenoverflow-prod`. Database name is
`main-prod`. Instance is not publicly accessible and has storage encryption
enabled.

### Task 5: Update documentation

**What:** Update `infra/terraform/README.md` with RDS information.

**Steps:**

1. Add RDS section to `infra/terraform/README.md` (see Documentation Changes
   section)
2. Commit:

   ```bash
   git add infra/terraform/README.md
   git commit -m "docs: add RDS PostgreSQL documentation to terraform README"
   ```

**Success:** README accurately describes the RDS instance configuration,
credential management approach, rotation procedure, and deploy workflow.
