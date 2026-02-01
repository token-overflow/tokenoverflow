# Design: Pgbouncer Ec2

## Architecture Overview

### Goal

Deploy PgBouncer on a t4g.nano EC2 instance in the production VPC's private
subnet to pool database connections for the Lambda-based API. PgBouncer sits
between Lambda and RDS, consolidating per-invocation connections into a small
pool of persistent server connections. This prevents connection exhaustion on the
db.t4g.micro RDS instance (~112 max connections) and mirrors the local
docker-compose PgBouncer setup already used in development.

### Scope

This design covers:

- PgBouncer EC2 instance deployed via ASG (min=max=desired=1) for auto-recovery
- Dedicated ENI with a static private IP for stable connectivity across instance
  replacements (AL2023 auto-configures policy routing via `amazon-ec2-net-utils`)
- Security group configuration (PgBouncer SG with ingress from Lambda and
  bastion, plus RDS SG ingress rule)
- User-data script for ENI attachment, PgBouncer installation and configuration
  via PGDG repo and systemd
- Lambda module changes to inject `TOKENOVERFLOW__DATABASE__HOST` and
  `TOKENOVERFLOW__DATABASE__PORT` environment variables
- Production config update to route through PgBouncer (port 6432)
- Terragrunt live config for prod

This design does NOT cover:

- PgBouncer admin console exposure (disabled; not needed)
- CloudWatch custom metrics for PgBouncer (deferred; basic EC2 monitoring is
  free and sufficient)
- Multi-AZ PgBouncer deployment (single instance is acceptable for MVP)
- Dev environment PgBouncer EC2 (local docker-compose already provides this)
- TLS between Lambda and PgBouncer (both in private subnets within the same
  VPC; PgBouncer-to-RDS uses `server_tls_sslmode = require`)

### Why PgBouncer Instead of RDS Proxy

The API is migrating from Fargate (persistent connections) to Lambda (ephemeral
connections). Each Lambda invocation opens a new database connection, which
exhausts RDS connection limits quickly under even moderate concurrency. A
connection pooler is required.

| Criteria | AWS RDS Proxy | PgBouncer (t4g.nano) |
|----------|---------------|----------------------|
| Monthly cost | ~$20+ (vCPU-based pricing) | ~$3.71 (EC2 + EBS) |
| Managed | Yes | No (ASG auto-recovery) |
| Protocol support | PostgreSQL wire protocol | Full PostgreSQL wire protocol |
| Transaction pooling | Yes | Yes |
| Prepared statements | Limited support | Full support (max_prepared_statements) |
| Operational overhead | Zero | Minimal (ASG handles replacement) |
| Matches local setup | No | Yes (mirrors docker-compose) |

**Decision: PgBouncer on EC2.** At roughly 1/5th the cost of RDS Proxy, with
full prepared statement support and parity with the local development
environment, PgBouncer is the clear choice for an MVP. If operational overhead
becomes a concern at scale, migrating to RDS Proxy is straightforward since the
application only needs a host/port change.

### Why Native Install Instead of Docker

