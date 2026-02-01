# Design: fargate-api-deployment

## Architecture Overview

### Goal

Deploy the TokenOverflow API container to AWS Fargate on ECS, running on ARM
(Graviton) architecture with Spot capacity, connecting to the existing RDS
PostgreSQL database in the production VPC. The deployment must achieve absolute
minimum cost while maintaining production-grade health checking and
observability.

### Scope

This design covers:

- ECR repository for the API container image (Terraform-managed)
- ECS cluster creation
- ECS task definition for the API container
- ECS service with Fargate Spot capacity provider
- IAM roles (task execution role + task role)
- Security group for the Fargate task
- RDS security group ingress rule update (allow Fargate tasks to connect)
- CloudWatch log group with retention policy
- Container health check configuration
- Environment variable configuration (TOKENOVERFLOW_ENV, secrets)
- VPC-internal connectivity guarantee (Fargate to RDS stays inside the VPC)
- Cost analysis for compute, logging, metrics, tracing, and SSM

This design does NOT cover:

- Application Load Balancer (separate concern, deferred)
- Auto-scaling policies (single task for MVP)
- CI/CD pipeline for image builds and deployments
- Custom domain / Route 53 configuration
- VPC endpoints for ECR (relies on NAT instance for image pulls)
- Dev environment ECS deployment (deferred, but module supports it)

### Prerequisites

The following infrastructure must exist before deploying this module:

| Resource                 | Module        | Status   |
|--------------------------|---------------|----------|
| VPC with private subnets | `modules/vpc` | Deployed |
| RDS PostgreSQL instance  | `modules/rds` | Deployed |
| fck-nat instance         | `modules/nat` | Deployed |

The ECR repository is created by the ECS module itself (see Interfaces
section). No external prerequisite for ECR.

The fck-nat instance is critical because Fargate tasks in private subnets need
outbound internet access to pull container images from ECR (unless a VPC
endpoint is configured) and to call external APIs (Voyage AI for embeddings).

### Container Configuration

The API container has the following characteristics, confirmed from the
codebase:

| Setting            | Value                                    | Source                                                |
|--------------------|------------------------------------------|-------------------------------------------------------|
| Port               | 8080                                     | `apps/api/config/production.toml` (`api.port = 8080`) |
| Health endpoint    | `GET /health`                            | `apps/api/src/api/routes/configure.rs`                |
| Health response    | `{"status":"ok","database":"connected"}` | `apps/api/src/api/routes/health.rs`                   |
| Config env var     | `TOKENOVERFLOW_ENV=production`           | `apps/api/src/config.rs` (line 76)                    |
| Config overrides   | `TOKENOVERFLOW__SECTION__KEY` format     | `apps/api/src/config.rs` (line 86-89)                 |
| DB password secret | `TOKENOVERFLOW_DATABASE_PASSWORD`        | `apps/api/src/config.rs` (line 95)                    |
| Embedding API key  | `TOKENOVERFLOW_EMBEDDING_API_KEY`        | `apps/api/src/config.rs` (line 96)                    |
| Dockerfile         | Multi-stage, ARM64 platform pinned       | `apps/api/Dockerfile`                                 |
| Runtime image      | `debian:bookworm-slim` (ARM64)           | `apps/api/Dockerfile` (line 57)                       |
| Runs as            | `appuser` (UID 1001)                     | `apps/api/Dockerfile` (line 71)                       |

The Dockerfile already pins `--platform=linux/arm64` in both the build and
runtime stages and strips debug symbols from the binary. No Dockerfile changes
are required.

### Compute Configuration

| Setting           | Value                                      |
|-------------------|--------------------------------------------|
| CPU               | 0.25 vCPU (256 CPU units)                  |
| Memory            | 0.5 GB (512 MiB)                           |
| Architecture      | ARM64 (Graviton)                           |
| Operating system  | Linux                                      |
| Capacity provider | FARGATE_SPOT (primary), FARGATE (fallback) |
| Ephemeral storage | 20 GB (default, no additional)             |
| Desired count     | 1                                          |

### Network Architecture

```text
              Internet
                 |
            [IGW]
                 |
  +--------------+-----------------------------+
  |         VPC 10.0.0.0/16                    |
  |                                            |
  |  Tier 1: Public Subnets                    |
  |  +------------------+  +----------------+ |
  |  | 10.0.0.0/24      |  | 10.0.1.0/24    | |
  |  | us-east-1a       |  | us-east-1b     | |
  |  |                  |  |                 | |
  |  | [fck-nat]        |  |                 | |
  |  +--------+---------+  +----------------+ |
  |           |                                |
  |           | Route: 0.0.0.0/0 -> ENI        |
  |           |                                |
  |  Tier 2: Private Subnets                   |
  |  +------------------+  +----------------+ |
  |  | 10.0.10.0/24     |  | 10.0.11.0/24   | |
  |  | us-east-1a       |  | us-east-1b     | |
  |  |                  |  |                 | |
  |  | [ECS Fargate]    |  |                 | |
  |  | api container    |  |                 | |
  |  | :8080            |  |                 | |
  |  +--------+---------+  +----------------+ |
  |           |                                |
  |           | VPC-internal (private IP)       |
  |           |                                |
  |  Tier 3: Isolated Subnets                  |
  |  +------------------+  +----------------+ |
  |  | 10.0.20.0/24     |  | 10.0.21.0/24   | |
  |  | us-east-1a       |  | us-east-1b     | |
  |  | [RDS PostgreSQL] |  |                 | |
  |  +------------------+  +----------------+ |
  +--------------------------------------------+
```

**Traffic flow:**

1. ECS Fargate task starts in a private subnet (10.0.10.0/24 or 10.0.11.0/24)
2. Task pulls container image from ECR via fck-nat (outbound internet)
3. Task connects to RDS in the isolated subnet (10.0.20.0/24) on port 5432
   **via VPC-internal routing** (never touches the public internet)
4. Task calls Voyage AI API via fck-nat (outbound internet for embeddings)
5. Task sends logs to CloudWatch Logs via AWS internal networking

### VPC-Internal Connectivity: Fargate to RDS

This section addresses the critical requirement that Fargate tasks must
connect to RDS entirely within the VPC, never traversing the public internet.

**How VPC routing guarantees internal connectivity:**

The Fargate task connects to RDS using the RDS endpoint hostname (e.g.,
`main-prod.xxxx.us-east-1.rds.amazonaws.com`). This hostname resolves to the
RDS instance's **private IP address** within the VPC (e.g., `10.0.20.x`).
The connection path is:

1. Fargate task ENI (10.0.10.x or 10.0.11.x) initiates TCP connection to
   RDS private IP (10.0.20.x) on port 5432
2. VPC route table has a `local` route for `10.0.0.0/16` -- all traffic
   within the VPC CIDR is routed internally by the VPC router
3. The packet travels directly from the private subnet to the isolated
   subnet via the VPC internal router
4. The RDS security group's ingress rule allows port 5432 from the Fargate
   security group
5. The connection is established entirely within the VPC

**Why this traffic never touches the internet:**

- The RDS instance has `publicly_accessible = false` (confirmed in
  `modules/rds/main.tf`). This means RDS has no public IP address and its
  DNS hostname resolves only to a private IP.
- The destination IP (10.0.20.x) falls within the VPC CIDR (10.0.0.0/16).
  The VPC route table's `local` route has the highest priority and handles
  all intra-VPC traffic before the `0.0.0.0/0` route to fck-nat is
  evaluated. Traffic destined for a private IP within the VPC is never sent
  to the NAT instance.
- The isolated subnets have no route to the internet (no `0.0.0.0/0` route
  in their route table), so even if traffic somehow reached the internet,
  the return path would not exist.

**Security layers guaranteeing VPC-internal RDS access:**

| Layer                   | Mechanism                                                  | Effect                                           |
|-------------------------|------------------------------------------------------------|--------------------------------------------------|
| RDS configuration       | `publicly_accessible = false`                              | No public IP; DNS resolves to private IP only    |
| VPC routing             | `local` route for 10.0.0.0/16                              | Intra-VPC traffic never leaves the VPC           |
| Isolated subnet routing | No 0.0.0.0/0 route                                         | Database subnets have zero internet connectivity |
| RDS security group      | Ingress from Fargate SG on port 5432 only                  | Only the API Fargate tasks can reach RDS         |
| Fargate security group  | Egress to 0.0.0.0/0 (all outbound)                         | Allows the connection to RDS private IP          |
| DNS resolution          | `enable_dns_hostnames = true`, `enable_dns_support = true` | RDS hostname resolves to private IP within VPC   |

