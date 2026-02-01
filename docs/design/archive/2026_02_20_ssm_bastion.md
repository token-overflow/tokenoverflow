# Design: SSM Bastion for Production Database Access

## Architecture Overview

### Goal

Deploy a dedicated SSM bastion host in a private subnet to enable secure access
to the production RDS (PostgreSQL 17, in isolated subnets with no public
access) from local machines. The bastion supports both SSM port forwarding
(for CLI tools like psql) and SSH tunneling via SSM ProxyCommand (for native
integration with DataGrip and DBeaver).

### Scope

This design covers:

- Dedicated EC2 bastion instance via ASG (spot, auto-replace on interruption)
- IAM role with SSM managed policy
- SSH key pair for DataGrip/DBeaver native SSH tunnel support
- Security group allowing bastion-to-RDS access on port 5432
- Helper script for SSM port forwarding (`db-tunnel.sh`)
- SSH config template for DataGrip/DBeaver integration
- Cost analysis

This design does NOT cover:

- VPN or Tailscale (evaluated and rejected; see Decision section)
- Public bastion (no public IP; SSH only reachable through SSM proxy)
- Multi-region bastion (single region, us-east-1)
- Automated session logging to S3/CloudWatch (deferred)
- MFA enforcement on SSM sessions (deferred)

### Why a Dedicated SSM Bastion

Six options were evaluated:

| Option | Monthly Cost | DB Client Support | Complexity |
|--------|-------------|-------------------|------------|
| SSM via fck-nat | $0 extra | No SSH tunnel | Repurposes NAT (bad isolation) |
| **Dedicated bastion (spot)** | **~$1.59** | **Native SSH tunnel** | **Low** |
| ECS Exec | $0 extra | No SSH tunnel | Task must be running |
| AWS Client VPN | ~$81 | Full VPN | High cost |
| Tailscale | ~$6.59 | Full VPN | Extra dependency |
| Public RDS | $0 extra | Direct connect | Security risk |

**Decision: Dedicated SSM bastion.** At ~$1.59/month (spot t4g.nano + 8GB
gp3), it provides native SSH tunnel support for DataGrip/DBeaver, complete
isolation from other infrastructure, and automatic recovery via ASG.

### How SSM Works from a Private Subnet

The bastion has no public IP. It sits in a private subnet with internet access
through fck-nat. The SSM agent on the bastion makes outbound HTTPS calls to
AWS SSM endpoints (routed through fck-nat). Your local `aws ssm start-session`
also connects to the SSM service. The SSM service brokers both sides -- no
inbound ports are needed on the bastion.

```text
  Local Machine                    AWS Cloud
  +-------------+                  +----------------------------------+
  | DataGrip    |                  |  VPC 10.0.0.0/16                 |
  |  SSH tunnel |                  |                                  |
  |  via SSM    |<-- SSM Service ->|  Private Subnet (10.0.10.0/24)   |
  |  ProxyCmd   |                  |  +----------+                    |
  +-------------+                  |  | Bastion  |--- port 5432 --->  |
                                   |  | t4g.nano |    Isolated Subnet |
  +-------------+                  |  | (spot)   |    +----------+    |
  | psql        |                  |  +----------+    |   RDS    |    |
  |  localhost   |<-- SSM port  -->|       |          | PG 17    |    |
  |  :5432      |    forwarding    |       v          +----------+    |
  +-------------+                  |  fck-nat (outbound HTTPS to SSM) |
                                   +----------------------------------+
```

### Cost Analysis

| Component | Monthly Cost |
|-----------|-------------|
| t4g.nano spot instance (24/7) | ~$0.95 |
| 8 GB gp3 EBS volume | ~$0.64 |
| SSM sessions | $0 |
| **Total** | **~$1.59** |

Spot pricing is based on us-east-1 historical rates for t4g.nano (~$0.0013/hr).
The ASG with mixed instances policy falls back to on-demand (~$0.0042/hr,
~$3.07/month) if spot capacity is unavailable.

### Instance Type Decision

| Instance Type | Arch | vCPUs | Memory | Monthly (spot) | Monthly (on-demand) |
|---------------|------|-------|--------|----------------|---------------------|
| t4g.nano | ARM | 2 | 0.5 GB | ~$0.95 | ~$3.07 |
| t4g.micro | ARM | 2 | 1 GB | ~$1.87 | ~$6.05 |