Docker was considered for ease of configuration (env vars instead of config
files). However, the Docker daemon uses 200-400 MB of RAM, which is problematic
on a t4g.nano (512 MB total). There is also a
[known AL2023 race condition](https://github.com/amazonlinux/amazon-linux-2023/issues/397)
where `dnf install docker` in user-data fails ~90% of the time due to SSM
auto-update conflicts.

| Approach | Daemon RAM | PgBouncer Config | Reliability |
|----------|-----------|-----------------|-------------|
| Docker (`bitnami/pgbouncer`) | 200-400 MB | Env vars | Risky on 512 MB; AL2023 race condition |
| containerd + nerdctl | ~50 MB | Env vars | Better, but extra layer |
| Native (`dnf install pgbouncer`) | 0 MB | 15-line ini file | Most reliable, simplest |

**Decision: Native install.** PgBouncer itself uses 2-20 MB of RAM. Installing
it directly via the PGDG repo is one command (`dnf install -y pgbouncer`), gets
systemd integration for free, and leaves 400+ MB of headroom on the t4g.nano.
The `pgbouncer.ini` config file is ~15 lines -- not meaningfully harder than
Docker env vars. Upgrading to t4g.micro ($6.05/month) just to run Docker would
double the cost for no benefit.

### Instance Type Decision

PgBouncer is a single-threaded, event-driven process that uses less than 10 MB
of memory. The smallest available instance provides more than enough resources.

| Instance Type | Architecture | vCPUs | Memory | Monthly Cost |
|---------------|-------------|-------|--------|-------------|
| t4g.nano | ARM (Graviton2) | 2 | 512 MB | $3.07 |
| t4g.micro | ARM (Graviton2) | 2 | 1 GB | $6.05 |

**Decision: t4g.nano ($3.07/month).** PgBouncer's memory footprint is
negligible. The 512 MB on a t4g.nano is more than 50x what PgBouncer needs. The
ARM architecture matches the bastion instance pattern and benefits from
Graviton's price-performance advantage.

### High-Availability via ASG

| Mode | How It Works | Failover |
|------|-------------|----------|
| Bare EC2 | Single instance, no recovery | Manual: must re-create instance |
| ASG min=max=1 | ASG wraps the instance | Automatic: ASG replaces unhealthy instance in ~2-3 min |

**Decision: ASG min=max=desired=1.** The ASG itself is free; you only pay for
the EC2 instance. When the instance fails a health check or is terminated, the
ASG launches a replacement. This follows the exact same pattern as the bastion
module (`infra/terraform/modules/bastion/main.tf`).

### Stable Endpoint via Static ENI

Lambda needs a stable IP address to connect to PgBouncer. If the ASG replaces
the instance, a new instance gets a new primary IP. A dedicated ENI solves this:

1. Terraform creates an `aws_network_interface` with a fixed private IP in the
   private subnet (10.0.10.0/24)
2. The ENI is an independent resource that persists across instance replacements
3. User-data on each new instance attaches this ENI as a secondary interface
   (device_index=1)
4. AL2023's `amazon-ec2-net-utils` 2.x package **automatically** detects the
   new interface via udev rules, assigns the IP from IMDS, and configures
   policy routing via systemd-networkd -- no manual routing scripts needed
5. Lambda points to this fixed ENI IP on port 6432

The ENI is free. There is no additional cost for this approach.

**Why AL2023 makes this simple:** Amazon Linux 2023 ships with
`amazon-ec2-net-utils` 2.x (pre-installed), which is a complete rewrite of the
AL2 version built around systemd-networkd. When a secondary ENI is attached, it
automatically:

- Queries IMDS for the ENI's IP addresses and device index
- Generates systemd-networkd `.network` files in `/run/systemd/network/`
- Creates `[RoutingPolicyRule]` entries that route traffic sourced from the
  secondary ENI's IP through the secondary interface (not the primary)
- This prevents VPC source/destination check drops on asymmetric responses

No manual `ip rule`, `ip route`, or routing table configuration is needed. This
is the same mechanism that makes multi-ENI EKS pods work on AL2023.

### Network Architecture

```text
  Tier 2: Private Subnets
  +-------------------------------------+  +------------------+
  | 10.0.10.0/24 (us-east-1a)          |  | 10.0.11.0/24     |
  |                                     |  | (us-east-1b)     |
  |  [Bastion] ─────┐                  |  |                  |
  |                  │                  |  |                  |
  |  [Lambda]  ──────┤ port 6432       |  |  [Lambda]        |
  |                  ▼                  |  |    │             |
  |  [PgBouncer EC2]                    |  |    │  port 6432  |
  |   eth0: primary (ASG-assigned)      |  |    │             |
  |   eth1: dedicated ENI               |  |    │             |
  |    └─ fixed IP: 10.0.10.200:6432  ◄────────┘             |
  |              │                      |  |                  |
  +──────────────┼──────────────────────+  +------------------+
                 │
                 │ port 5432 (server_tls_sslmode = require)
                 ▼
  Tier 3: Isolated Subnets
  +----------------------------------+  +------------------+
  | 10.0.20.0/24 (us-east-1a)       |  | 10.0.21.0/24     |
  |                                  |  | (us-east-1b)     |
  |  [RDS PostgreSQL 17]            |  |                  |
  |   main.ccp4e4gum1b0...          |  |                  |
  +----------------------------------+  +------------------+
```

**Traffic flow:**

1. Lambda invocation starts in a private subnet, connects to PgBouncer's fixed
   ENI IP (10.0.10.200) on port 6432
2. PgBouncer receives the connection on eth1, authenticates via SCRAM-SHA-256,
   and assigns a pooled server connection
3. PgBouncer connects to RDS on port 5432 with `server_tls_sslmode = require`
4. Responses flow back: RDS -> PgBouncer -> Lambda (policy routing ensures
   responses from the ENI IP exit via eth1, handled automatically by AL2023)
5. When Lambda finishes, the client connection closes but PgBouncer keeps the
   server connection in the pool for the next invocation

**Bastion access:**

The bastion can also connect to PgBouncer on port 6432 (via the ENI IP),
enabling the bastion -> PgBouncer -> RDS route for database administration. The
PgBouncer SG allows ingress from the bastion SG on port 6432.

**Key points:**

- PgBouncer is in the first private subnet (us-east-1a, 10.0.10.0/24), same
  tier as Lambda and the bastion
- Lambda in us-east-1a connects to PgBouncer within the same AZ (no cross-AZ
  charges)
- Lambda in us-east-1b connecting to PgBouncer in us-east-1a incurs $0.01/GB
  cross-AZ charges (negligible at MVP traffic)
- PgBouncer reaches RDS in the isolated subnet via VPC local routing

### Dependency Graph

**Before (current):**

```text
VPC
 |-- NAT
 |-- RDS
 |-- Bastion (-> VPC, NAT)
 |-- Lambda (-> VPC, RDS)
 '-- API Gateway (-> Lambda)
```

**After:**

```text
VPC
 |-- NAT
 |-- RDS
 |-- Bastion (-> VPC, NAT)
 |-- PgBouncer (-> VPC, RDS, Lambda, Bastion)
 |-- Lambda (-> VPC, RDS)
 '-- API Gateway (-> Lambda)
```

The PgBouncer module depends on VPC (subnet, VPC ID), RDS (security group ID,
endpoint), Lambda (security group ID for ingress rules), and Bastion (security
group ID for ingress rules).

**Important: No circular dependency.** Lambda does NOT declare a Terragrunt
`dependency` on PgBouncer. The PgBouncer ENI IP (`10.0.10.200`) is a static
value hardcoded in the Lambda Terragrunt inputs. This avoids a circular
dependency (PgBouncer -> Lambda -> PgBouncer) that would prevent
`terragrunt run --all` from building a valid DAG. Since the IP is controlled by
Terraform and never changes (the ENI persists independently of instances),
hardcoding it in the Lambda live config is safe and simple.

### Cost Analysis

| Resource | Monthly Cost |
|----------|-------------|
| t4g.nano EC2 (on-demand) | $3.07 |
| 8 GB gp3 EBS | $0.64 |
| Dedicated ENI | $0.00 |
| ASG | $0.00 |
| **Total** | **~$3.71** |

For comparison, AWS RDS Proxy would cost ~$20+/month for a db.t4g.micro
instance. PgBouncer on EC2 saves approximately $16+/month.

### Module Strategy

Consistent with the bastion module pattern, a self-contained module is used:

```text
infra/terraform/
  modules/
    pgbouncer/
      main.tf                    # ENI, launch template, ASG, IAM
      security_groups.tf         # PgBouncer SG + RDS SG ingress
      variables.tf               # Module inputs
      outputs.tf                 # private_ip, security_group_id
      user_data.sh.tftpl         # ENI attach, PgBouncer install + config
  live/
    prod/
      pgbouncer/
        terragrunt.hcl           # Prod config
```

### Directory Structure (After)

```text
infra/terraform/
  modules/
    bastion/                     # existing (unchanged)
    nat/                         # existing (unchanged)
    rds/                         # existing (unchanged)
    vpc/                         # existing (unchanged)
    lambda/                      # existing (modified: new variables + env vars)
    pgbouncer/                   # new
      main.tf
      security_groups.tf
      variables.tf
      outputs.tf
      user_data.sh.tftpl
  live/
    prod/
      bastion/                   # existing (unchanged)
      nat/                       # existing (unchanged)
      rds/                       # existing (unchanged)
      vpc/                       # existing (unchanged)
      lambda/                    # existing (modified: new inputs)
        terragrunt.hcl
      pgbouncer/                 # new
        terragrunt.hcl
```

---

## Interfaces

This section documents every file that will be created or modified, and the
contracts between them.

### New Files

#### `infra/terraform/modules/pgbouncer/variables.tf`

```hcl
variable "env_name" {
  description = "Environment name (e.g., prod). Used for resource naming and tagging."
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC where PgBouncer will be deployed."
  type        = string
}

variable "subnet_id" {
  description = "ID of the private subnet where PgBouncer will be placed."
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type for the PgBouncer instance."
  type        = string
  default     = "t4g.nano"
}

variable "rds_endpoint" {
  description = "RDS instance endpoint hostname (e.g., main.xxx.us-east-1.rds.amazonaws.com)."
  type        = string
}

variable "rds_security_group_id" {
  description = "Security group ID of the RDS instance. Used to add an ingress rule allowing PgBouncer."
  type        = string
}

variable "lambda_security_group_id" {
  description = "Security group ID of the Lambda function. Used to allow ingress from Lambda to PgBouncer."
  type        = string
}

variable "bastion_security_group_id" {
  description = "Security group ID of the bastion. Used to allow ingress from bastion to PgBouncer. Set to empty string to skip."
  type        = string
  default     = ""
}

variable "database_name" {
  description = "Name of the PostgreSQL database to pool."
  type        = string
  default     = "tokenoverflow"
}

variable "database_user" {
  description = "PostgreSQL user for PgBouncer authentication."
  type        = string
  default     = "tokenoverflow"
}

variable "database_password_ssm_name" {
  description = "SSM Parameter Store name containing the database password."
  type        = string
}

variable "eni_private_ip" {
  description = "Fixed private IP address for the dedicated ENI. Must be within the subnet CIDR."
  type        = string
}
```

#### `infra/terraform/modules/pgbouncer/security_groups.tf`

```hcl
resource "aws_security_group" "pgbouncer" {
  name        = "pgbouncer"
  description = "Security group for PgBouncer connection pooler"
  vpc_id      = var.vpc_id

  tags = {
    Name        = "pgbouncer"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

# Ingress: Allow PgBouncer port from Lambda
resource "aws_vpc_security_group_ingress_rule" "pgbouncer_from_lambda" {
  security_group_id            = aws_security_group.pgbouncer.id
  description                  = "Allow PgBouncer from Lambda"
  from_port                    = 6432
  to_port                      = 6432
  ip_protocol                  = "tcp"
  referenced_security_group_id = var.lambda_security_group_id
}

# Ingress: Allow PgBouncer port from bastion (for DB admin via bastion -> PgBouncer -> RDS)
resource "aws_vpc_security_group_ingress_rule" "pgbouncer_from_bastion" {
  count = var.bastion_security_group_id != "" ? 1 : 0

  security_group_id            = aws_security_group.pgbouncer.id
  description                  = "Allow PgBouncer from bastion"
  from_port                    = 6432
  to_port                      = 6432
  ip_protocol                  = "tcp"
  referenced_security_group_id = var.bastion_security_group_id
}

# Egress: Allow all outbound traffic (SSM agent, package repos, RDS)
resource "aws_vpc_security_group_egress_rule" "all_outbound" {
  security_group_id = aws_security_group.pgbouncer.id
  description       = "Allow all outbound traffic (SSM agent, package repos, RDS)"
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}

# RDS SG: Allow ingress from PgBouncer (cross-module rule, same pattern as Lambda module)
resource "aws_vpc_security_group_ingress_rule" "rds_from_pgbouncer" {
  security_group_id            = var.rds_security_group_id
  description                  = "Allow PostgreSQL from PgBouncer"
  from_port                    = 5432
  to_port                      = 5432
  ip_protocol                  = "tcp"
  referenced_security_group_id = aws_security_group.pgbouncer.id
}
```

#### `infra/terraform/modules/pgbouncer/main.tf`

```hcl
data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

data "aws_ssm_parameter" "db_password" {
  name = var.database_password_ssm_name
}

# Dedicated ENI with a fixed private IP. This ENI persists across ASG instance
# replacements, giving Lambda a stable target address. AL2023's
# amazon-ec2-net-utils automatically configures policy routing when attached.
resource "aws_network_interface" "pgbouncer" {
  subnet_id       = var.subnet_id
  private_ips     = [var.eni_private_ip]
  security_groups = [aws_security_group.pgbouncer.id]

  tags = {
    Name        = "pgbouncer"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_launch_template" "pgbouncer" {
  name          = "pgbouncer"
  image_id      = data.aws_ssm_parameter.al2023_arm64.value
  instance_type = var.instance_type

  iam_instance_profile {
    arn = aws_iam_instance_profile.pgbouncer.arn
  }

  # Primary network interface (eth0) -- ASG-assigned IP, used for outbound
  # AWS service calls (SSM, package repos). The dedicated ENI (eth1) is
  # attached by user-data after boot. AL2023 handles routing automatically.
  network_interfaces {
    associate_public_ip_address = false
    security_groups             = [aws_security_group.pgbouncer.id]
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

  user_data = base64encode(templatefile("${path.module}/user_data.sh.tftpl", {
    eni_id            = aws_network_interface.pgbouncer.id
    rds_endpoint      = var.rds_endpoint
    database_name     = var.database_name
    database_user     = var.database_user
    database_password = data.aws_ssm_parameter.db_password.value
  }))

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name        = "pgbouncer"
      Environment = var.env_name
      ManagedBy   = "opentofu"
    }
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_autoscaling_group" "pgbouncer" {
  name                = "pgbouncer"
  min_size            = 1
  max_size            = 1
  desired_capacity    = 1
  vpc_zone_identifier = [var.subnet_id]

  launch_template {
    id      = aws_launch_template.pgbouncer.id
    version = "$Latest"
  }

  tag {
    key                 = "Name"
    value               = "pgbouncer"
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

# IAM role for the PgBouncer instance. Needs:
# 1. SSM Session Manager access (for debugging, no SSH)
# 2. EC2 ENI attach/detach permissions (for user-data to attach the dedicated ENI)
resource "aws_iam_role" "pgbouncer" {
  name = "pgbouncer"

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
  role       = aws_iam_role.pgbouncer.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_role_policy" "eni_manage" {
  name = "pgbouncer-eni-manage"
  role = aws_iam_role.pgbouncer.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ec2:AttachNetworkInterface",
          "ec2:DetachNetworkInterface"
        ]
        Resource = "*"
        Condition = {
          StringEquals = {
            "ec2:ResourceTag/Name" = "pgbouncer"
          }
        }
      },
      {
        Effect   = "Allow"
        Action   = "ec2:DescribeNetworkInterfaces"
        Resource = "*"
      }
    ]
  })
}

resource "aws_iam_instance_profile" "pgbouncer" {
  name = "pgbouncer"
  role = aws_iam_role.pgbouncer.name
}
```

#### `infra/terraform/modules/pgbouncer/outputs.tf`

```hcl
output "private_ip" {
  description = "The fixed private IP address of the PgBouncer ENI. Use this as the database host."
  value       = var.eni_private_ip
}

output "security_group_id" {
  description = "The ID of the PgBouncer security group."
  value       = aws_security_group.pgbouncer.id
}

output "eni_id" {
  description = "The ID of the dedicated ENI used by PgBouncer."
  value       = aws_network_interface.pgbouncer.id
}

output "autoscaling_group_name" {
  description = "The name of the PgBouncer autoscaling group."
  value       = aws_autoscaling_group.pgbouncer.name
}
```

#### `infra/terraform/modules/pgbouncer/user_data.sh.tftpl`

```bash
#!/bin/bash
set -euo pipefail

# =============================================================================
# PgBouncer EC2 User Data
#
# This script runs on every instance launch (including ASG replacements).
# It performs two tasks:
#   1. Attach the dedicated ENI (AL2023 auto-configures policy routing)
#   2. Install, configure, and start PgBouncer
# =============================================================================

exec > >(tee /var/log/user-data.log) 2>&1
echo "=== PgBouncer user-data starting at $(date -u) ==="

# ---------------------------------------------------------------------------
# Variables from Terraform templatefile
# ---------------------------------------------------------------------------
ENI_ID="${eni_id}"
RDS_ENDPOINT="${rds_endpoint}"
DB_NAME="${database_name}"
DB_USER="${database_user}"
DB_PASSWORD="${database_password}"

# ---------------------------------------------------------------------------
# Step 1: Attach the dedicated ENI
# ---------------------------------------------------------------------------
echo "=== Step 1: Attaching ENI $ENI_ID ==="

# Get instance ID and region from IMDS (IMDSv2)
TOKEN=$(curl -sX PUT "http://169.254.169.254/latest/api/token" \
  -H "X-aws-ec2-metadata-token-ttl-seconds: 300")
INSTANCE_ID=$(curl -s "http://169.254.169.254/latest/meta-data/instance-id" \
  -H "X-aws-ec2-metadata-token: $TOKEN")
REGION=$(curl -s "http://169.254.169.254/latest/meta-data/placement/region" \
  -H "X-aws-ec2-metadata-token: $TOKEN")

echo "Instance: $INSTANCE_ID, Region: $REGION"

# Force-detach if still attached to a previous instance (ASG replacement race)
ATTACHMENT_ID=$(aws ec2 describe-network-interfaces \
  --network-interface-ids "$ENI_ID" \
  --region "$REGION" \
  --query 'NetworkInterfaces[0].Attachment.AttachmentId' \
  --output text 2>/dev/null || echo "None")

if [ "$ATTACHMENT_ID" != "None" ] && [ -n "$ATTACHMENT_ID" ]; then
  echo "ENI attached elsewhere ($ATTACHMENT_ID), force-detaching..."
  aws ec2 detach-network-interface \
    --attachment-id "$ATTACHMENT_ID" \
    --force \
    --region "$REGION" || true
  for i in $(seq 1 30); do
    STATUS=$(aws ec2 describe-network-interfaces \
      --network-interface-ids "$ENI_ID" \
      --region "$REGION" \
      --query 'NetworkInterfaces[0].Status' \
      --output text)
    [ "$STATUS" = "available" ] && break
    sleep 1
  done
fi

aws ec2 attach-network-interface \
  --network-interface-id "$ENI_ID" \
  --instance-id "$INSTANCE_ID" \
  --device-index 1 \
  --region "$REGION"

# AL2023's amazon-ec2-net-utils detects the new ENI via udev, assigns its IP,
# and configures policy routing automatically via systemd-networkd. No manual
# ip rule/route commands needed.
echo "ENI attached. AL2023 will auto-configure IP and policy routing."

# ---------------------------------------------------------------------------
# Step 2: Install PgBouncer
# ---------------------------------------------------------------------------
echo "=== Step 2: Installing PgBouncer ==="

# Install the PostgreSQL PGDG repository for AL2023 (aarch64).
# AL2023 lacks /etc/redhat-release which the RPM declares as a dependency,
# but the repo and packages are fully EL-9 compatible. Use --nodeps to skip.
rpm -ivh --nodeps https://download.postgresql.org/pub/repos/yum/reporpms/EL-9-aarch64/pgdg-redhat-repo-latest.noarch.rpm

# AL2023 sets $releasever to its own version (e.g. 2023.10.xxx) instead of "9",
# causing PGDG repo URLs to 404. Pin all pgdg repos to releasever=9.
for repo in /etc/yum.repos.d/pgdg-redhat-all.repo; do
  sed -i '/^\[pgdg/,/^$/{ /^$/i\module_hotfixes=1
  }' "$repo"
  sed -i 's|\$releasever|9|g' "$repo"
done

# Disable the default PostgreSQL module to avoid conflicts
dnf -qy module disable postgresql 2>/dev/null || true

# Install PgBouncer
dnf install -y pgbouncer

# ---------------------------------------------------------------------------
# Step 3: Configure PgBouncer
# ---------------------------------------------------------------------------
echo "=== Step 3: Configuring PgBouncer ==="

cat > /etc/pgbouncer/pgbouncer.ini << 'PGBOUNCER_EOF'
[databases]
PLACEHOLDER_DB = host=PLACEHOLDER_RDS port=5432 dbname=PLACEHOLDER_DBNAME

[pgbouncer]
listen_addr = 0.0.0.0
listen_port = 6432
auth_type = scram-sha-256
auth_file = /etc/pgbouncer/userlist.txt
pool_mode = transaction
max_prepared_statements = 500
default_pool_size = 5
max_client_conn = 1000
min_pool_size = 2
server_connect_timeout = 15
server_idle_timeout = 600
query_wait_timeout = 120
server_tls_sslmode = require
log_connections = 1
log_disconnections = 1
log_pooler_errors = 1
admin_users = pgbouncer
PGBOUNCER_EOF

sed -i "s|PLACEHOLDER_DBNAME|$DB_NAME|g" /etc/pgbouncer/pgbouncer.ini
sed -i "s|PLACEHOLDER_RDS|$RDS_ENDPOINT|g" /etc/pgbouncer/pgbouncer.ini
sed -i "s|PLACEHOLDER_DB|$DB_NAME|g" /etc/pgbouncer/pgbouncer.ini

cat > /etc/pgbouncer/userlist.txt << USERLIST_EOF
"$DB_USER" "$DB_PASSWORD"
USERLIST_EOF

chmod 600 /etc/pgbouncer/userlist.txt
chown pgbouncer:pgbouncer /etc/pgbouncer/userlist.txt
chown pgbouncer:pgbouncer /etc/pgbouncer/pgbouncer.ini

# ---------------------------------------------------------------------------
# Step 4: Start PgBouncer
# ---------------------------------------------------------------------------
echo "=== Step 4: Starting PgBouncer ==="

systemctl enable pgbouncer
systemctl start pgbouncer

sleep 2
if systemctl is-active --quiet pgbouncer; then
  echo "PgBouncer is running successfully"
  systemctl status pgbouncer --no-pager
else
  echo "ERROR: PgBouncer failed to start"
  journalctl -u pgbouncer --no-pager -n 50
  exit 1
fi

echo "=== PgBouncer user-data completed at $(date -u) ==="
```

#### `infra/terraform/live/prod/pgbouncer/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/pgbouncer"
}