**No VPC endpoints are needed for Fargate-to-RDS connectivity.** VPC
endpoints are for accessing AWS services (S3, ECR, CloudWatch, etc.) without
NAT. RDS is not an AWS API service -- it is a database instance running
inside the VPC with a private IP. The connection is a direct TCP socket
between two private IPs in the same VPC, using standard VPC routing.

### Capacity Provider Strategy: Spot vs On-Demand

The user requested Spot instances with the ability to easily switch to
on-demand. ECS supports this through capacity provider strategies at the
service level.

| Strategy                            | How It Works                           | Monthly Cost (0.25 vCPU, 0.5 GB ARM) | Risk                                       |
|-------------------------------------|----------------------------------------|--------------------------------------|--------------------------------------------|
| FARGATE_SPOT only                   | All tasks run on Spot                  | ~$2.16 (70% discount)                | Task can be interrupted with 2-min warning |
| FARGATE only                        | All tasks run on on-demand             | ~$7.21                               | No interruption risk                       |
| Mixed (base=1 FARGATE, weight SPOT) | Guarantees 1 on-demand, extras on Spot | On-demand cost + Spot for extras     | Base task never interrupted                |

**On-demand cost calculation (ARM, us-east-1):**

- vCPU: 0.25 * $0.03238/hr * 730 hrs/month = $5.91/month
- Memory: 0.5 GB * $0.00356/hr * 730 hrs/month = $1.30/month
- Ephemeral storage: 20 GB included (no charge under 20 GB)
- **Total on-demand: ~$7.21/month**

Note: The per-hour rates above are derived from the per-second rates
($0.0000089944/vCPU-sec and $0.0000009889/GB-sec) published by AWS.

**Spot cost calculation (ARM, us-east-1, ~70% discount):**

- **Total Spot: ~$2.16/month**

AWS Fargate Spot prices are not fixed; they adjust gradually based on supply
and demand. The 70% discount is the maximum advertised discount. In practice,
the discount varies but typically remains between 50-70% for ARM workloads in
us-east-1.

**Decision: FARGATE_SPOT with FARGATE fallback.** The capacity provider
strategy will use `FARGATE_SPOT` as the primary provider with `weight=1` and
`FARGATE` as fallback with `weight=0` and `base=0`. This means all tasks run
on Spot by default. To switch to on-demand, change the weights:

```hcl
# Current: Spot only
capacity_provider = "FARGATE_SPOT"  # weight=1
# fallback: FARGATE                 # weight=0

# To switch to on-demand: change capacity_provider to "FARGATE"
capacity_provider = "FARGATE"       # weight=1
```

This is a single-line change in the Terragrunt inputs.

### Can Fargate Cost Be Reduced Further?

The 0.25 vCPU / 0.5 GB configuration is the **absolute minimum** that AWS
Fargate supports. There is no smaller Fargate task size available. Combined
with Spot pricing and ARM (Graviton) architecture, this is already the
cheapest possible Fargate configuration.

| Optimization                                       | Already Applied? | Savings                                       |
|----------------------------------------------------|------------------|-----------------------------------------------|
| ARM64 (Graviton) instead of x86_64                 | Yes              | ~20% cheaper per vCPU-second                  |
| Fargate Spot instead of on-demand                  | Yes              | Up to 70% discount                            |
| Minimum task size (0.25 vCPU, 0.5 GB)              | Yes              | Smallest available                            |
| Compute Savings Plan (1-year or 3-year commitment) | No               | Up to 50% additional, but requires commitment |

The only remaining option to reduce compute cost further is a **Compute
Savings Plan**, which requires committing to a fixed hourly spend for 1 or 3
years. For an MVP, this commitment is premature. At ~$2.16/month, the cost is
already negligible.

To go below Fargate's $2.16/month minimum, you would need to switch to a
fundamentally different compute model:

| Alternative                         | Approximate Monthly Cost | Trade-off                                     |
|-------------------------------------|--------------------------|-----------------------------------------------|
| Fargate Spot (current)              | ~$2.16                   | Managed, no servers to maintain               |
| ECS on EC2 Spot (t4g.nano shared)   | ~$1.50-2.00              | Must manage EC2 instances, patching, AMIs     |
| AWS Lambda (if < 1M requests/month) | ~$0.00-0.50              | Requires rewriting the app for Lambda runtime |
| Fly.io / Railway free tier          | $0.00                    | Not AWS, vendor lock-in, limited resources    |

None of these alternatives are worth the added complexity for the MVP. The
current Fargate Spot configuration at ~$2.16/month is the best balance of
cost, simplicity, and operational overhead.

### IAM Role Strategy

ECS Fargate requires two distinct IAM roles:

1. **Execution Role** (`api_execution_role`) -- Used by the ECS agent (not
   the container) to:
    - Pull container images from ECR
    - Write logs to CloudWatch Logs
    - Read secrets from SSM Parameter Store

2. **Task Role** (`api_task_role`) -- Used by the application code inside
   the container to:
    - Access AWS services the application needs at runtime

The user asked whether the IAM role should be generic (shared by all Fargate
tasks) or specific (per-service). Here is the analysis:

| Approach                                               | Scope                        | Pros                                                     | Cons                                               |
|--------------------------------------------------------|------------------------------|----------------------------------------------------------|----------------------------------------------------|
| A. Generic shared role (e.g., `ecs-task-execution`)    | All ECS tasks in the account | Simple, fewer roles to manage                            | Overly broad permissions; violates least privilege |
| B. Per-service role (e.g., `api_execution_role`)       | Only the API service         | Least privilege; each service gets exactly what it needs | More roles to manage (one per service)             |
| C. Per-function role (e.g., `api-db-reader-execution`) | Only DB-reading tasks        | Too granular for ECS (roles are per-task-definition)     | Unnecessary complexity                             |

**Decision: Option B (per-service roles).** This follows AWS best practices
for least privilege. Each service (API, embedding-service, future services)
gets its own pair of roles.

Role names:

- Execution role: `api_execution_role`
- Task role: `api_task_role`

The execution role needs:

- `ecr:GetAuthorizationToken`
- `ecr:BatchCheckLayerAvailability`, `ecr:GetDownloadUrlForLayer`,
  `ecr:BatchGetImage`
- `logs:CreateLogStream`, `logs:PutLogEvents`
- `ssm:GetParameters` (scoped to the specific parameter ARNs)

### RDS Connectivity: IAM vs Password Authentication

The user asked whether IAM roles are needed for RDS database access. Here is
the analysis:

**How the API currently connects to RDS:**

The API connects to RDS using standard PostgreSQL password authentication.
The connection string is constructed from config values (`host`, `port`,
`user`, `name`) and the `TOKENOVERFLOW_DATABASE_PASSWORD` environment
variable (see `apps/api/src/config.rs`, `DatabaseConfig::url()`). This is
a direct TCP connection using the `libpq` PostgreSQL client library.

**IAM database authentication vs password authentication:**

| Aspect              | Password Authentication (current)                     | IAM Database Authentication                                                    |
|---------------------|-------------------------------------------------------|--------------------------------------------------------------------------------|
| How it works        | Static password stored in SSM, injected as env var    | Temporary token (15-min lifetime) generated via AWS API                        |
| Code changes needed | None (already implemented)                            | Significant: must integrate AWS SDK to generate auth tokens on each connection |
| Connection limit    | Unlimited                                             | AWS recommends < 200 connections/second                                        |
| Network path        | Direct TCP to RDS private IP                          | Same TCP path, but also requires IAM API call to get token                     |
| Security            | Password rotated manually; stored in SSM SecureString | No persistent password; tokens auto-expire                                     |
| IAM role needed     | No (password is a database-level credential)          | Yes (task role needs `rds-db:connect` permission)                              |

**Decision: Keep password authentication.** The API already uses password
authentication via `TOKENOVERFLOW_DATABASE_PASSWORD`. Switching to IAM
authentication would require significant application code changes (integrating
the AWS SDK for Rust to generate authentication tokens) with no meaningful
security benefit at MVP scale -- the password is already stored securely in
SSM Parameter Store and injected by ECS at task startup.

**The task role (`api_task_role`) does not need any RDS-related IAM
permissions.** RDS access is controlled by:

1. **Security groups:** The RDS security group allows ingress on port 5432
   only from the Fargate security group
2. **Database credentials:** Username from config, password from SSM
3. **Network isolation:** RDS is in isolated subnets with no internet route

The task role is created empty for future use (e.g., if the API needs to
access S3, SQS, SNS, etc.).

### Health Check Configuration

The API exposes `GET /health` which returns
`{"status":"ok","database":"connected"}`
with a 200 status code when healthy. The health check verifies database
connectivity by acquiring a connection from the pool.

ECS supports container-level health checks in the task definition. These are
separate from load balancer health checks (which are not applicable here since
there is no ALB yet).

| Parameter    | Docker Default | Dockerfile Current                | ECS Best Practice                        | Decision                                                |
|--------------|----------------|-----------------------------------|------------------------------------------|---------------------------------------------------------|
| Command      | NONE           | (none -- removed from Dockerfile) | HTTP check via curl                      | `["CMD", "curl", "-f", "http://localhost:8080/health"]` |
| Interval     | 30s            | --                                | 15-30s for non-critical services         | 30s                                                     |
| Timeout      | 30s            | --                                | 5-10s                                    | 5s                                                      |
| Retries      | 3              | --                                | 3 (industry standard)                    | 3                                                       |
| Start period | 0s             | --                                | 30-60s for apps that need DB connections | 60s                                                     |

**Rationale for 30s interval:** Each health check invocation acquires a
database connection from the pool. At 30s intervals, that is 2 checks per
minute -- sufficient to detect failures within 90 seconds (30s interval * 3
retries) while reducing unnecessary database pool churn. Industry standard
for container health checks is 15-30 seconds.

**Rationale for 60s start period:** The application needs to establish a
database connection pool on startup. In a cloud environment, the first
connection to RDS through the network may take longer, especially after a cold
start. 60 seconds provides a generous buffer without affecting steady-state
behavior. During the start period, failed health checks do not count toward
the retry limit.

### Restart Policy

ECS Fargate services have built-in restart behavior. When a task fails
(health check failure, OOM, application crash), the ECS service scheduler
automatically launches a replacement task to maintain the desired count.

For additional resilience, the ECS service's `deployment_circuit_breaker`
should be enabled. This prevents an infinite crash loop during a bad
deployment by rolling back to the last working task definition after a
configurable number of failures.

### Deployment Strategy

Deployments are fully Terraform-managed. When the `container_image` variable
is updated (e.g., to a new tag like `v1.2.3`), running `terragrunt apply`
will:

1. Create a new ECS task definition revision with the updated image
2. Update the ECS service to reference the new task definition
3. ECS performs a rolling deployment: starts a new task with the new image,
   waits for it to become healthy, then stops the old task

There is no `lifecycle { ignore_changes }` block on the ECS service. This
means Terraform is the single source of truth for the deployed image version.
Every `terragrunt apply` ensures the running service matches the declared
configuration.

To deploy a new image:

1. Build and push the new image to ECR with a unique tag
2. Update `container_image` in `terragrunt.hcl` to reference the new tag
3. Run `terragrunt apply`

### CloudWatch Logs Cost Analysis

The API will run with `info` level logging in production. The `production.toml`
config currently has `level = "warn"`, but this will be overridden via the
`TOKENOVERFLOW__LOGGING__LEVEL` environment variable set to `info` in the task
definition.

**Pricing (us-east-1):**

| Component                         | Rate              | Free Tier                  |
|-----------------------------------|-------------------|----------------------------|
| Log ingestion (standard)          | $0.50/GB          | 5 GB/month                 |
| Log storage                       | $0.03/GB/month    | Included in 5 GB free tier |
| Log Insights queries              | $0.005/GB scanned | Included in 5 GB free tier |
| Log ingestion (Infrequent Access) | $0.25/GB          | Same 5 GB pool             |

**Estimated log volume for a single API container at `info` level:**

- Startup logs: ~5 KB (one-time)
- Request logging at info level: ~0.5-1 KB per request
- At 1,000 requests/day: ~15-30 MB/month
- At 10,000 requests/day: ~150-300 MB/month
- Warn/error logs: ~0-10 KB/day additional

| Scenario                              | Monthly Log Volume | Monthly Cost      |
|---------------------------------------|--------------------|-------------------|
| Low traffic (1,000 req/day, info)     | ~30 MB             | $0.00 (free tier) |
| Medium traffic (10,000 req/day, info) | ~300 MB            | $0.00 (free tier) |
| High traffic (100,000 req/day, info)  | ~3 GB              | $0.00 (free tier) |
| Extreme (debug level, high traffic)   | ~10 GB             | $2.50             |

**Retention policy decision:** 14 days. Logs older than 14 days are
automatically deleted. This keeps storage minimal while providing enough
history for debugging recent issues. At the expected log volumes (well within
the 5 GB free tier), storage cost is effectively zero regardless of retention,
but short retention is good hygiene.

**Log class decision:** Standard (not Infrequent Access). At the expected
volume, the price difference is irrelevant. Standard logs allow real-time
tailing and full Logs Insights support.

### Metrics Cost Analysis

**Built-in ECS metrics (free):**

ECS publishes the following metrics to CloudWatch at no additional cost:

- CPUUtilization
- MemoryUtilization

These are published at the cluster and service level. No additional
configuration is required.

**Container Insights (paid):**

| Option                         | What You Get                                   | Custom Metrics Generated                                    | Monthly Cost          |
|--------------------------------|------------------------------------------------|-------------------------------------------------------------|-----------------------|
| A. Disabled                    | Only CPUUtilization + MemoryUtilization (free) | 0                                                           | $0.00                 |
| B. Standard Container Insights | Task-level CPU, memory, network, storage       | ~86 metrics (1 cluster × 29 + 1 service × 31 + 1 task × 26) | ~$6.02 ($0.07/metric) |
| C. Enhanced observability      | All of B + application-level metrics           | ~112+ metrics                                               | ~$7.84+               |

**Decision: Option A (disabled).** The free CPUUtilization and
MemoryUtilization metrics are sufficient for an MVP with a single task.
Container Insights would cost $6-8/month for a service that costs $2-7/month
in compute. The cost-to-value ratio is poor at this scale. Container Insights
can be enabled later with a single flag in the ECS cluster configuration when
the service grows.

### Trace Collection Cost Analysis

**AWS X-Ray pricing:**

| Component        | Rate             | Free Tier              |
|------------------|------------------|------------------------|
| Traces recorded  | $5.00/million    | 100,000 traces/month   |
| Traces retrieved | $0.50/million    | 1,000,000 traces/month |
| Trace storage    | Free for 30 days | --                     |

**Estimated trace volume for a single API container:**

At MVP traffic levels (estimated 1,000-10,000 requests/day):

| Scenario                        | Monthly Traces | Monthly Cost      |
|---------------------------------|----------------|-------------------|
| Low traffic (1,000 req/day)     | ~30,000        | $0.00 (free tier) |
| Medium traffic (10,000 req/day) | ~300,000       | $1.00             |
| High traffic (100,000 req/day)  | ~3,000,000     | $14.50            |

**Decision: Do not enable X-Ray tracing.** The API does not currently have
X-Ray instrumentation in the Rust application code. Adding tracing would
require integrating an X-Ray SDK or OpenTelemetry collector sidecar, which
adds complexity and sidecar cost. At MVP scale, the CloudWatch Logs at
`info` level provide sufficient debugging capability. Tracing can be added
as a separate design when the API has enough traffic to warrant it.

### SSM Parameter Store Cost Analysis

The design uses SSM Parameter Store (Standard tier) for storing the database
password and Voyage AI API key as SecureString parameters.

**SSM Parameter Store pricing:**

| Component                                     | Rate                  | Free Tier                |
|-----------------------------------------------|-----------------------|--------------------------|
| Standard parameters (up to 10,000)            | $0.00                 | Free (unlimited storage) |
| Advanced parameters                           | $0.05/parameter/month | --                       |
| API interactions (GetParameter, PutParameter) | $0.00 (standard tier) | Free                     |

**KMS pricing for SecureString decryption:**

SecureString parameters are encrypted using AWS KMS. Each time ECS retrieves
a secret to inject into a container, it calls KMS to decrypt the value.