**Decision: t4g.nano.** A bastion only runs SSM agent and sshd. 0.5 GB
memory is more than sufficient. Matches the fck-nat instance type choice.

### Spot via ASG

The bastion uses a mixed instances policy ASG (min=max=desired=1) with spot
preferred and on-demand fallback:

- **Spot preferred**: ~69% cost savings vs on-demand
- **Capacity rebalancing**: ASG proactively replaces instances when AWS
  signals upcoming spot interruption
- **On-demand fallback**: If spot is unavailable, ASG launches on-demand
  (seamless, no manual intervention)
- **Always-on**: No start/stop complexity. Simplest UX at ~$1.59/month

### SSH Key Pair

An SSH key pair is needed for DataGrip's native SSH tunnel feature. The
public key is managed via Terraform (`aws_key_pair`), and the private key
is stored locally on the developer's machine.

Generate the key:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/bastion -C "bastion"
```

The public key is passed to Terraform via `TF_VAR_ssh_public_key`.

### Network Architecture

```text
            Internet
               |
          [IGW] (existing)
               |
+--------------+-----------------------------+
|         VPC 10.0.0.0/16                    |
|                                            |
|  Tier 1: Public Subnets                    |
|  +------------------+  +----------------+ |
|  | 10.0.0.0/24      |  | 10.0.1.0/24    | |
|  | [fck-nat]        |  |                 | |
|  +--------+---------+  +----------------+ |
|           |                                |
|  Tier 2: Private Subnets                   |
|  +------------------+  +----------------+ |
|  | 10.0.10.0/24     |  | 10.0.11.0/24   | |
|  | [Bastion]        |  | (Lambda)       | |
|  | [Lambda]         |  |                 | |
|  +--------+---------+  +----------------+ |
|           |                                |
|  Tier 3: Isolated Subnets                  |
|  +------------------+  +----------------+ |
|  | 10.0.20.0/24     |  | 10.0.21.0/24   | |
|  | [RDS]            |  |                 | |
|  +------------------+  +----------------+ |
+--------------------------------------------+
```

**Traffic flow (SSH tunnel via SSM):**

1. DataGrip opens SSH connection using SSM ProxyCommand
2. SSM agent on bastion accepts the SSH session
3. DataGrip creates SSH tunnel: `localhost:5432 -> bastion -> RDS:5432`
4. SQL queries flow through the tunnel to RDS

**Traffic flow (SSM port forwarding):**

1. `db-tunnel.sh` discovers bastion instance ID from ASG
2. Opens SSM port forwarding: `localhost:5432 -> bastion -> RDS:5432`
3. `psql -h localhost -p 5432` connects through the tunnel

### Module Strategy

Consistent with the VPC, RDS, NAT, and Lambda modules:

```text
infra/terraform/
  modules/
    bastion/                       # new module
      main.tf                      # EC2, ASG, SG, key pair, launch template
      variables.tf
      outputs.tf
      iam.tf                       # IAM role + SSM policy
  live/
    prod/
      bastion/                     # new Terragrunt unit
        terragrunt.hcl
```

### Terragrunt Dependencies

```text
vpc ─────> nat ─────> bastion ─────> (RDS ingress update)
  └────────────────────> rds ────────┘
```

The bastion depends on VPC (subnet, vpc_id) and NAT (for internet access so
the SSM agent can reach AWS endpoints). The RDS module is modified to accept
the bastion's security group ID for an ingress rule.

### Directory Structure (After)

```text
infra/terraform/
  modules/
    vpc/                     # existing
    rds/                     # modified (bastion SG ingress)
    nat/                     # existing
    lambda/                  # existing
    api_gateway/             # existing
    bastion/                 # new
      main.tf
      variables.tf
      outputs.tf
      iam.tf
  live/
    prod/
      vpc/                   # existing (unchanged)
      nat/                   # existing (unchanged)
      rds/                   # modified (bastion dependency)
      lambda/                # existing (unchanged)
      api_gateway/           # existing (unchanged)
      bastion/               # new
        terragrunt.hcl