dependency "vpc" {
  config_path = "../vpc"
}

dependency "rds" {
  config_path = "../rds"
}

dependency "lambda" {
  config_path = "../lambda"
}

dependency "bastion" {
  config_path = "../bastion"
}

inputs = {
  env_name  = "prod"
  vpc_id    = dependency.vpc.outputs.vpc_id
  subnet_id = dependency.vpc.outputs.private_subnet_ids[0]

  rds_endpoint          = dependency.rds.outputs.db_instance_address
  rds_security_group_id = dependency.rds.outputs.security_group_id

  lambda_security_group_id  = dependency.lambda.outputs.security_group_id
  bastion_security_group_id = dependency.bastion.outputs.security_group_id

  database_name              = "tokenoverflow"
  database_user              = "tokenoverflow"
  database_password_ssm_name = "/tokenoverflow/prod/database-password"

  # Fixed private IP for the dedicated ENI. Must be within 10.0.10.0/24
  # and not conflict with other resources. Using .200 to avoid DHCP range.
  eni_private_ip = "10.0.10.200"
}
```

### Modified Files

#### `apps/api/config/production.toml` (point to PgBouncer)

Update the `[database]` section to point to PgBouncer:

```toml
# Production environment

[api]
host = "0.0.0.0"
port = 8080
base_url = "https://api.tokenoverflow.io"
request_timeout_secs = 30