| Component                           | Rate                  | Free Tier                  |
|-------------------------------------|-----------------------|----------------------------|
| KMS symmetric key (aws/ssm default) | $0.00                 | AWS-managed key is free    |
| KMS Decrypt operations              | $0.03/10,000 requests | 20,000 requests/month free |

**Estimated KMS usage:**

Each task start retrieves 2 secrets (database password + embedding API key) =
2 KMS Decrypt calls per task start. With a single Fargate task that restarts
occasionally (Spot interruptions, deployments), the monthly KMS Decrypt calls
will be well under 100 -- far below the 20,000/month free tier.

| Scenario                      | Monthly KMS Decrypt Calls | Monthly Cost      |
|-------------------------------|---------------------------|-------------------|
| Stable task (few restarts)    | ~10-20                    | $0.00 (free tier) |
| Frequent restarts (daily)     | ~60                       | $0.00 (free tier) |
| Heavy churn (hourly restarts) | ~1,500                    | $0.00 (free tier) |

**Decision: SSM Parameter Store costs $0.00/month.** Standard tier parameters
are free. The AWS-managed KMS key (`aws/ssm`) is free. KMS Decrypt operations
are well within the 20,000/month free tier. There is zero cost for this
component.

### Total Monthly Cost Summary

| Component                                    | Monthly Cost               |
|----------------------------------------------|----------------------------|
| Fargate Spot compute (0.25 vCPU, 0.5 GB ARM) | ~$2.16                     |
| ECR repository (private)                     | $0.00 (no per-repo charge) |
| ECR storage (single image, ~50 MB)           | $0.01 ($0.10/GB/month)     |
| CloudWatch Logs (info level, < 300 MB/month) | $0.00 (free tier)          |
| CloudWatch Metrics (built-in only)           | $0.00 (free)               |
| Container Insights                           | $0.00 (disabled)           |
| X-Ray tracing                                | $0.00 (not enabled)        |
| SSM Parameter Store (2 standard parameters)  | $0.00 (free tier)          |
| KMS Decrypt (< 100 calls/month)              | $0.00 (free tier)          |
| Ephemeral storage (20 GB default)            | $0.00 (included)           |
| **Total**                                    | **~$2.17/month**           |

If Spot is unavailable and falls back to on-demand:

| Component                 | Monthly Cost     |
|---------------------------|------------------|
| Fargate On-Demand compute | ~$7.21/month     |
| Everything else           | ~$0.01           |
| **Total**                 | **~$7.22/month** |

### Module Strategy

Consistent with the VPC, RDS, and NAT designs, a wrapper module pattern is
used. The module is split into files by concern:

```text
infra/terraform/
  modules/
    ecs/                          # new wrapper module
      cluster.tf                  # ECS cluster + capacity providers (shared)
      api.tf                      # API: IAM roles, security groups, log group, task definition, service
      ecr.tf                      # ECR repository for API images
      variables.tf
      outputs.tf
  live/
    prod/
      ecs/                        # new Terragrunt unit
        terragrunt.hcl
```

The module is named `ecs` (not `fargate` or `api`) because:

- It manages the ECS cluster, which could host multiple services in the future
- The Fargate launch type is a configuration detail, not a naming concern
- Naming it `api` would be too narrow if the embedding-service is deployed
  to the same cluster later

The file split follows a simple principle: **`cluster.tf` contains shared
cluster resources; `api.tf` contains everything specific to the API service.**
If you delete `api.tf`, all API-linked resources are gone -- IAM roles,
security groups, log group, task definition, and ECS service. When a second
service (e.g., embedding-service) is added to the cluster, it gets its own
`.tf` file without touching the cluster configuration or `api.tf`.

### Directory Structure (After)

```text
infra/terraform/
  modules/
    org/                     # existing
    sso/                     # existing
    vpc/                     # existing
    rds/                     # existing
    nat/                     # existing
    ecs/                     # new
      cluster.tf
      api.tf
      ecr.tf
      variables.tf
      outputs.tf
  live/
    root.hcl                 # existing (unchanged)
    global/
      env.hcl                # existing (unchanged)
      org/                   # existing (unchanged)
      sso/                   # existing (unchanged)
    prod/
      env.hcl                # existing (unchanged)
      vpc/
        terragrunt.hcl       # existing (unchanged)
      rds/
        terragrunt.hcl       # existing (unchanged)
      nat/
        terragrunt.hcl       # existing (unchanged)
      ecs/
        terragrunt.hcl       # new
    dev/
      env.hcl                # existing (unchanged)
```

---

## Interfaces

This section documents every file that will be created or modified, and the
contracts between them.

### New Files

#### `infra/terraform/modules/ecs/variables.tf`

```hcl
variable "env_name" {
    description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
    type        = string
}

variable "vpc_id" {
    description = "ID of the VPC where the ECS resources will be created."
    type        = string
}

variable "private_subnet_ids" {
    description = "List of private subnet IDs where Fargate tasks will be placed."
    type = list(string)
}

variable "container_image" {
    description = "Full ECR image URI including tag (e.g., 123456789.dkr.ecr.us-east-1.amazonaws.com/api:v1.0.0)."
    type        = string
}

variable "container_port" {
    description = "Port the container listens on."
    type        = number
    default     = 8080
}

variable "cpu" {
    description = "CPU units for the Fargate task (256 = 0.25 vCPU)."
    type        = number
    default     = 256
}

variable "memory" {
    description = "Memory in MiB for the Fargate task (512 = 0.5 GB)."
    type        = number
    default     = 512
}

variable "desired_count" {
    description = "Number of task instances to run."
    type        = number
    default     = 1
}

variable "capacity_provider" {
    description = "Primary capacity provider for the ECS service. Use FARGATE_SPOT for Spot or FARGATE for on-demand."
    type        = string
    default     = "FARGATE_SPOT"

    validation {
        condition = contains(["FARGATE", "FARGATE_SPOT"], var.capacity_provider)
        error_message = "capacity_provider must be either FARGATE or FARGATE_SPOT."
    }
}

variable "tokenoverflow_env" {
    description = "Value for the TOKENOVERFLOW_ENV environment variable (e.g., production, development)."
    type        = string
    default     = "production"
}

variable "database_password_arn" {
    description = "ARN of the SSM Parameter Store parameter containing the database password."
    type        = string
}

variable "embedding_api_key_arn" {
    description = "ARN of the SSM Parameter Store parameter containing the Voyage AI API key."
    type        = string
}

variable "database_host" {
    description = "RDS database hostname for the TOKENOVERFLOW__DATABASE__HOST override."
    type        = string
}

variable "database_port" {
    description = "RDS database port for the TOKENOVERFLOW__DATABASE__PORT override."
    type        = number
    default     = 5432
}

variable "rds_security_group_id" {
    description = "ID of the RDS security group. Used to add an ingress rule allowing Fargate tasks to connect."
    type        = string
}

variable "log_retention_days" {
    description = "Number of days to retain CloudWatch Logs. Set to 0 for indefinite retention."
    type        = number
    default     = 14
}

variable "enable_container_insights" {
    description = "Enable CloudWatch Container Insights on the ECS cluster."
    type        = bool
    default     = false
}

variable "ecr_image_keep_count" {
    description = "Number of tagged images to retain in ECR. Older images are expired by lifecycle policy."
    type        = number
    default     = 10
}
```

#### `infra/terraform/modules/ecs/ecr.tf`

```hcl
# -----------------------------------------------------------------------------
# ECR Repository
# -----------------------------------------------------------------------------

resource "aws_ecr_repository" "api" {
    name                 = "api"
    image_tag_mutability = "MUTABLE"

    image_scanning_configuration {
        scan_on_push = true
    }

    tags = {
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}

resource "aws_ecr_lifecycle_policy" "api" {
    repository = aws_ecr_repository.api.name

    policy = jsonencode({
        rules = [
            {
                rulePriority = 1
                description  = "Keep only the last ${var.ecr_image_keep_count} images"
                selection = {
                    tagStatus   = "any"
                    countType   = "imageCountMoreThan"
                    countNumber = var.ecr_image_keep_count
                }
                action = {
                    type = "expire"
                }
            }
        ]
    })
}
```

#### `infra/terraform/modules/ecs/cluster.tf`