```

### DB Client Integration

| Client | Method | Setup |
|--------|--------|-------|
| DataGrip | Native SSH tunnel via SSH config | Add SSH config, configure DataGrip SSH host |
| DBeaver | Built-in SSH tunnel config | Configure SSH with SSM ProxyCommand |
| psql/CLI | `db-tunnel.sh` + localhost | Run script, connect to localhost:5432 |

---

## Interfaces

This section documents every file that will be created or modified, and the
contracts between them.

### New Files

#### `infra/terraform/modules/bastion/variables.tf`

```hcl
variable "env_name" {
  description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC where the bastion will be deployed."
  type        = string
}

variable "subnet_id" {
  description = "ID of the private subnet where the bastion will be placed."
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type for the bastion."
  type        = string
  default     = "t4g.nano"
}

variable "ssh_public_key" {
  description = "SSH public key for the bastion key pair. Generate with: ssh-keygen -t ed25519 -f ~/.ssh/bastion"
  type        = string
}
```

#### `infra/terraform/modules/bastion/iam.tf`

```hcl
resource "aws_iam_role" "bastion" {
  name = "bastion_ssm"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
    }]
  })

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_iam_role_policy_attachment" "ssm" {
  role       = aws_iam_role.bastion.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_instance_profile" "bastion" {
  name = "bastion_ssm"
  role = aws_iam_role.bastion.name
}
```

#### `infra/terraform/modules/bastion/main.tf`

```hcl
data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

resource "aws_key_pair" "bastion" {
  key_name   = "bastion"
  public_key = var.ssh_public_key

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_security_group" "bastion" {
  name        = "bastion"
  description = "Security group for SSM bastion (no inbound needed)"
  vpc_id      = var.vpc_id

  tags = {
    Name        = "bastion"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_vpc_security_group_egress_rule" "all_outbound" {
  security_group_id = aws_security_group.bastion.id
  description       = "Allow all outbound traffic (SSM agent, DB access)"
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}

resource "aws_launch_template" "bastion" {
  name          = "bastion"
  image_id      = data.aws_ssm_parameter.al2023_arm64.value
  instance_type = var.instance_type
  key_name      = aws_key_pair.bastion.key_name

  iam_instance_profile {
    arn = aws_iam_instance_profile.bastion.arn
  }

  network_interfaces {
    associate_public_ip_address = false
    security_groups             = [aws_security_group.bastion.id]
  }

  metadata_options {
    http_endpoint = "enabled"
    http_tokens   = "required"
  }

  block_device_mappings {
    device_name = "/dev/xvda"
    ebs {
      volume_size = 8
      volume_type = "gp3"
      encrypted   = true
    }
  }

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name        = "bastion"
      Environment = var.env_name
      ManagedBy   = "opentofu"
    }
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_autoscaling_group" "bastion" {
  name                = "bastion"
  min_size            = 1
  max_size            = 1
  desired_capacity    = 1
  vpc_zone_identifier = [var.subnet_id]
  capacity_rebalancing = true

  mixed_instances_policy {
    instances_distribution {
      on_demand_base_capacity                  = 0
      on_demand_percentage_above_base_capacity = 0
      spot_allocation_strategy                 = "capacity-optimized"
    }
    launch_template {
      launch_template_specification {
        launch_template_id = aws_launch_template.bastion.id
        version            = "$Latest"
      }
    }
  }

  tag {
    key                 = "Name"
    value               = "bastion"
    propagate_at_launch = true
  }

  tag {
    key                 = "Environment"
    value               = var.env_name
    propagate_at_launch = true
  }

  tag {
    key                 = "ManagedBy"
    value               = "opentofu"
    propagate_at_launch = true
  }
}
```

#### `infra/terraform/modules/bastion/outputs.tf`

```hcl
output "security_group_id" {
  description = "The ID of the bastion security group."
  value       = aws_security_group.bastion.id
}

output "autoscaling_group_arn" {
  description = "The ARN of the bastion autoscaling group."
  value       = aws_autoscaling_group.bastion.arn
}

output "autoscaling_group_name" {
  description = "The name of the bastion autoscaling group."
  value       = aws_autoscaling_group.bastion.name
}

output "instance_profile_arn" {
  description = "The ARN of the bastion instance profile."
  value       = aws_iam_instance_profile.bastion.arn
}
```

#### `infra/terraform/live/prod/bastion/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/bastion"
}

dependency "vpc" {
  config_path = "../vpc"
}

dependency "nat" {
  config_path = "../nat"
}