[database]
host = "10.0.10.200"
port = 6432
user = "tokenoverflow"
name = "tokenoverflow"

[embedding]
base_url = "https://api.voyageai.com/v1"
model = "voyage-code-3"
output_dimension = 256

[mcp]
base_url = "https://api.tokenoverflow.io/mcp"

[logging]
level = "warn"
```

### Files NOT Modified

| File | Reason |
|------|--------|
| `infra/terraform/modules/lambda/variables.tf` | No changes needed. The `production.toml` config handles the PgBouncer host/port. |
| `infra/terraform/modules/lambda/function.tf` | No changes needed. No Lambda environment variable overrides required. |
| `infra/terraform/live/prod/lambda/terragrunt.hcl` | No changes needed. No new inputs required. |
| `infra/terraform/modules/rds/main.tf` | No changes needed. PgBouncer SG ingress rule is managed by the PgBouncer module. |
| `infra/terraform/modules/rds/security_groups.tf` | Cross-module SG rule is in the PgBouncer module (same pattern as Lambda). |
| `infra/terraform/modules/rds/variables.tf` | No new variables needed for PgBouncer. |
| `infra/terraform/modules/bastion/main.tf` | Unrelated to PgBouncer. Bastion SG ID is read via Terragrunt dependency. |
| `infra/terraform/live/prod/rds/terragrunt.hcl` | No new inputs needed. PgBouncer module manages its own RDS SG rule. |
| `infra/terraform/live/prod/bastion/terragrunt.hcl` | Unrelated to PgBouncer. Bastion SG ID is exported by the bastion module. |
| `infra/terraform/live/root.hcl` | Provider version 6.33.0 is sufficient. |
| `docker-compose.yml` | Local PgBouncer already works. No changes needed. |
| `apps/api/config/local.toml` | Local dev uses docker-compose PgBouncer on localhost:6432 (already configured). |

### Lambda Module Outputs (Existing Required Output)

The PgBouncer Terragrunt unit depends on
`dependency.lambda.outputs.security_group_id`. This output already exists in
`infra/terraform/modules/lambda/outputs.tf` (line 16):

```hcl
output "security_group_id" {
  description = "Lambda security group ID"
  value       = aws_security_group.lambda.id
}
```

No changes needed. This was verified during research.

### Bastion Module Outputs (Existing Required Output)

The PgBouncer Terragrunt unit depends on
`dependency.bastion.outputs.security_group_id`. This output already exists in
`infra/terraform/modules/bastion/outputs.tf` (line 1-3):

```hcl
output "security_group_id" {
  description = "The ID of the bastion security group."
  value       = aws_security_group.bastion.id
}
```

No changes needed.

---

## Logic

This section defines the exact sequence of operations to implement the PgBouncer
infrastructure. The deployment is split into phases to minimize risk and allow
verification at each step.

### Phase 1: Create the PgBouncer module

**Step 1.1:** Create the module directory:

```bash
mkdir -p infra/terraform/modules/pgbouncer
```

**Step 1.2:** Create `infra/terraform/modules/pgbouncer/variables.tf` with the
content defined in the Interfaces section.

**Step 1.3:** Create `infra/terraform/modules/pgbouncer/security_groups.tf`
with the content defined in the Interfaces section.

**Step 1.4:** Create `infra/terraform/modules/pgbouncer/main.tf` with the
content defined in the Interfaces section.

**Step 1.5:** Create `infra/terraform/modules/pgbouncer/outputs.tf` with the
content defined in the Interfaces section.

**Step 1.6:** Create `infra/terraform/modules/pgbouncer/user_data.sh.tftpl`
with the content defined in the Interfaces section.

**Step 1.7:** Validate with TFLint:

```bash
cd infra/terraform/modules/pgbouncer
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