```hcl
# -----------------------------------------------------------------------------
# ECS Cluster (shared across all services in this environment)
# -----------------------------------------------------------------------------

resource "aws_ecs_cluster" "main" {
    name = var.env_name

    setting {
        name  = "containerInsights"
        value = var.enable_container_insights ? "enabled" : "disabled"
    }

    tags = {
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}

resource "aws_ecs_cluster_capacity_providers" "main" {
    cluster_name = aws_ecs_cluster.main.name
    capacity_providers = ["FARGATE", "FARGATE_SPOT"]
}

data "aws_region" "current" {}
```

#### `infra/terraform/modules/ecs/api.tf`

```hcl
# -----------------------------------------------------------------------------
# IAM: Execution Role (api_execution_role)
# Used by the ECS agent to pull images from ECR, write logs to CloudWatch,
# and read secrets from SSM Parameter Store.
# -----------------------------------------------------------------------------

data "aws_iam_policy_document" "ecs_assume_role" {
    statement {
        actions = ["sts:AssumeRole"]
        principals {
            type = "Service"
            identifiers = ["ecs-tasks.amazonaws.com"]
        }
    }
}

resource "aws_iam_role" "execution" {
    name               = "api_execution_role"
    assume_role_policy = data.aws_iam_policy_document.ecs_assume_role.json

    tags = {
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}

resource "aws_iam_role_policy_attachment" "execution_base" {
    role       = aws_iam_role.execution.name
    policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

# Allow the execution role to read secrets from SSM Parameter Store
data "aws_iam_policy_document" "secrets_read" {
    statement {
        actions = [
            "ssm:GetParameters",
        ]
        resources = [
            var.database_password_arn,
            var.embedding_api_key_arn,
        ]
    }
}

resource "aws_iam_role_policy" "execution_secrets" {
    name   = "api-secrets-read"
    role   = aws_iam_role.execution.id
    policy = data.aws_iam_policy_document.secrets_read.json
}

# -----------------------------------------------------------------------------
# IAM: Task Role (api_task_role)
# Used by the application code inside the container. Currently empty.
#
# RDS access does NOT require IAM permissions. The API connects to RDS using
# standard PostgreSQL password authentication over a direct TCP connection
# within the VPC. Access is controlled by:
#   1. Security groups (RDS SG allows ingress from Fargate SG on port 5432)
#   2. Database credentials (password from SSM, injected by ECS)
#   3. Network isolation (RDS in isolated subnets, no public IP)
#
# This role is created empty for future use (e.g., S3, SQS, SNS access).
# -----------------------------------------------------------------------------

resource "aws_iam_role" "task" {
    name               = "api_task_role"
    assume_role_policy = data.aws_iam_policy_document.ecs_assume_role.json

    tags = {
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}

# -----------------------------------------------------------------------------
# Security Groups
# -----------------------------------------------------------------------------

# Security group for Fargate tasks
resource "aws_security_group" "fargate" {
    name        = "${var.env_name}-api-fargate"
    description = "Allow outbound traffic from API Fargate tasks"
    vpc_id      = var.vpc_id

    tags = {
        Name        = "${var.env_name}-api-fargate"
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}

# Allow all outbound traffic (needed for ECR pulls, external API calls, RDS)
resource "aws_vpc_security_group_egress_rule" "all_outbound" {
    security_group_id = aws_security_group.fargate.id
    description       = "All outbound traffic"
    ip_protocol       = "-1"
    cidr_ipv4         = "0.0.0.0/0"
}

# Allow RDS access: add ingress rule to the RDS security group.
# This uses security group referencing (not CIDR), so traffic is allowed
# based on the source security group ID. The connection stays entirely
# within the VPC -- the Fargate task connects to the RDS private IP via
# the VPC's local route (10.0.0.0/16 -> local). No internet traversal.
resource "aws_vpc_security_group_ingress_rule" "rds_from_fargate" {
    security_group_id            = var.rds_security_group_id
    description                  = "PostgreSQL from API Fargate tasks"
    from_port                    = 5432
    to_port                      = 5432
    ip_protocol                  = "tcp"
    referenced_security_group_id = aws_security_group.fargate.id
}

# -----------------------------------------------------------------------------
# CloudWatch Log Group (API)
# -----------------------------------------------------------------------------

resource "aws_cloudwatch_log_group" "api" {
    name              = "/ecs/${var.env_name}/api"
    retention_in_days = var.log_retention_days

    tags = {
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}

# -----------------------------------------------------------------------------
# ECS Task Definition (API)
# -----------------------------------------------------------------------------

resource "aws_ecs_task_definition" "api" {
    family             = "${var.env_name}-api"
    requires_compatibilities = ["FARGATE"]
    network_mode       = "awsvpc"
    cpu                = var.cpu
    memory             = var.memory
    execution_role_arn = aws_iam_role.execution.arn
    task_role_arn      = aws_iam_role.task.arn

    runtime_platform {
        operating_system_family = "LINUX"
        cpu_architecture        = "ARM64"
    }

    container_definitions = jsonencode([
        {
            name      = "api"
            image     = var.container_image
            essential = true

            portMappings = [
                {
                    containerPort = var.container_port
                    protocol      = "tcp"
                }
            ]

            environment = [
                {
                    name  = "TOKENOVERFLOW_ENV"
                    value = var.tokenoverflow_env
                },
                {
                    name  = "TOKENOVERFLOW__DATABASE__HOST"
                    value = var.database_host
                },
                {
                    name = "TOKENOVERFLOW__DATABASE__PORT"
                    value = tostring(var.database_port)
                },
                {
                    name  = "TOKENOVERFLOW__LOGGING__LEVEL"
                    value = "info"
                },
            ]

            secrets = [
                {
                    name      = "TOKENOVERFLOW_DATABASE_PASSWORD"
                    valueFrom = var.database_password_arn
                },
                {
                    name      = "TOKENOVERFLOW_EMBEDDING_API_KEY"
                    valueFrom = var.embedding_api_key_arn
                },
            ]

            healthCheck = {
                command = ["CMD", "curl", "-f", "http://localhost:8080/health"]
                interval    = 30
                timeout     = 5
                retries     = 3
                startPeriod = 60
            }

            logConfiguration = {
                logDriver = "awslogs"
                options = {
                    "awslogs-group"         = aws_cloudwatch_log_group.api.name
                    "awslogs-region"        = data.aws_region.current.name
                    "awslogs-stream-prefix" = "api"
                }
            }
        }
    ])

    tags = {
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}

# -----------------------------------------------------------------------------
# ECS Service (API)
# -----------------------------------------------------------------------------

resource "aws_ecs_service" "api" {
    name            = "api"
    cluster         = aws_ecs_cluster.main.id
    task_definition = aws_ecs_task_definition.api.arn
    desired_count   = var.desired_count

    capacity_provider_strategy {
        capacity_provider = var.capacity_provider
        weight            = 1
        base              = 0
    }

    network_configuration {
        subnets          = var.private_subnet_ids
        security_groups = [aws_security_group.fargate.id]
        assign_public_ip = false
    }

    deployment_circuit_breaker {
        enable   = true
        rollback = true
    }

    tags = {
        Environment = var.env_name
        ManagedBy   = "opentofu"
    }
}
```

#### `infra/terraform/modules/ecs/outputs.tf`

```hcl
output "cluster_id" {
    description = "The ID of the ECS cluster."
    value       = aws_ecs_cluster.main.id
}

output "cluster_name" {
    description = "The name of the ECS cluster."
    value       = aws_ecs_cluster.main.name
}

output "service_name" {
    description = "The name of the ECS service."
    value       = aws_ecs_service.api.name
}

output "task_definition_arn" {
    description = "The ARN of the ECS task definition."
    value       = aws_ecs_task_definition.api.arn
}

output "execution_role_arn" {
    description = "The ARN of the task execution IAM role."
    value       = aws_iam_role.execution.arn
}

output "task_role_arn" {
    description = "The ARN of the task IAM role."
    value       = aws_iam_role.task.arn
}

output "security_group_id" {
    description = "The ID of the Fargate task security group."
    value       = aws_security_group.fargate.id
}

output "log_group_name" {
    description = "The name of the CloudWatch Logs log group."
    value       = aws_cloudwatch_log_group.api.name
}

output "ecr_repository_url" {
    description = "The URL of the ECR repository for API images."
    value       = aws_ecr_repository.api.repository_url
}
```

#### `infra/terraform/live/prod/ecs/terragrunt.hcl`