inputs = {
  env_name  = "prod"
  vpc_id    = dependency.vpc.outputs.vpc_id
  subnet_id = dependency.vpc.outputs.private_subnet_ids[0]

  # ssh_public_key is provided via environment variable:
  #   TF_VAR_ssh_public_key="$(cat ~/.ssh/bastion.pub)"
}
```

#### `scripts/src/db-tunnel.sh`

A convenience script that discovers the bastion instance ID from the ASG
and opens an SSM port forwarding session to the RDS endpoint.

#### `scripts/src/ssh-config-bastion.example`

An SSH ProxyCommand config template for DataGrip/DBeaver native SSH tunnel
integration.

### Modified Files

#### `infra/terraform/modules/rds/variables.tf`

Add:

```hcl
variable "bastion_security_group_id" {
  description = "Security group ID of the bastion. When set, allows PostgreSQL access from the bastion."
  type        = string
  default     = ""
}
```

#### `infra/terraform/modules/rds/security_groups.tf`

Add after the existing `aws_vpc_security_group_ingress_rule.postgresql`:

```hcl
resource "aws_vpc_security_group_ingress_rule" "bastion" {
  count = var.bastion_security_group_id != "" ? 1 : 0

  security_group_id            = aws_security_group.rds.id
  description                  = "Allow PostgreSQL from bastion"
  from_port                    = 5432
  to_port                      = 5432
  ip_protocol                  = "tcp"
  referenced_security_group_id = var.bastion_security_group_id
}
```

#### `infra/terraform/live/prod/rds/terragrunt.hcl`

Add bastion dependency and pass security group ID:

```hcl
dependency "bastion" {
  config_path = "../bastion"
}