### Phase 2: Create the prod Terragrunt unit for PgBouncer

**Step 2.1:** Create the unit directory:

```bash
mkdir -p infra/terraform/live/prod/pgbouncer
```

**Step 2.2:** Create `infra/terraform/live/prod/pgbouncer/terragrunt.hcl` with
the content defined in the Interfaces section.

### Phase 3: Deploy PgBouncer (Lambda still points to RDS)

At this point, PgBouncer is deployed but Lambda continues to connect directly
to RDS. This allows verifying PgBouncer independently before cutting over.

**Step 3.1:** Log in to the prod AWS account:

```bash
aws sso login --profile tokenoverflow-prod-admin
```

**Step 3.2:** Initialize and plan:

```bash
cd infra/terraform/live/prod/pgbouncer
terragrunt init
terragrunt plan
```

Expected resources to be created:

- 1 `aws_network_interface` with `private_ips = ["10.0.10.200"]`
- 1 `aws_security_group` named `pgbouncer`
- 1 `aws_vpc_security_group_ingress_rule` (PgBouncer port 6432 from Lambda SG)
- 1 `aws_vpc_security_group_ingress_rule` (PgBouncer port 6432 from bastion SG)
- 1 `aws_vpc_security_group_egress_rule` (all outbound)
- 1 `aws_vpc_security_group_ingress_rule` on the RDS SG (from PgBouncer)
- 1 `aws_launch_template` with `instance_type = "t4g.nano"` and user-data
- 1 `aws_autoscaling_group` with `min_size = 1`, `max_size = 1`
- 1 `aws_iam_role` + 1 `aws_iam_instance_profile` + 2 IAM policies (SSM + ENI)

The plan should NOT show changes to VPC, RDS, Lambda, or any other resources.

**Step 3.3:** Apply:

```bash
terragrunt apply
```

**Step 3.4:** Verify the instance is running and PgBouncer is healthy:

```bash
# Check ASG has a running instance
aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names "pgbouncer" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'AutoScalingGroups[0].Instances[0].{Id:InstanceId,State:LifecycleState,Health:HealthStatus}'

# Connect via SSM and check PgBouncer is running
INSTANCE_ID=$(aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names "pgbouncer" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'AutoScalingGroups[0].Instances[0].InstanceId' \
  --output text)

aws ssm start-session \
  --target "$INSTANCE_ID" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1

# Inside the session:
# systemctl status pgbouncer
# ss -tlnp | grep 6432
# cat /var/log/user-data.log
# ip addr show  (verify ENI is attached with fixed IP)
```