```hcl
include "root" {
    path = find_in_parent_folders("root.hcl")
}

terraform {
    source = "../../../modules/ecs"
}

dependency "vpc" {
    config_path = "../vpc"
}

dependency "rds" {
    config_path = "../rds"
}

inputs = {
    env_name = "prod"
    vpc_id   = dependency.vpc.outputs.vpc_id
    private_subnet_ids = dependency.vpc.outputs.private_subnet_ids

    # Container
    container_image = "591120835062.dkr.ecr.us-east-1.amazonaws.com/api:latest"
    container_port = 8080

    # Compute
    cpu           = 256
    memory        = 512
    desired_count = 1
    capacity_provider = "FARGATE_SPOT"

    # Application config
    tokenoverflow_env = "production"
    database_host     = dependency.rds.outputs.db_instance_address
    database_port     = dependency.rds.outputs.db_instance_port

    # Secrets (SSM Parameter Store ARNs)
    # These must be created manually in SSM Parameter Store before deploying:
    #   aws ssm put-parameter --name "/tokenoverflow/prod/database-password" \
    #     --type SecureString --value "<password>" --profile tokenoverflow-prod-admin
    #   aws ssm put-parameter --name "/tokenoverflow/prod/embedding-api-key" \
    #     --type SecureString --value "<key>" --profile tokenoverflow-prod-admin
    database_password_arn = "arn:aws:ssm:us-east-1:591120835062:parameter/tokenoverflow/prod/database-password"
    embedding_api_key_arn = "arn:aws:ssm:us-east-1:591120835062:parameter/tokenoverflow/prod/embedding-api-key"

    # Security
    rds_security_group_id = dependency.rds.outputs.security_group_id

    # Observability
    log_retention_days = 14
    enable_container_insights = false

    # ECR
    ecr_image_keep_count = 10
}
```

### Files NOT Modified

| File                              | Reason                                                                                                         |
|-----------------------------------|----------------------------------------------------------------------------------------------------------------|
| `modules/vpc/main.tf`             | No changes needed. Private subnets already exist.                                                              |
| `modules/vpc/outputs.tf`          | Already exports `vpc_id` and `private_subnet_ids`.                                                             |
| `modules/nat/main.tf`             | fck-nat already provides outbound internet for private subnets.                                                |
| `modules/rds/main.tf`             | RDS configuration unchanged. `publicly_accessible = false` already set.                                        |
| `modules/rds/outputs.tf`          | Already exports `security_group_id`, `db_instance_address`, `db_instance_port`.                                |
| `modules/rds/security_groups.tf`  | The ECS module adds its own ingress rule to the RDS SG using `rds_security_group_id`. No changes here.         |
| `live/root.hcl`                   | Provider version 6.33.0 satisfies all ECS/ECR resource requirements.                                           |
| `live/prod/vpc/terragrunt.hcl`    | No changes needed.                                                                                             |
| `live/prod/rds/terragrunt.hcl`    | No changes needed. The ECS module manages the RDS SG ingress rule.                                             |
| `live/prod/nat/terragrunt.hcl`    | No changes needed.                                                                                             |
| `apps/api/Dockerfile`             | No changes needed. Already pins `--platform=linux/arm64` and strips the binary.                                |
| `apps/api/config/production.toml` | No changes needed. Log level is overridden via `TOKENOVERFLOW__LOGGING__LEVEL` env var in the task definition. |

---

## Logic

This section defines the exact sequence of operations to implement the ECS
Fargate deployment.

### Phase 0: Prerequisites

Before starting, verify these prerequisites are met:

**Step 0.1:** Confirm the VPC, RDS, and NAT modules are deployed:

```bash
source scripts/src/includes.sh
tg plan prod
```

All three units (vpc, rds, nat) should show "No changes."

**Step 0.2:** Create SSM Parameter Store secrets:

```bash
aws ssm put-parameter \
  --name "/tokenoverflow/prod/database-password" \
  --type SecureString \
  --value "<database-password>" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1

aws ssm put-parameter \
  --name "/tokenoverflow/prod/embedding-api-key" \
  --type SecureString \
  --value "<voyage-api-key>" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1
```

### Phase 1: Create the ECS wrapper module

**Step 1.1:** Create the module directory:

```bash
mkdir -p infra/terraform/modules/ecs
```

**Step 1.2:** Create `infra/terraform/modules/ecs/variables.tf` with the
content defined in the Interfaces section.

**Step 1.3:** Create `infra/terraform/modules/ecs/ecr.tf` with the content
defined in the Interfaces section.

**Step 1.4:** Create `infra/terraform/modules/ecs/cluster.tf` with the
content defined in the Interfaces section.

**Step 1.5:** Create `infra/terraform/modules/ecs/api.tf` with the content
defined in the Interfaces section.

**Step 1.6:** Create `infra/terraform/modules/ecs/outputs.tf` with the
content defined in the Interfaces section.

**Step 1.7:** Validate with TFLint:

```bash
cd infra/terraform/modules/ecs
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

### Phase 2: Create the prod Terragrunt unit

**Step 2.1:** Create the unit directory:

```bash
mkdir -p infra/terraform/live/prod/ecs
```

**Step 2.2:** Create `infra/terraform/live/prod/ecs/terragrunt.hcl` with
the content defined in the Interfaces section.

### Phase 3: Deploy ECS to production

**Step 3.1:** Log in to the prod AWS account:

```bash
aws sso login --profile tokenoverflow-prod-admin
```

**Step 3.2:** Initialize the prod ECS unit:

```bash
cd infra/terraform/live/prod/ecs
terragrunt init
```

**Step 3.3:** Review the plan:

```bash
terragrunt plan
```

Expected resources to be created:

- 1 `aws_ecr_repository` (api)
- 1 `aws_ecr_lifecycle_policy` (keep last 10 images)
- 1 `aws_ecs_cluster` named `prod`
- 1 `aws_ecs_cluster_capacity_providers` (FARGATE + FARGATE_SPOT)
- 1 `aws_ecs_task_definition` (family: `prod-api`, ARM64, 256 CPU, 512 MiB)
- 1 `aws_ecs_service` (name: `api`, FARGATE_SPOT, desired_count: 1)
- 1 `aws_iam_role` (execution: `api_execution_role`)
- 1 `aws_iam_role_policy_attachment` (AmazonECSTaskExecutionRolePolicy)
- 1 `aws_iam_role_policy` (secrets read)
- 1 `aws_iam_role` (task: `api_task_role`)
- 1 `aws_security_group` (fargate: `prod-api-fargate`)
- 1 `aws_vpc_security_group_egress_rule` (all outbound)
- 1 `aws_vpc_security_group_ingress_rule` (RDS from Fargate)
- 1 `aws_cloudwatch_log_group` (`/ecs/prod/api`, 14 day retention)

The plan should NOT show any changes to VPC, RDS, or NAT resources (except
the RDS security group will gain a new ingress rule, which is expected).

**Step 3.4:** Apply (ECR repository first, then build/push image, then the
rest):

The first `terragrunt apply` will create all resources including the ECR
repository. However, the ECS service will fail to start because no container
image exists yet. This is expected.

```bash
terragrunt apply
```

**Step 3.5:** Build and push the API container image (ARM64):

```bash
# Get the ECR repository URL from Terraform output
ECR_URL=$(terragrunt output -raw ecr_repository_url)

# Log in to ECR
aws ecr get-login-password --region us-east-1 --profile tokenoverflow-prod-admin | \
  docker login --username AWS --password-stdin "$ECR_URL"

# Build and push for ARM64
docker buildx build \
  --platform linux/arm64 \
  --tag "${ECR_URL}:latest" \
  --push \
  --file apps/api/Dockerfile \
  .
```

**Step 3.6:** Force a new deployment to pick up the pushed image:

```bash
aws ecs update-service \
  --cluster prod \
  --service api \
  --force-new-deployment \
  --profile tokenoverflow-prod-admin \
  --region us-east-1
```

**Step 3.7:** Verify the task is running:

```bash
aws ecs describe-services \
  --cluster prod \
  --services api \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'services[0].{DesiredCount:desiredCount,RunningCount:runningCount,Status:status,CapacityProvider:capacityProviderStrategy[0].capacityProvider}'
```

Expected output: `RunningCount: 1`, `Status: ACTIVE`,
`CapacityProvider: FARGATE_SPOT`.

**Step 3.8:** Check container health:

```bash
TASK_ARN=$(aws ecs list-tasks \
  --cluster prod \
  --service-name api \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'taskArns[0]' \
  --output text)