# Add to inputs:
bastion_security_group_id = dependency.bastion.outputs.security_group_id
```

### Files NOT Modified

| File | Reason |
|------|--------|
| `modules/vpc/*` | No new subnets or outputs needed. |
| `modules/nat/*` | NAT is independent. Bastion uses it implicitly. |
| `modules/lambda/*` | Lambda is independent. |
| `live/prod/vpc/terragrunt.hcl` | No changes needed. |
| `live/prod/nat/terragrunt.hcl` | No changes needed. |
| `live/prod/lambda/terragrunt.hcl` | No changes needed. |

---

## Logic

### Phase 1: Create the bastion module

**Step 1.1:** Create the module directory and files:

```bash
mkdir -p infra/terraform/modules/bastion
```

**Step 1.2:** Create `variables.tf`, `iam.tf`, `main.tf`, and `outputs.tf`
with the content from the Interfaces section.

**Step 1.3:** Validate with TFLint:

```bash
cd infra/terraform/modules/bastion
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

### Phase 2: Modify the RDS module

**Step 2.1:** Add `bastion_security_group_id` variable to
`infra/terraform/modules/rds/variables.tf`.

**Step 2.2:** Add bastion ingress rule to
`infra/terraform/modules/rds/security_groups.tf`.

### Phase 3: Create the prod Terragrunt unit

**Step 3.1:** Create the bastion unit:

```bash
mkdir -p infra/terraform/live/prod/bastion
```

**Step 3.2:** Create `terragrunt.hcl` with VPC and NAT dependencies.

**Step 3.3:** Update `live/prod/rds/terragrunt.hcl` with bastion dependency.

### Phase 4: Create helper scripts

**Step 4.1:** Create `scripts/src/db-tunnel.sh` for SSM port forwarding.

**Step 4.2:** Create `scripts/src/ssh-config-bastion.example` for
DataGrip/DBeaver SSH tunnel config.

### Phase 5: Deploy (manual)

**Step 5.1:** Generate SSH key pair (if not already done):

```bash
ssh-keygen -t ed25519 -f ~/.ssh/bastion -C "bastion"
```

**Step 5.2:** Deploy bastion:

```bash
cd infra/terraform/live/prod/bastion
TF_VAR_ssh_public_key="$(cat ~/.ssh/bastion.pub)" terragrunt init
TF_VAR_ssh_public_key="$(cat ~/.ssh/bastion.pub)" terragrunt plan
TF_VAR_ssh_public_key="$(cat ~/.ssh/bastion.pub)" terragrunt apply
```

**Step 5.3:** Update RDS to allow bastion access:

```bash
cd infra/terraform/live/prod/rds
TF_VAR_password_wo=<password> terragrunt plan
TF_VAR_password_wo=<password> terragrunt apply
```

**Step 5.4:** Verify bastion is running:

```bash
aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names bastion \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'AutoScalingGroups[0].Instances[0].{Id:InstanceId,State:LifecycleState}'
```

**Step 5.5:** Test SSM connectivity:

```bash
INSTANCE_ID=$(aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names bastion \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'AutoScalingGroups[0].Instances[0].InstanceId' \
  --output text)

aws ssm start-session \
  --target "$INSTANCE_ID" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1
```

---

## Edge Cases & Constraints

### 1. Spot interruption

**Risk:** AWS may reclaim the spot instance at any time with a 2-minute
warning.

**Mitigation:** The ASG with `capacity_rebalancing` proactively replaces
the instance when AWS signals upcoming interruption. The mixed instances
policy falls back to on-demand if spot is unavailable. Recovery time is
typically 1-2 minutes.

### 2. SSM agent requires outbound internet

**Risk:** If fck-nat goes down, the bastion's SSM agent loses connectivity
to AWS endpoints, making the bastion unreachable.

**Mitigation:** fck-nat has its own ASG with auto-recovery. Both the NAT
and bastion ASGs would recover within minutes. This is an accepted
dependency chain for a development/debugging tool.

### 3. SSH key rotation

**Risk:** The SSH key pair is long-lived. If the private key is compromised,
an attacker with AWS SSO credentials could SSH to the bastion.

**Mitigation:** The bastion has no public IP and is only reachable through
SSM, which requires valid AWS credentials. An attacker would need both the
SSH private key AND AWS SSO access. To rotate: generate a new key pair,
update `TF_VAR_ssh_public_key`, and apply.

### 4. RDS password not stored in bastion

**Risk:** The bastion provides network access to RDS but does not store
database credentials.

**Mitigation:** This is by design. Database credentials are managed
separately (SSM Parameter Store for Lambda, manual entry for DataGrip/psql).
The bastion is purely a network tunnel.

### 5. Single AZ placement

**Risk:** The bastion is in a single private subnet (us-east-1a). If that
AZ has issues, the bastion is unavailable.

**Mitigation:** This is a development/debugging tool, not a production
workload. Single-AZ is acceptable for ~$1.59/month. If multi-AZ is needed,
add a second subnet to `vpc_zone_identifier`.

### 6. ASG name collision

**Risk:** The ASG is named `bastion` without environment suffix. If we add
a dev environment later, the names could collide.

**Mitigation:** Each environment is a separate AWS account (per CLAUDE.md
rules), so names cannot collide across environments.

---

## Test Plan

### Verification Checklist

#### 1. TFLint passes on the bastion module

```bash
cd infra/terraform/modules/bastion
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

**Success:** No errors or warnings.

#### 2. Bastion plan creates expected resources

```bash
cd infra/terraform/live/prod/bastion
TF_VAR_ssh_public_key="$(cat ~/.ssh/bastion.pub)" terragrunt plan
```

**Success:** Plan shows creation of:

- 1 `aws_key_pair` named `bastion`
- 1 `aws_security_group` named `bastion`
- 1 `aws_vpc_security_group_egress_rule` (all outbound)
- 1 `aws_launch_template` named `bastion` with `instance_type = "t4g.nano"`
- 1 `aws_autoscaling_group` named `bastion` with `min_size = max_size = 1`
- 1 `aws_iam_role` named `bastion_ssm`
- 1 `aws_iam_instance_profile` named `bastion_ssm`
- 1 `aws_iam_role_policy_attachment` for SSM

#### 3. RDS plan shows bastion ingress rule

```bash
cd infra/terraform/live/prod/rds
TF_VAR_password_wo=dummy terragrunt plan
```

**Success:** Plan shows creation of 1 new
`aws_vpc_security_group_ingress_rule` allowing port 5432 from the bastion
security group. No other RDS changes.

#### 4. Existing infrastructure is unaffected

```bash
source scripts/src/includes.sh
tg plan prod
```

**Success:** VPC, NAT, Lambda, and API Gateway units show "No changes."

#### 5. Post-apply: ASG has a running instance

```bash
aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names bastion \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'AutoScalingGroups[0].{DesiredCapacity:DesiredCapacity,Running:Instances[?LifecycleState==`InService`]|length(@)}'
```

**Success:** Returns `{ "DesiredCapacity": 1, "Running": 1 }`.

#### 6. Post-apply: SSM session works

```bash
./scripts/src/db-tunnel.sh
# In another terminal:
psql -h localhost -p 5432 -U tokenoverflow tokenoverflow
```

**Success:** psql connects to the RDS instance through the tunnel.

#### 7. Post-apply: DataGrip SSH tunnel works

1. Copy `scripts/src/ssh-config-bastion.example` to `~/.ssh/config`
2. Update `<instance-id>` with the actual bastion instance ID
3. In DataGrip: SSH tunnel using host `bastion_prod`, port 22
4. Remote host: RDS endpoint, port 5432

**Success:** DataGrip connects to RDS through the SSH tunnel.

---

## Documentation Changes

### Files to Update

| File | Change |
|------|--------|
| `infra/terraform/README.md` | Add bastion section |

### Content to Add to `infra/terraform/README.md`

Add after the NAT section:

```markdown
## Bastion (SSM)

The bastion module (`modules/bastion/`) provides secure access to the
production RDS instance from local machines via SSM.

| Setting | Value |
|---------|-------|
| Instance type | t4g.nano spot (~$0.95/month) |
| HA mode | ASG min=max=1, auto-replace on spot interruption |
| Placement | First private subnet (us-east-1a) |
| SSH | Enabled (via SSM ProxyCommand only, no public IP) |
| SSM | Enabled (AmazonSSMManagedInstanceCore) |

### Connect via psql

    $ ./scripts/src/db-tunnel.sh
    # In another terminal:
    $ psql -h localhost -p 5432 -U tokenoverflow tokenoverflow

### Connect via DataGrip

1. Copy `scripts/src/ssh-config-bastion.example` to `~/.ssh/config`
2. In DataGrip: SSH/SSL tab -> Use SSH tunnel
3. SSH config: `bastion_prod` host
4. Remote host: RDS endpoint, port 5432
```

---

## Development Environment Changes

### Brewfile

No changes needed. `session-manager-plugin` is assumed to be installed
separately (not available via Homebrew). Install via:

```bash
brew install --cask session-manager-plugin
```

### Environment Variables

| Variable | Purpose | Scope |
|----------|---------|-------|
| `TF_VAR_ssh_public_key` | SSH public key for bastion key pair | `terragrunt plan/apply` on bastion unit |

### Setup Flow

No changes to `source scripts/src/includes.sh && setup`. The bastion is
infrastructure-only and does not affect the application development workflow.

---

## Tasks

### Task 1: Create the bastion Terraform module

**What:** Create `infra/terraform/modules/bastion/` with `main.tf`,
`variables.tf`, `outputs.tf`, and `iam.tf`.

**Steps:**

1. `mkdir -p infra/terraform/modules/bastion`
2. Create all four files with content from the Interfaces section
3. Run TFLint

**Success:** TFLint passes. All AWS resource names use snake_case. No
`tokenoverflow-` prefix or `-{env}` suffix on resource names.

### Task 2: Modify the RDS module

**What:** Add `bastion_security_group_id` variable and ingress rule.

**Steps:**

1. Add variable to `modules/rds/variables.tf`
2. Add conditional ingress rule to `modules/rds/security_groups.tf`

**Success:** `count = 0` when variable is empty (backward compatible).
`count = 1` when bastion SG ID is provided.

### Task 3: Create Terragrunt units

**What:** Create `live/prod/bastion/terragrunt.hcl` and update
`live/prod/rds/terragrunt.hcl`.

**Steps:**

1. Create bastion unit with VPC and NAT dependencies
2. Update RDS unit with bastion dependency and security group ID

**Success:** Dependency chain: vpc -> nat -> bastion -> rds (for SG rule).

### Task 4: Create helper scripts

**What:** Create `scripts/src/db-tunnel.sh` and
`scripts/src/ssh-config-bastion.example`.

**Steps:**

1. Create port forwarding script
2. Create SSH config template

**Success:** `db-tunnel.sh` discovers instance ID and opens SSM tunnel.
SSH config works with DataGrip's native SSH tunnel feature.

### Task 5: Deploy to production (manual)

**What:** Deploy bastion and update RDS security group.

**Steps:**

1. Generate SSH key pair
2. Deploy bastion unit
3. Apply RDS changes
4. Verify connectivity

**Success:** psql connects to RDS via localhost:5432 tunnel.