**Step 3.5:** Commit:

```bash
git add infra/terraform/modules/pgbouncer/
git add infra/terraform/live/prod/pgbouncer/
git commit -m "infra: add pgbouncer ec2 for lambda connection pooling"
```

### Phase 4: Update production config and deploy API

**Step 4.1:** Update `apps/api/config/production.toml` to change the
`[database]` section to point to PgBouncer (`host = "10.0.10.200"`,
`port = 6432`).

**Step 4.2:** Deploy the API with the updated config (via the existing Lambda
deployment pipeline). No Terraform/Terragrunt changes needed for Lambda.

**Step 4.3:** Commit:

```bash
git add apps/api/config/production.toml
git commit -m "config: route production database connections through pgbouncer"
```

### Phase 5: Verify end-to-end

**Step 5.1:** Test the API health endpoint:

```bash
curl -s https://api.tokenoverflow.io/health | jq .
```

**Step 5.2:** Test a database-dependent endpoint (e.g., search):

```bash
curl -s "https://api.tokenoverflow.io/api/v1/questions?page=1&per_page=5" | jq .
```

**Step 5.3:** Connect to PgBouncer via SSM and check connection stats:

```bash
# Inside SSM session on the PgBouncer instance:
psql -h 127.0.0.1 -p 6432 -U pgbouncer pgbouncer -c "SHOW POOLS;"
psql -h 127.0.0.1 -p 6432 -U pgbouncer pgbouncer -c "SHOW STATS;"
```

**Step 5.4:** Test bastion -> PgBouncer -> RDS route:

```bash
# From the bastion (via SSM):
psql -h 10.0.10.200 -p 6432 -U tokenoverflow tokenoverflow -c "SELECT 1;"
```

---

## Edge Cases & Constraints

### 1. ASG instance replacement (~2-3 min downtime)

**Risk:** When the ASG replaces the PgBouncer instance (due to health check
failure, AZ impairment, or manual termination), there is a 2-3 minute window
where Lambda cannot reach PgBouncer. All database queries during this window
will fail.

**Mitigation:** This is an accepted trade-off for an MVP at $3.71/month. Lambda
invocations will receive connection errors and return 500s. API Gateway can
return a cached response or a retry-after header. The recovery is automatic
with no manual intervention. If higher availability is needed, consider RDS
Proxy or a second PgBouncer instance with a Network Load Balancer.

### 2. ENI attachment race during ASG replacement

**Risk:** When the ASG terminates the old instance and launches a new one, the
old instance may not have fully released the ENI before the new instance's
user-data tries to attach it.

**Mitigation:** The user-data script handles this:
1. Checks if the ENI is currently attached
2. Force-detaches it if so (`--force` flag)
3. Polls for up to 30 seconds until the ENI becomes available
4. Then attaches it to the new instance

Secondary ENIs are automatically detached when an instance terminates. Since the
ASG takes 1-2 minutes to launch the replacement, the ENI is typically already
available. The force-detach is a safety net for edge cases.

### 3. AL2023 auto-configuration of secondary ENI

**Risk:** The design relies on AL2023's `amazon-ec2-net-utils` to automatically
configure IP assignment and policy routing for the secondary ENI. If this
mechanism fails, traffic to the ENI IP would not be routed correctly.

**Mitigation:** `amazon-ec2-net-utils` 2.x is pre-installed and battle-tested
on AL2023. It uses udev rules for hotplug detection and generates
systemd-networkd configuration. The same mechanism powers multi-ENI workloads
in EKS on AL2023. Keep AL2023 updated to get the latest ec2-net-utils fixes.
The user-data log will show if the ENI attachment succeeded, and `ip addr show`
on the instance confirms the IP is assigned.

### 4. Credential rotation

**Risk:** The database password is baked into the user-data at apply time (read
from SSM). If the password is rotated in SSM, the running PgBouncer instance
still uses the old password.

**Mitigation:** To rotate credentials:
1. Update the password in SSM Parameter Store
2. Update the password in RDS
3. Terminate the PgBouncer instance (`aws autoscaling terminate-instance-in-auto-scaling-group`)
4. The ASG launches a new instance that reads the new password from SSM

Alternatively, run `terragrunt apply` on the PgBouncer unit (the user-data
template will regenerate with the new password) and then terminate the running
instance to force replacement.

### 5. PgBouncer not in AL2023 default repos

**Risk:** Amazon Linux 2023 does not include PgBouncer in its default
repositories. The PGDG (PostgreSQL Global Development Group) repository must
be installed first.

**Mitigation:** The user-data script installs the PGDG repo for EL-9 aarch64
(`pgdg-redhat-repo-latest.noarch.rpm`) before installing PgBouncer. AL2023 is
based on Fedora/RHEL and is compatible with the EL-9 PGDG packages. The PGDG
repo is the official PostgreSQL package source and is actively maintained.

### 6. RDS max_connections headroom

**Risk:** The db.t4g.micro RDS instance supports approximately 112 connections
(`max_connections` based on memory formula). PgBouncer's `default_pool_size=5`
plus `min_pool_size=2` means up to 5 server connections per database.

**Mitigation:** With a single database (`tokenoverflow`), PgBouncer will use
at most 5 server connections, leaving 107 connections for the bastion, migrations,
and other admin tasks. This provides ample headroom. At ~10ms per transaction,
5 connections handle ~500 txn/s -- more than enough for MVP. If more databases
are added, each gets its own pool, and the total server connections must be
monitored.

### 7. Cross-AZ traffic charges

**Risk:** Lambda invocations in us-east-1b connecting to PgBouncer in
us-east-1a incur $0.01/GB cross-AZ data transfer charges in each direction.

**Mitigation:** Database query payloads are small (typically a few KB per
request). At MVP traffic levels, cross-AZ costs are negligible (e.g., 10 GB
cross-AZ traffic costs $0.20/month). This is the same trade-off accepted for
fck-nat. If costs become material, deploy a second PgBouncer instance in
us-east-1b.

### 8. SCRAM-SHA-256 authentication with plaintext password in userlist.txt

**Risk:** The `userlist.txt` file contains the database password in plaintext
on the EC2 instance's filesystem.

**Mitigation:** The file is chmod 600 and owned by the `pgbouncer` user. The
instance has no SSH access (SSM only), and SSM access is restricted to admin
IAM roles. The password is already stored in SSM Parameter Store (which the
instance reads during user-data). The EC2 EBS volume is encrypted. This is an
acceptable security posture for an MVP.

### 9. user-data failure leaves instance without PgBouncer

**Risk:** If user-data fails (e.g., PGDG repo is down, ENI attachment fails),
the instance will be running but PgBouncer will not be available. The ASG EC2
health check only verifies the instance is running, not that PgBouncer is
healthy.