aws ecs describe-tasks \
  --cluster prod \
  --tasks "$TASK_ARN" \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'tasks[0].containers[0].{Name:name,HealthStatus:healthStatus,LastStatus:lastStatus}'
```

Expected output: `HealthStatus: HEALTHY`, `LastStatus: RUNNING`.

**Step 3.9:** Check CloudWatch Logs:

```bash
aws logs tail /ecs/prod/api \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --since 5m
```

Should show application startup logs at `info` level.

**Step 3.10:** Commit:

```bash
git add infra/terraform/modules/ecs/
git add infra/terraform/live/prod/ecs/
git commit -m "infra: deploy API to ECS Fargate Spot (ARM64) with ECR"
```

---

## Edge Cases & Constraints

### 1. Fargate Spot interruption

**Risk:** AWS can reclaim Spot capacity with a 2-minute warning. The running
task will be stopped and the ECS service scheduler will attempt to launch a
replacement. During the interruption window, the API is unavailable.

**Mitigation:** For an MVP with a single task, brief interruptions are
acceptable. The ECS service scheduler automatically launches a replacement
task. The `deployment_circuit_breaker` prevents infinite restart loops. If
Spot availability is consistently poor, change `capacity_provider` from
`FARGATE_SPOT` to `FARGATE` in the Terragrunt inputs (a single-line change).

### 2. Database connection on startup

**Risk:** The API connects to RDS on startup. If the database is temporarily
unreachable (e.g., during an RDS maintenance window), the task may fail to
start and the health check will report unhealthy.

**Mitigation:** The 60-second `startPeriod` on the health check provides time
for the database to become available. The ECS service scheduler will continue
restarting the task. The `deployment_circuit_breaker` with rollback enabled
ensures that a fundamentally broken deployment does not consume resources
indefinitely.

### 3. Secrets rotation

**Risk:** If the database password or Voyage AI API key is rotated in SSM
Parameter Store, running tasks will continue using the old value until they
are restarted.

**Mitigation:** After rotating a secret in SSM, force a new deployment to
pick up the new value:

```bash
aws ecs update-service \
  --cluster prod \
  --service api \
  --force-new-deployment \
  --profile tokenoverflow-prod-admin \
  --region us-east-1
```

### 4. Container image architecture mismatch

**Risk:** If an x86_64 image is accidentally pushed to ECR and the task
definition specifies ARM64, the task will fail to start with an architecture
mismatch error.

**Mitigation:** The Dockerfile already pins `--platform=linux/arm64` in both
the build and runtime stages. The `runtime_platform` block in the task
definition explicitly specifies ARM64. CI/CD pipelines (when built) should
enforce this.

### 5. No ALB means no external access

**Risk:** Without an Application Load Balancer or public IP, the API is not
accessible from the internet. Fargate tasks in private subnets with
`assign_public_ip = false` have no inbound path.

**Mitigation:** This is expected for the initial deployment. The ALB is a
separate infrastructure concern that will be addressed in a follow-up design.
For testing, use ECS Exec (requires additional IAM permissions) or temporarily
assign a public IP in a public subnet.

### 6. fck-nat single point of failure affects image pulls

**Risk:** If the fck-nat instance is down when a new Fargate task starts, the
task cannot pull the container image from ECR (unless a VPC endpoint exists).
The task will fail to start.

**Mitigation:** The fck-nat ASG auto-replaces failed instances within 2-3
minutes. ECS will retry launching the task. For higher reliability, add a VPC
endpoint for ECR (a separate design concern that would eliminate the dependency
on NAT for image pulls).

### 7. Cross-AZ task placement

**Risk:** ECS may place the Fargate task in either private subnet
(us-east-1a or us-east-1b). If placed in us-east-1b and fck-nat is in
us-east-1a, there will be cross-AZ data transfer charges for outbound traffic.

**Mitigation:** Cross-AZ charges are $0.01/GB per direction. At MVP traffic
levels, this cost is negligible (a few cents per month). Restricting placement
to a single AZ would reduce resilience without meaningful cost savings.

### 8. First apply before image push

**Risk:** The first `terragrunt apply` creates the ECS service before any
image exists in ECR. The service will attempt to start tasks that fail to
pull the image.

**Mitigation:** This is expected and harmless. The ECS service will keep
retrying until the image is pushed. The `deployment_circuit_breaker` prevents
infinite resource consumption. After pushing the image and forcing a new
deployment, the service will stabilize.

### 9. Terraform-managed deployments and image tag changes

**Risk:** Since deployments are fully Terraform-managed (no `lifecycle`
`ignore_changes`), every `terragrunt apply` will compare the current task
definition with the declared one. If someone deploys a new image outside of
Terraform (e.g., via `aws ecs update-service`), the next `terragrunt apply`
will revert to the image tag in `terragrunt.hcl`.

**Mitigation:** This is intentional. Terraform is the single source of truth.
All image changes must go through `terragrunt.hcl`. The deployment workflow
is: (1) push image to ECR with a new tag, (2) update `container_image` in
`terragrunt.hcl`, (3) run `terragrunt apply`.

---

## Test Plan

### Verification Checklist

Infrastructure changes are verified through plan output inspection and
post-apply validation.

#### 1. TFLint passes on the ECS module

```bash
cd infra/terraform/modules/ecs
tflint --config="$(pwd)/../../.tflint.hcl" --init
tflint --config="$(pwd)/../../.tflint.hcl"
```

**Success:** No errors or warnings.

#### 2. ECS plan creates expected resources

```bash
cd infra/terraform/live/prod/ecs
terragrunt plan
```

**Success:** Plan shows creation of:

- 1 `aws_ecr_repository` named `api`
- 1 `aws_ecr_lifecycle_policy` (keep last 10 images)
- 1 `aws_ecs_cluster` named `prod`
- 1 `aws_ecs_cluster_capacity_providers` with FARGATE and FARGATE_SPOT
- 1 `aws_ecs_task_definition` with `cpu = "256"`, `memory = "512"`,
  `runtime_platform` ARM64, container named "api"
- 1 `aws_ecs_service` with `capacity_provider_strategy` FARGATE_SPOT,
  `desired_count = 1`, `deployment_circuit_breaker` enabled
- 2 IAM roles (`api_execution_role`, `api_task_role`)
- 1 security group (`prod-api-fargate`) with all-outbound egress
- 1 RDS security group ingress rule (PostgreSQL from Fargate SG)
- 1 CloudWatch log group `/ecs/prod/api` with 14-day retention

Plan should NOT show changes to VPC, NAT, or RDS cluster resources.

#### 3. Existing infrastructure is unaffected

```bash
source scripts/src/includes.sh
tg plan prod
```

**Success:** VPC and NAT units show "No changes." RDS unit shows one
additional ingress rule on its security group (expected).

#### 4. Post-apply: ECR repository exists

```bash
aws ecr describe-repositories \
  --repository-names api \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'repositories[0].{Name:repositoryName,URI:repositoryUri}'
```

**Success:** Returns the repository name and URI.

#### 5. Post-apply: Service is running with healthy task

```bash
aws ecs describe-services \
  --cluster prod \
  --services api \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'services[0].{Desired:desiredCount,Running:runningCount,Pending:pendingCount}'
```

**Success:** Returns `{ "Desired": 1, "Running": 1, "Pending": 0 }`.

#### 6. Post-apply: Container health check is HEALTHY

```bash
TASK_ARN=$(aws ecs list-tasks \
  --cluster prod --service-name api \
  --profile tokenoverflow-prod-admin --region us-east-1 \
  --query 'taskArns[0]' --output text)

aws ecs describe-tasks \
  --cluster prod --tasks "$TASK_ARN" \
  --profile tokenoverflow-prod-admin --region us-east-1 \
  --query 'tasks[0].containers[0].healthStatus'
```

**Success:** Returns `HEALTHY`.

#### 7. Post-apply: Logs appear in CloudWatch

```bash
aws logs tail /ecs/prod/api \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --since 10m
```

**Success:** Shows application startup log lines at `info` level.

#### 8. Post-apply: Task is using Spot capacity

```bash
aws ecs describe-tasks \
  --cluster prod --tasks "$TASK_ARN" \
  --profile tokenoverflow-prod-admin --region us-east-1 \
  --query 'tasks[0].capacityProviderName'