**Mitigation:** The user-data script uses `set -euo pipefail` and logs all
output to `/var/log/user-data.log`. If PgBouncer fails to start, the script
exits with error code 1. While this does not automatically trigger ASG
replacement (EC2 health checks only check instance status), the operator can
check `/var/log/user-data.log` via SSM. For automated detection, an ELB health
check on port 6432 could be added in the future.

### 10. Hardcoded PgBouncer IP in Lambda config

**Risk:** The PgBouncer ENI IP (`10.0.10.200`) is hardcoded in the Lambda
Terragrunt inputs rather than read via a `dependency` block. If someone changes
the IP in the PgBouncer config without updating Lambda, the API will break.

**Mitigation:** This is an intentional trade-off to avoid a circular Terragrunt
dependency (PgBouncer -> Lambda -> PgBouncer). The IP is a static value assigned
to a Terraform-managed ENI that persists independently of instances. It should
rarely (if ever) change. A comment in the Lambda Terragrunt config documents
this coupling and references the PgBouncer module. If the IP must change, both
files must be updated together in the same commit.

---

## Test Plan

### Verification Checklist

Infrastructure changes are verified through plan output inspection and
post-apply validation. The application-level tests (existing e2e tests) verify
database connectivity through PgBouncer since the local docker-compose already
uses PgBouncer.

#### 1. TFLint passes on the PgBouncer module

```bash
cd infra/terraform/modules/pgbouncer
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

**Success:** No errors or warnings.

#### 2. PgBouncer plan creates expected resources

```bash
cd infra/terraform/live/prod/pgbouncer
terragrunt plan
```

**Success:** Plan shows creation of:

- 1 `aws_network_interface` with `private_ips = ["10.0.10.200"]`
- 1 `aws_security_group` named `pgbouncer`
- 1 `aws_vpc_security_group_ingress_rule` (PgBouncer port 6432 from Lambda SG)
- 1 `aws_vpc_security_group_ingress_rule` (PgBouncer port 6432 from bastion SG)
- 1 `aws_vpc_security_group_egress_rule` (all outbound)
- 1 `aws_vpc_security_group_ingress_rule` on the RDS SG
  (port 5432 from PgBouncer SG)
- 1 `aws_launch_template` with `instance_type = "t4g.nano"` and user-data
- 1 `aws_autoscaling_group` with `min_size = 1`, `max_size = 1`
- 1 `aws_iam_role` named `pgbouncer`
- 1 `aws_iam_instance_profile` named `pgbouncer`
- 1 `aws_iam_role_policy_attachment` (SSM)
- 1 `aws_iam_role_policy` (ENI manage)

Plan should NOT show changes to VPC, RDS, Lambda, or other existing resources.

#### 3. Lambda infrastructure is unaffected

```bash
cd infra/terraform/live/prod/lambda
terragrunt plan
```

**Success:** Plan shows "No changes." The Lambda module is not modified. The
database connection routing is handled entirely by `production.toml`.

#### 4. Existing infrastructure is unaffected

```bash
source scripts/src/includes.sh
tg plan prod
```

**Success:** VPC, RDS, Bastion, NAT units all show "No changes."

#### 5. No circular dependency in Terragrunt DAG

```bash
cd infra/terraform/live/prod
terragrunt graph-dependencies
```

**Success:** The dependency graph is a valid DAG with no cycles. PgBouncer
depends on Lambda and Bastion (one direction only). Lambda does NOT depend on
PgBouncer.

#### 6. Post-apply: ASG has a running instance

```bash
aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names "pgbouncer" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'AutoScalingGroups[0].{DesiredCapacity:DesiredCapacity,RunningInstances:Instances[?LifecycleState==`InService`]|length(@)}'
```

**Success:** Returns `{ "DesiredCapacity": 1, "RunningInstances": 1 }`.

#### 7. Post-apply: PgBouncer is listening on port 6432

```bash
INSTANCE_ID=$(aws autoscaling describe-auto-scaling-groups \
  --auto-scaling-group-names "pgbouncer" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'AutoScalingGroups[0].Instances[0].InstanceId' \
  --output text)

aws ssm start-session \
  --target "$INSTANCE_ID" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1
```

Inside the SSM session:

```bash
systemctl status pgbouncer
ss -tlnp | grep 6432
cat /var/log/user-data.log
```

**Success:** PgBouncer systemd service is active. Port 6432 is listening.
User-data log shows no errors.

#### 8. Post-apply: ENI is attached and AL2023 configured routing

Inside the SSM session:

```bash
ip addr show
ip rule show
```

**Success:** A secondary interface shows IP 10.0.10.200. Policy routing rules
exist for the secondary interface (created automatically by
`amazon-ec2-net-utils`).

#### 9. Post-apply: PgBouncer can connect to RDS

Inside the SSM session:

```bash
psql -h 127.0.0.1 -p 6432 -U pgbouncer pgbouncer -c "SHOW POOLS;"
```

**Success:** Shows the `tokenoverflow` database pool with `sv_active`,
`sv_idle`, and `sv_used` columns. `sv_idle` should be >= `min_pool_size` (2).

#### 10. Post-apply: Bastion can connect via PgBouncer

From the bastion (via SSM):

```bash
psql -h 10.0.10.200 -p 6432 -U tokenoverflow tokenoverflow -c "SELECT 1;"
```

**Success:** Returns `1`. This verifies the bastion -> PgBouncer -> RDS route
works end-to-end.

#### 11. Post-apply: API health check passes

After Lambda is updated to point to PgBouncer:

```bash
curl -s https://api.tokenoverflow.io/health | jq .
```

**Success:** Returns a healthy response with database connectivity confirmed.

#### 12. Local e2e tests still pass

The local docker-compose already routes through PgBouncer. Verify existing tests
still pass:

```bash
docker compose up -d --build api
cargo test -p tokenoverflow --test e2e
```

**Success:** All e2e tests pass. This confirms the application works correctly
through PgBouncer with transaction pooling and prepared statements.

---

## Documentation Changes

### Files to Update

| File | Change |
|------|--------|
| `infra/terraform/README.md` | Add PgBouncer section with architecture, cost, deploy, and troubleshoot commands |

### Content to Add to `infra/terraform/README.md`

Add the following after the existing NAT section:

```markdown
## PgBouncer (Connection Pooler)

The PgBouncer module (`modules/pgbouncer/`) deploys a PgBouncer connection
pooler on a t4g.nano EC2 instance, sitting between Lambda and RDS to prevent
connection exhaustion.

| Setting | Value |
|---------|-------|
| Instance type | t4g.nano ($3.07/month + $0.64 EBS = $3.71) |
| Pool mode | Transaction |
| Listen port | 6432 |
| Default pool size | 5 server connections |
| Max client connections | 1000 |
| Max prepared statements | 500 |
| HA mode | ASG min=max=1 (auto-replace on failure) |
| Placement | First private subnet (us-east-1a, 10.0.10.0/24) |
| Static IP | Dedicated ENI at 10.0.10.200 |
| Access | SSM Session Manager (no SSH) |