```

**Success:** Returns `FARGATE_SPOT`.

#### 9. Post-apply: Task is running on ARM64

```bash
aws ecs describe-task-definition \
  --task-definition prod-api \
  --profile tokenoverflow-prod-admin --region us-east-1 \
  --query 'taskDefinition.runtimePlatform'
```

**Success:** Returns
`{ "cpuArchitecture": "ARM64", "operatingSystemFamily": "LINUX" }`.

#### 10. Post-apply: RDS connection is VPC-internal

```bash
# Verify RDS has no public IP
aws rds describe-db-instances \
  --db-instance-identifier main-prod \
  --profile tokenoverflow-prod-admin --region us-east-1 \
  --query 'DBInstances[0].{PubliclyAccessible:PubliclyAccessible,Endpoint:Endpoint}'
```

**Success:** `PubliclyAccessible: false`. Endpoint address is a private
hostname resolving to an IP in the 10.0.20.0/24 or 10.0.21.0/24 range.

#### 11. E2E: API health endpoint responds

Once an ALB or public access mechanism is configured, run:

```bash
TOKENOVERFLOW_ENV=production cargo test -p tokenoverflow --test e2e
```

Note: Until an ALB is deployed, this test requires ECS Exec or a bastion host
in the VPC to reach the Fargate task's private IP.

---

## Documentation Changes

### Files to Update

| File                        | Change                                                               |
|-----------------------------|----------------------------------------------------------------------|
| `infra/terraform/README.md` | Add ECS/Fargate section with architecture, cost, and deploy commands |

### Content to Add to `infra/terraform/README.md`

Add the following after the existing NAT section:

```markdown
## ECS (Fargate)

The ECS module (`modules/ecs/`) deploys the TokenOverflow API to AWS Fargate
on ARM64 (Graviton) architecture with Spot pricing. The module also manages
the ECR repository for container images.

| Setting | Value |
|---------|-------|
| CPU / Memory | 0.25 vCPU / 0.5 GB |
| Architecture | ARM64 (Graviton) |
| Capacity | FARGATE_SPOT (Spot pricing, ~70% discount) |
| Container port | 8080 |
| Health check | GET /health (30s interval, 60s start period) |
| Logs | CloudWatch `/ecs/prod/api` (14-day retention, info level) |
| Monthly cost | ~$2.17 (Spot) / ~$7.22 (On-demand) |

### Switch from Spot to On-Demand

Change `capacity_provider` in `live/prod/ecs/terragrunt.hcl`:

```hcl
capacity_provider = "FARGATE"  # was "FARGATE_SPOT"
```

### Deploy a New Image

```shell
# 1. Build and push new image
ECR_URL=$(cd infra/terraform/live/prod/ecs && terragrunt output -raw ecr_repository_url)
docker buildx build --platform linux/arm64 --tag "${ECR_URL}:v1.2.3" --push -f apps/api/Dockerfile .

# 2. Update container_image in terragrunt.hcl to the new tag
# 3. Apply
cd infra/terraform/live/prod/ecs
terragrunt apply
```

### Deploy

```shell
cd infra/terraform/live/prod/ecs
terragrunt init
terragrunt plan
terragrunt apply
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

No new local environment variables are introduced. The `TOKENOVERFLOW_ENV`
variable is only set inside the Fargate container (as `production`). The
secrets (`TOKENOVERFLOW_DATABASE_PASSWORD`, `TOKENOVERFLOW_EMBEDDING_API_KEY`)
are injected from SSM Parameter Store into the container by ECS, not by the
developer.

### Setup Flow

No changes. The `source scripts/src/includes.sh && setup` command continues
to work. No new tools or dependencies are required for local development.

The `docker compose up -d` local stack is not affected by this change.

---

## Tasks

### Task 1: Create SSM Parameter Store secrets

**What:** Store the database password and Voyage AI API key in SSM Parameter
Store as SecureString parameters.

**Steps:**

1. Store database password:

   ```bash
   aws ssm put-parameter \
     --name "/tokenoverflow/prod/database-password" \
     --type SecureString \
     --value "<database-password>" \
     --profile tokenoverflow-prod-admin \
     --region us-east-1
   ```

1. Store Voyage AI API key:

   ```bash
   aws ssm put-parameter \
     --name "/tokenoverflow/prod/embedding-api-key" \
     --type SecureString \
     --value "<voyage-api-key>" \
     --profile tokenoverflow-prod-admin \
     --region us-east-1
   ```

**Success:** Both parameters exist and can be retrieved:

```bash
aws ssm get-parameter \
  --name "/tokenoverflow/prod/database-password" \
  --with-decryption \
  --profile tokenoverflow-prod-admin \
  --region us-east-1 \
  --query 'Parameter.Value'
```

### Task 2: Create the ECS wrapper module

**What:** Create `infra/terraform/modules/ecs/` with `cluster.tf`, `api.tf`,
`ecr.tf`, `variables.tf`, and `outputs.tf`.

**Steps:**

1. `mkdir -p infra/terraform/modules/ecs`
2. Create `variables.tf` with the content from the Interfaces section
3. Create `ecr.tf` with the content from the Interfaces section
4. Create `cluster.tf` with the content from the Interfaces section
5. Create `api.tf` with the content from the Interfaces section
6. Create `outputs.tf` with the content from the Interfaces section
7. Run TFLint:

   ```bash
   cd infra/terraform/modules/ecs
   tflint --config="$(pwd)/../../.tflint.hcl" --init
   tflint --config="$(pwd)/../../.tflint.hcl"
   ```

**Success:** TFLint passes with no errors. The files follow the same patterns
as existing modules (vpc, rds, nat).

### Task 3: Create the prod Terragrunt unit

**What:** Create `infra/terraform/live/prod/ecs/terragrunt.hcl` with
prod-specific inputs and VPC/RDS dependencies.

**Steps:**

1. `mkdir -p infra/terraform/live/prod/ecs`
2. Create `terragrunt.hcl` with the content from the Interfaces section

**Success:** File exists and follows the same pattern as existing units. The
`dependency` blocks reference `../vpc` and `../rds`. Inputs include private
subnet IDs, database host, and RDS security group ID from dependencies.

### Task 4: Deploy ECS to production

**What:** Initialize, plan, and apply the ECS infrastructure, then build and
push the container image.

**Steps:**

1. Log in: `aws sso login --profile tokenoverflow-prod-admin`
2. Initialize:

   ```bash
   cd infra/terraform/live/prod/ecs
   terragrunt init
   ```

3. Plan: `terragrunt plan` -- review output against the expected resource
   list in Logic Phase 3. Confirm:
    - ECR repository named `api`
    - Cluster named `prod`
    - Task definition with ARM64, 256 CPU, 512 MiB memory
    - Container named "api" with port 8080
    - FARGATE_SPOT capacity provider
    - Health check with 30s interval, 60s startPeriod
    - Log level override `TOKENOVERFLOW__LOGGING__LEVEL=info`
    - RDS security group gains an ingress rule from the Fargate SG
    - IAM roles named `api_execution_role` and `api_task_role`
4. Apply: `terragrunt apply`
5. Build and push the ARM64 image (see Logic Phase 3, Step 3.5)
6. Force new deployment (see Logic Phase 3, Step 3.6)
7. Verify service is running (see Test Plan section 5)
8. Verify container health is HEALTHY (see Test Plan section 6)
9. Verify logs appear in CloudWatch at info level (see Test Plan section 7)
10. Verify Spot capacity provider (see Test Plan section 8)
11. Commit:

    ```bash
    git add infra/terraform/modules/ecs/
    git add infra/terraform/live/prod/ecs/
    git commit -m "infra: deploy API to ECS Fargate Spot (ARM64) with ECR"
    ```

**Success:** ECR repository exists. Service shows `RunningCount: 1`.
Container health status is `HEALTHY`. Logs appear in `/ecs/prod/api` at
`info` level. Capacity provider is `FARGATE_SPOT`.

### Task 5: Update documentation

**What:** Update `infra/terraform/README.md` with ECS/Fargate information.

**Steps:**

1. Add ECS section to `infra/terraform/README.md` (see Documentation
   Changes section)
2. Commit:

   ```bash
   git add infra/terraform/README.md
   git commit -m "docs: add ECS Fargate deployment documentation to terraform README"
   ```

**Success:** README accurately describes the ECS deployment, cost,
Spot-to-on-demand switching, and new image deployment workflow.