### Deploy

    $ cd infra/terraform/live/prod/pgbouncer
    $ terragrunt init
    $ terragrunt plan
    $ terragrunt apply

### Troubleshoot

Connect via SSM Session Manager:

    $ INSTANCE_ID=$(aws autoscaling describe-auto-scaling-groups \
        --auto-scaling-group-names "pgbouncer" \
        --query 'AutoScalingGroups[0].Instances[0].InstanceId' \
        --output text)
    $ aws ssm start-session --target "$INSTANCE_ID"

Inside the session:

    $ systemctl status pgbouncer
    $ cat /var/log/user-data.log
    $ psql -h 127.0.0.1 -p 6432 -U pgbouncer pgbouncer -c "SHOW POOLS;"

### Credential Rotation

1. Update password in SSM: `/tokenoverflow/prod/database-password`
2. Update password in RDS
3. Run `terragrunt apply` on the PgBouncer unit
4. Terminate the running instance to force ASG replacement

### Important: Hardcoded IP

The PgBouncer ENI IP (`10.0.10.200`) is hardcoded in
`live/prod/lambda/terragrunt.hcl` (not via Terragrunt dependency) to avoid a
circular dependency. If the IP changes, update both the PgBouncer and Lambda
Terragrunt configs.
```

### Files NOT Updated

Historical design documents are not updated. They are a snapshot of the codebase
at the time they were written.

---

## Development Environment Changes

### Brewfile

No changes needed. `tofuenv`, `terragrunt`, and `tflint` are already installed.

### Environment Variables

No new environment variables are introduced for local development. The existing
`docker-compose.yml` already runs PgBouncer with the same configuration that
this design deploys to production.

### Setup Flow

No changes. The `source scripts/src/includes.sh && setup` command continues to
work. No new tools or dependencies are required.

### Local Development Parity

The local `docker-compose.yml` PgBouncer configuration matches the production
deployment:

| Setting | Local (docker-compose) | Production (EC2) |
|---------|----------------------|------------------|
| Pool mode | transaction | transaction |
| Max prepared statements | 500 | 500 |
| Default pool size | 20 | 5 |
| Max client connections | 100 | 1000 |
| Listen port | 6432 | 6432 |
| Auth type | scram-sha-256 | scram-sha-256 |
| Server TLS | N/A (local) | require |

---

## Tasks

### Task 1: Create the PgBouncer Terraform module

**What:** Create `infra/terraform/modules/pgbouncer/` with all five files:
`variables.tf`, `security_groups.tf`, `main.tf`, `outputs.tf`, and
`user_data.sh.tftpl`.

**Steps:**

1. Create the module directory:

   ```bash
   mkdir -p infra/terraform/modules/pgbouncer
   ```

2. Create `variables.tf` with the content from the Interfaces section
3. Create `security_groups.tf` with the content from the Interfaces section
4. Create `main.tf` with the content from the Interfaces section
5. Create `outputs.tf` with the content from the Interfaces section
6. Create `user_data.sh.tftpl` with the content from the Interfaces section
7. Run TFLint:

   ```bash
   cd infra/terraform/modules/pgbouncer
   tflint --config="$(pwd)/../../.tflint.hcl" --init
   tflint --config="$(pwd)/../../.tflint.hcl"
   ```

**Success:** TFLint passes with no errors. The five files follow the bastion
module pattern. Security group rules use `aws_vpc_security_group_ingress_rule`
and `aws_vpc_security_group_egress_rule` (not inline rules). IAM role has SSM
and ENI attachment policies. User-data script attaches the ENI (AL2023 handles
routing) and installs PgBouncer via PGDG repo.

### Task 2: Create the prod Terragrunt unit for PgBouncer

**What:** Create `infra/terraform/live/prod/pgbouncer/terragrunt.hcl` with
prod-specific inputs and dependencies on VPC, RDS, Lambda, and Bastion.

**Steps:**

1. Create the unit directory:

   ```bash
   mkdir -p infra/terraform/live/prod/pgbouncer
   ```

2. Create `terragrunt.hcl` with the content from the Interfaces section

**Success:** File exists and follows the same pattern as existing Terragrunt
units. Dependencies reference `../vpc`, `../rds`, `../lambda`, and `../bastion`.
The `eni_private_ip` is set to `10.0.10.200`.

### Task 3: Deploy PgBouncer to production

**What:** Initialize, plan, and apply the PgBouncer infrastructure. Lambda
continues to connect directly to RDS during this task.

**Steps:**

1. Log in: `aws sso login --profile tokenoverflow-prod-admin`
2. Initialize and plan:

   ```bash
   cd infra/terraform/live/prod/pgbouncer
   terragrunt init
   terragrunt plan
   ```

3. Review the plan against the expected resource list in Test Plan section 2
4. Apply: `terragrunt apply`
5. Run verification checks from Test Plan sections 6, 7, 8, 9, and 10
6. Commit:

   ```bash
   git add infra/terraform/modules/pgbouncer/
   git add infra/terraform/live/prod/pgbouncer/
   git commit -m "infra: add pgbouncer ec2 for lambda connection pooling"
   ```

**Success:** ASG has one InService instance. PgBouncer is listening on port
6432. ENI is attached with IP 10.0.10.200 (AL2023 auto-configured routing).
PgBouncer can connect to RDS (SHOW POOLS shows the pool). Bastion can connect
through PgBouncer. No changes to existing infrastructure.

### Task 4: Update production config to route through PgBouncer

**What:** Update `apps/api/config/production.toml` to point the database
connection to PgBouncer instead of RDS directly. No Lambda module or Terragrunt
changes needed — the TOML config is sufficient.

**Steps:**

1. Update `apps/api/config/production.toml` to change host to `10.0.10.200`
   and port to 6432 (see Interfaces section)
2. Deploy the API with the updated config via the existing deployment pipeline
3. Verify API health
4. Verify no circular dependency: `cd infra/terraform/live/prod && terragrunt graph-dependencies`
5. Commit:

   ```bash
   git add apps/api/config/production.toml
   git commit -m "config: route production database connections through pgbouncer"
   ```

**Success:** API health endpoint returns success. Database queries work through
PgBouncer. PgBouncer `SHOW POOLS` shows active connections. Terragrunt
dependency graph has no cycles.

### Task 5: Update documentation

**What:** Update `infra/terraform/README.md` with PgBouncer information.

**Steps:**

1. Add PgBouncer section to `infra/terraform/README.md` (see Documentation
   Changes section)
2. Commit:

   ```bash
   git add infra/terraform/README.md
   git commit -m "docs: add pgbouncer documentation to terraform README"
   ```

**Success:** README accurately describes the PgBouncer deployment, cost,
architecture, deploy instructions, troubleshooting commands, credential
rotation procedure, and the hardcoded IP caveat.
