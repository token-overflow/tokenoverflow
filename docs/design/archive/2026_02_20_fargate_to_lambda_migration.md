# Design: Fargate To Lambda Migration

## Architecture Overview

Migrate the TokenOverflow API from ECS Fargate to AWS Lambda behind API Gateway
REST API. Nothing is currently deployed to Fargate, so the ECS Terraform module
can be deleted entirely.

### Current Architecture (ECS Fargate)

```
Internet → (no ingress configured) → Fargate (private subnet) → RDS (database subnet)
                                                                → fck-nat → Internet (Voyage AI)
```

### Target Architecture (Lambda + API Gateway)

```
Client
  │
  │  x-api-key header
  ▼
API Gateway REST API
  │  ├─ API key validation
  │  ├─ Usage plan enforcement (rate limits, quotas)
  │  └─ Lambda proxy integration (AWS_PROXY)
  ▼
Lambda (private subnets, ARM64, 512 MB)
  │  ├─ Axum router (same routes as today)
  │  ├─ MCP server (/mcp)
  │  └─ lambda_http adapter
  │
  ├──→ RDS (database subnet, SG-to-SG ingress on 5432)
  └──→ fck-nat (public subnet) → Internet (Voyage AI API)
```

### Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Deployment format | ZIP via `cargo lambda build` | Static linking of `pq-sys` (bundled) + `openssl-sys` (vendored) produces a self-contained binary. No Docker image needed for Lambda. |
| Build tool | `cargo-lambda` | Cross-compiles from macOS to `aarch64-unknown-linux-gnu` via Zig toolchain. Produces Lambda-ready ZIP with `bootstrap` binary. |
| Static linking | `pq-sys` bundled + `openssl-sys` vendored | `pq-sys` bundled compiles libpq from source. libpq depends on OpenSSL, so `openssl-sys` vendored provides it. `reqwest` 0.13 already uses `rustls` (no OpenSSL needed for HTTP). |
| Endpoint | API Gateway REST API (v1) | Required for API keys, usage plans (tiering), and per-client rate limiting. HTTP API (v2) does not support these features. |
| Lambda ↔ local dev | Runtime detection (`AWS_LAMBDA_RUNTIME_API` env var) | One binary for both. Branch in `async_run()`: Lambda mode calls `lambda_http::run(app)`, local mode binds TCP listener. |
| Memory | 512 MB | Matches current Fargate allocation. |
| Architecture | ARM64 (Graviton) | 13-24% faster cold starts, 30% cheaper than x86_64. |
| MCP | Works as-is | Tools are request-response. `LocalSessionManager` loses sessions on cold start — clients reconnect per MCP spec. |

### Network Flow

Lambda is placed in the existing **private subnets**:

1. **RDS access:** A new Lambda security group is created. An ingress rule is
   added to the RDS security group allowing TCP 5432 from the Lambda SG
   (security group referencing, same pattern as the current Fargate SG rule).

2. **Public internet via fck-nat:** The fck-nat module already routes
   `0.0.0.0/0` through the NAT instance on private subnet route tables. Lambda
   in those subnets inherits this route. Needed for Voyage AI API calls.

### Estimated Cold Start Breakdown

| Phase | Time |
|---|---|
| Lambda init + binary load | ~10-20ms |
| Rust binary init | ~16ms |
| Config load + logging init | ~1ms |
| DB pool creation + first connection | ~50-100ms |
| TagResolver load (2 DB queries) | ~50-100ms |
| VoyageClient + reqwest init | ~10ms |
| **Total cold start** | **~90-250ms** |

Warm invocations: ~5-50ms (state reused, DB connection alive).
ZIP packages have faster init than container images (~50ms advantage).

## Interfaces

### Client → API Gateway

All API and MCP traffic flows through API Gateway REST API. Clients must include
an API key in every request via the `x-api-key` header. API Gateway validates
the key and enforces usage plan limits before forwarding to Lambda.

```
POST https://api.tokenoverflow.io/mcp
x-api-key: abc123def456
Content-Type: application/json

{"jsonrpc":"2.0","method":"tools/call","params":{...},"id":1}
```

**Exception:** The `/health` endpoint does not require an API key. It is exposed
as a dedicated API Gateway resource with `api_key_required = false` for external
monitoring.

### API Gateway → Lambda

Uses **Lambda proxy integration** (`AWS_PROXY`). API Gateway forwards the full
HTTP request (method, path, headers, query params, body) as a Lambda event.
`lambda_http` deserializes this into a standard `http::Request` that Axum
processes identically to a direct TCP request.

Two API Gateway resources handle routing:

| Resource | Method | API Key Required | Purpose |
|---|---|---|---|
| `/health` | `GET` | No | Health check endpoint |
| `/{proxy+}` | `ANY` | Yes | All other routes (API + MCP) |

### Usage Plan

API Gateway usage plans enforce per-client rate limits and quotas. A single
**free** tier ships at launch. The goal is protection against abuse (runaway
agents, scraping) — not restricting normal use.

**Usage math for the free tier:**

A vibe coder using Claude Code with the TokenOverflow MCP plugin triggers
searches automatically in the background. Per the product brief, frequency is
"extremely high (10-100+ times per coding session)." Each problem encounter
produces ~2-4 MCP requests (search, read details, submit/upvote). A heavy
coding session (100 problems) generates ~300 requests. Two sessions per day =
~600 requests at the extreme end. The free tier should comfortably accommodate
this without the developer ever noticing rate limits.

| Tier | Rate Limit | Burst | Daily Quota | Rationale |
|---|---|---|---|---|
| Free | 10 req/s | 20 | 1,000 | Covers the heaviest individual daily use (~600 req). No agent sustains 10 req/s. Ceiling naturally identifies power users/teams for future paid plans. |

Higher tiers (Pro, Enterprise) are out of scope for now. The `usage_plans`
variable is a map, so adding tiers later is a one-line Terragrunt change.

API keys are created and associated with the free plan via Terraform or the AWS
CLI. Future work: self-service API key generation via a signup endpoint.

### Request Tracing (`X-Trace-Id`)

Every API Gateway request gets a unique `$context.requestId`. This ID is used
to correlate API Gateway access logs with Lambda application logs.

**Flow:**

```
Client request
  → API Gateway (logs requestId to CloudWatch access log)
  → Lambda event (requestContext.requestId included automatically by AWS_PROXY)
  → Rust middleware extracts requestId, injects into tracing span
  → Response includes X-Trace-Id header
```

**API Gateway access log format** (JSON, includes `requestId`):

```json
{
  "requestId": "$context.requestId",
  "ip": "$context.identity.sourceIp",
  "method": "$context.httpMethod",
  "path": "$context.path",
  "status": "$context.status",
  "latency": "$context.responseLatency",
  "apiKeyId": "$context.identity.apiKeyId"
}
```

**Rust middleware** extracts `requestId` from the Lambda request context (not
from a header — `AWS_PROXY` integration does not allow injecting custom request
headers, but the value is in the event's `requestContext`). The middleware:

1. In Lambda mode: reads `requestContext.requestId` from `lambda_http` request
   extensions
2. In local mode: generates a UUID
3. Adds `trace_id` field to the current tracing span
4. Adds `X-Trace-Id` response header

**Correlation:** Search API Gateway logs for `requestId`, then search Lambda
logs for the same value in the `trace_id` field. Zero cost (no X-Ray).

### Lambda Environment Variables

The Lambda function receives secrets and the environment selector through
environment variables. All other configuration lives in `production.toml`,
which is auto-bundled into the ZIP via `[package.metadata.lambda.build]`.

| Variable | Source | Example |
|---|---|---|
| `TOKENOVERFLOW_ENV` | Terraform variable | `production` |
| `TOKENOVERFLOW_DATABASE_PASSWORD` | SSM Parameter Store | (resolved at deploy time) |
| `TOKENOVERFLOW_EMBEDDING_API_KEY` | SSM Parameter Store | (resolved at deploy time) |

**Config files:** The `config/` directory is auto-bundled into the Lambda ZIP
by `cargo lambda build` (configured via `[package.metadata.lambda.build]` in
`apps/api/Cargo.toml`). The Lambda working directory is `/var/task`, so the
default `TOKENOVERFLOW_CONFIG_DIR` value of `config` resolves to
`/var/task/config/`.

### Lambda → Downstream (unchanged)

| Target | Protocol | Path |
|---|---|---|
| RDS | PostgreSQL TCP 5432 | Lambda SG → RDS SG (VPC internal) |
| Voyage AI | HTTPS 443 | Lambda → fck-nat → Internet |

## Logic

### 1. Rust Code Changes

#### 1a. Add Dependencies (`apps/api/Cargo.toml`)

```toml
[dependencies]
# Lambda HTTP adapter — converts API Gateway events to http::Request
lambda_http = "0.14"

# Static linking for Lambda: compile libpq and OpenSSL from source.
# pq-sys bundled: compiles libpq from C source (no system libpq needed).
# openssl-sys vendored: compiles OpenSSL from source (libpq depends on it).
# reqwest 0.13 uses rustls — OpenSSL is only needed for libpq.
pq-sys = { version = "0.7", features = ["bundled"] }
openssl-sys = { version = "0.9", features = ["vendored"] }

# UUID generation for trace IDs in local (non-Lambda) mode
uuid = { version = "1", features = ["v4"] }
```

#### 1b. Add Release Profile (`Cargo.toml` workspace root)

```toml
[profile.release]
opt-level = "s"       # Balanced speed/size
lto = "thin"          # Good optimization without excessive link time
strip = "symbols"     # Remove debug symbols
panic = "abort"       # No unwinding code
codegen-units = 1     # Single codegen unit for better optimization
```

#### 1c. Modify Server Bootstrap (`apps/api/src/api/server.rs`)

Add runtime detection branch in `async_run()`. After building the Axum Router
(unchanged), branch on the `AWS_LAMBDA_RUNTIME_API` environment variable:

```rust
// In async_run(), after building `app`:

if std::env::var("AWS_LAMBDA_RUNTIME_API").is_ok() {
    info!("Running in AWS Lambda mode");
    lambda_http::run(app).await.map_err(|e| e.into())
} else {
    let bind_addr = format!("{}:{}", config.api.host, config.api.port);
    info!("Starting server on {}", bind_addr);
    let listener = TcpListener::bind(&bind_addr).await?;
    serve_until_shutdown(listener, app, shutdown_signal()).await
}
```

Everything before the branch (Config::load, create_app_state, MCP service,
router, middleware) stays identical. `lambda_http::run(app)` accepts any
`tower::Service<http::Request<Body>>`, which `axum::Router` implements.

**No changes to:** `main.rs`, `config.rs`, `state.rs`, `mcp/`, `db/pool.rs`,
routes, or any other file.

#### 1d. Add Trace ID Middleware (`apps/api/src/api/middleware.rs`, new file)

Axum middleware that extracts the API Gateway request ID and wraps the request
in a tracing span. Every `info!()`, `warn!()`, `error!()` call during the
request automatically includes `trace_id` in its JSON output — no changes to
existing log calls anywhere in the codebase.

```rust
use axum::{extract::Request, middleware::Next, response::Response};
use http::HeaderValue;
use tracing::Instrument;

pub async fn trace_id(req: Request, next: Next) -> Response {
    let id = extract_trace_id(&req);
    let span = tracing::info_span!("request", trace_id = %id);

    async move {
        let mut response = next.run(req).await;
        if let Ok(val) = HeaderValue::from_str(&id) {
            response.headers_mut().insert("X-Trace-Id", val);
        }
        response
    }
    .instrument(span)
    .await
}

fn extract_trace_id(req: &Request) -> String {
    // In Lambda mode: extract requestContext.requestId from lambda_http
    // extensions. In local mode: generate a UUID.
    req.extensions()
        .get::<lambda_http::request::RequestContext>()
        .map(|ctx| match ctx {
            lambda_http::request::RequestContext::ApiGatewayV1(v1) => {
                v1.request_id.clone()
            }
            _ => uuid::Uuid::new_v4().to_string(),
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
```

**How it works:** `tracing::info_span!("request", trace_id = %id)` creates a
span with the `trace_id` field. `.instrument(span)` runs the request handler
inside that span. Every log line emitted during the request (in handlers,
services, repositories — anywhere) automatically includes `trace_id` in the
JSON output:

```json
{"timestamp":"2026-02-21T...","level":"INFO","trace_id":"abc-123","message":"Search returned 3 results"}
```

Wire the middleware into the router in `server.rs`:

```rust
use axum::middleware;

let app = routes::configure()
    .nest_service("/mcp", mcp_service)
    .with_state(app_state)
    .layer(middleware::from_fn(middleware::trace_id))
    .layer(/* existing timeout layers */);
```

**New dependency:** `uuid = { version = "1", features = ["v4"] }` in
`apps/api/Cargo.toml`.

### 2. Build Process

#### 2a. Build Command

```bash
cargo lambda build -p tokenoverflow --release --arm64 --output-format zip
```

This cross-compiles from macOS to `aarch64-unknown-linux-gnu` using the Zig
toolchain (cargo-lambda default). The `pq-sys` bundled and `openssl-sys`
vendored features compile libpq and OpenSSL from C source during the build,
producing a fully self-contained `bootstrap` binary.

Output: `target/lambda/tokenoverflow/bootstrap.zip`

**Fallback:** If the Zig toolchain fails to compile the C code for libpq or
OpenSSL, use the `cross` compiler which builds inside a Linux Docker container:

```bash
cargo lambda build -p tokenoverflow --release --arm64 --output-format zip \
  --compiler cross
```

#### 2b. Deploy

```bash
# Compute SHA for the ZIP (used as S3 key for versioning)
SHA=$(shasum -a 256 target/lambda/tokenoverflow/bootstrap.zip | cut -c1-12)

# Upload ZIP to S3
aws s3 cp target/lambda/tokenoverflow/bootstrap.zip \
  s3://tokenoverflow-lambda-prod/api/${SHA}.zip

# Update Lambda function code
aws lambda update-function-code \
  --function-name api \
  --s3-bucket tokenoverflow-lambda-prod \
  --s3-key "api/${SHA}.zip" \
  --architectures arm64
```

The SHA-based key provides immutable artifact versioning. Rollback is a
one-liner pointing back to a previous SHA.

### 3. Terraform: Lambda Module (`modules/lambda/`)

#### 3a. `iam.tf` — Execution Role

- `aws_iam_role` with `lambda.amazonaws.com` assume-role policy
- `AWSLambdaBasicExecutionRole` attachment (CloudWatch Logs)
- `AWSLambdaVPCAccessExecutionRole` attachment (ENI management)
- Inline policy for `ssm:GetParameters` on the two secret ARNs

#### 3b. `function.tf` — Lambda Function

```hcl
resource "aws_lambda_function" "api" {
  function_name = "tokenoverflow-api-${var.env_name}"
  role          = aws_iam_role.lambda.arn

  s3_bucket = aws_s3_bucket.deployments.id
  s3_key    = var.lambda_s3_key  # "api/{SHA}.zip", set by CI/CD

  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["arm64"]
  memory_size   = var.memory_size   # 512
  timeout       = var.timeout       # 30

  vpc_config {
    subnet_ids         = var.private_subnet_ids
    security_group_ids = [aws_security_group.lambda.id]
  }

  environment {
    variables = {
      TOKENOVERFLOW_ENV              = var.tokenoverflow_env
      TOKENOVERFLOW_DATABASE_PASSWORD = data.aws_ssm_parameter.db_password.value
      TOKENOVERFLOW_EMBEDDING_API_KEY = data.aws_ssm_parameter.embedding_key.value
    }
  }

  lifecycle {
    ignore_changes = [s3_key, s3_object_version, source_code_hash]
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
```

`lifecycle.ignore_changes` ensures Terraform does not revert code deployments
made by CI/CD.

#### 3c. `s3.tf` — Deployment Bucket

```hcl
resource "aws_s3_bucket" "deployments" {
  bucket = "tokenoverflow-lambda-${var.env_name}"

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_s3_bucket_versioning" "deployments" {
  bucket = aws_s3_bucket.deployments.id
  versioning_configuration {
    status = "Enabled"
  }
}
```

S3 versioning is enabled as a safety net. The primary versioning mechanism is
the SHA-based key path (`api/{SHA}.zip`).

#### 3d. `security_groups.tf`

Same pattern as the current ECS module:

- `aws_security_group` for Lambda
- `aws_vpc_security_group_egress_rule`: all outbound (`0.0.0.0/0`)
- `aws_vpc_security_group_ingress_rule` on the **RDS** security group: TCP 5432
  from Lambda SG (security group referencing)

#### 3e. `cloudwatch.tf`

```hcl
resource "aws_cloudwatch_log_group" "api" {
  name              = "/aws/lambda/tokenoverflow-api-${var.env_name}"
  retention_in_days = var.log_retention_days
}
```

#### 3f. Outputs

| Output | Value |
|---|---|
| `function_name` | Lambda function name |
| `function_arn` | Lambda function ARN |
| `invoke_arn` | Lambda invoke ARN (for API Gateway integration) |
| `security_group_id` | Lambda security group ID |
| `log_group_name` | CloudWatch log group name |
| `role_arn` | Lambda execution role ARN |
| `s3_bucket_name` | Deployment S3 bucket name |

### 4. Terraform: API Gateway Module (`modules/api_gateway/`)

#### 4a. `api.tf` — REST API + Lambda Integration

```hcl
resource "aws_api_gateway_rest_api" "main" {
  name = "main_api"

  endpoint_configuration {
    types = ["REGIONAL"]
  }
}

# --- /health (no API key) ---

resource "aws_api_gateway_resource" "health" {
  rest_api_id = aws_api_gateway_rest_api.main.id
  parent_id   = aws_api_gateway_rest_api.main.root_resource_id
  path_part   = "health"
}

resource "aws_api_gateway_method" "health_get" {
  rest_api_id      = aws_api_gateway_rest_api.main.id
  resource_id      = aws_api_gateway_resource.health.id
  http_method      = "GET"
  authorization    = "NONE"
  api_key_required = false
}

resource "aws_api_gateway_integration" "health_get" {
  rest_api_id             = aws_api_gateway_rest_api.main.id
  resource_id             = aws_api_gateway_resource.health.id
  http_method             = aws_api_gateway_method.health_get.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = var.lambda_invoke_arn
}

# --- /{proxy+} (API key required) ---

resource "aws_api_gateway_resource" "proxy" {
  rest_api_id = aws_api_gateway_rest_api.main.id
  parent_id   = aws_api_gateway_rest_api.main.root_resource_id
  path_part   = "{proxy+}"
}

resource "aws_api_gateway_method" "proxy_any" {
  rest_api_id      = aws_api_gateway_rest_api.main.id
  resource_id      = aws_api_gateway_resource.proxy.id
  http_method      = "ANY"
  authorization    = "NONE"
  api_key_required = true
}

resource "aws_api_gateway_integration" "proxy_any" {
  rest_api_id             = aws_api_gateway_rest_api.main.id
  resource_id             = aws_api_gateway_resource.proxy.id
  http_method             = aws_api_gateway_method.proxy_any.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = var.lambda_invoke_arn
}
```

#### 4b. `stage.tf` — Deployment + Stage

```hcl
resource "aws_api_gateway_deployment" "main" {
  rest_api_id = aws_api_gateway_rest_api.main.id

  triggers = {
    redeployment = sha1(jsonencode([
      aws_api_gateway_resource.health.id,
      aws_api_gateway_resource.proxy.id,
      aws_api_gateway_method.health_get.id,
      aws_api_gateway_method.proxy_any.id,
      aws_api_gateway_integration.health_get.id,
      aws_api_gateway_integration.proxy_any.id,
    ]))
  }

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_api_gateway_stage" "prod" {
  deployment_id = aws_api_gateway_deployment.main.id
  rest_api_id   = aws_api_gateway_rest_api.main.id
  stage_name    = var.env_name

  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.api_gateway.arn
    format = jsonencode({
      requestId     = "$context.requestId"
      ip            = "$context.identity.sourceIp"
      method        = "$context.httpMethod"
      path          = "$context.path"
      status        = "$context.status"
      latency       = "$context.responseLatency"
      apiKeyId      = "$context.identity.apiKeyId"
    })
  }
}

resource "aws_cloudwatch_log_group" "api_gateway" {
  name              = "/aws/apigateway/main_api-${var.env_name}"
  retention_in_days = var.log_retention_days
}

# IAM role for API Gateway to write CloudWatch logs
resource "aws_api_gateway_account" "main" {
  cloudwatch_role_arn = aws_iam_role.api_gateway_cloudwatch.arn
}

resource "aws_iam_role" "api_gateway_cloudwatch" {
  name = "api-gateway-cloudwatch-${var.env_name}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "apigateway.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "api_gateway_cloudwatch" {
  role       = aws_iam_role.api_gateway_cloudwatch.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonAPIGatewayPushToCloudWatchLogs"
}
```

The `requestId` field in the access log is the same value that Lambda receives
in `requestContext.requestId`. This is the correlation key between API Gateway
and Lambda logs.

#### 4c. `usage_plans.tf` — Tiering + API Keys

```hcl
variable "usage_plans" {
  type = map(object({
    description  = string
    rate_limit   = number
    burst_limit  = number
    quota_limit  = number
    quota_period = string
  }))
}

resource "aws_api_gateway_usage_plan" "tiers" {
  for_each    = var.usage_plans
  name        = "${each.key}-${var.env_name}"
  description = each.value.description

  api_stages {
    api_id = aws_api_gateway_rest_api.main.id
    stage  = aws_api_gateway_stage.prod.stage_name
  }

  throttle_settings {
    rate_limit  = each.value.rate_limit
    burst_limit = each.value.burst_limit
  }

  quota_settings {
    limit  = each.value.quota_limit
    period = each.value.quota_period
  }
}
```

API keys are created and associated with plans via Terraform or AWS CLI. The
module does not hardcode any API keys — that is an operational concern.

#### 4d. `permissions.tf` — Allow API Gateway to Invoke Lambda

```hcl
resource "aws_lambda_permission" "apigw" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = var.lambda_function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_api_gateway_rest_api.main.execution_arn}/*/*"
}
```

#### 4e. Variables

| Variable | Type | Description |
|---|---|---|
| `env_name` | `string` | Environment name (e.g., `prod`) |
| `lambda_invoke_arn` | `string` | Lambda invoke ARN |
| `lambda_function_name` | `string` | Lambda function name |
| `usage_plans` | `map(object)` | Usage plan tier configurations |
| `log_retention_days` | `number` | CloudWatch log retention (default: 14) |

#### 4f. Outputs

| Output | Value |
|---|---|
| `rest_api_id` | REST API ID |
| `stage_invoke_url` | Stage invoke URL (e.g., `https://abc123.execute-api.us-east-1.amazonaws.com/prod`) |
| `rest_api_execution_arn` | Execution ARN for IAM policies |
| `access_log_group_name` | API Gateway access log CloudWatch log group |

### 5. Terragrunt Live Configs

#### 5a. `live/prod/lambda/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/lambda"
}

dependency "vpc" {
  config_path = "../vpc"
}

dependency "rds" {
  config_path = "../rds"
}

inputs = {
  env_name           = "prod"
  vpc_id             = dependency.vpc.outputs.vpc_id
  private_subnet_ids = dependency.vpc.outputs.private_subnet_ids

  memory_size = 512
  timeout     = 30

  tokenoverflow_env = "production"

  database_password_ssm_name = "/tokenoverflow/prod/database-password"
  embedding_api_key_ssm_name = "/tokenoverflow/prod/embedding-api-key"

  rds_security_group_id = dependency.rds.outputs.security_group_id

  log_retention_days = 14
}
```

#### 5b. `live/prod/api_gateway/terragrunt.hcl`

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/api_gateway"
}

dependency "lambda" {
  config_path = "../lambda"
}

inputs = {
  env_name             = "prod"
  lambda_invoke_arn    = dependency.lambda.outputs.invoke_arn
  lambda_function_name = dependency.lambda.outputs.function_name
  log_retention_days   = 14

  usage_plans = {
    free = {
      description  = "Free tier: 10 req/s, 1000 req/day"
      rate_limit   = 10
      burst_limit  = 20
      quota_limit  = 1000
      quota_period = "DAY"
    }
  }
}
```

### 6. Delete ECS Module

Delete entirely:
- `infra/terraform/modules/ecs/` (all files)
- `infra/terraform/live/prod/ecs/` (terragrunt.hcl + lock file + cache)

## Edge Cases & Constraints

### Lambda Execution Model

- **One request per instance.** Each Lambda instance processes a single request
  at a time. Concurrent requests spin up additional instances. This means DB
  pool size should be small (1-2 connections per instance) to avoid exhausting
  RDS connection limits under high concurrency. The existing `bb8` pool config
  should be reviewed and a `max_size` override added via environment variable if
  needed (e.g., `TOKENOVERFLOW__DATABASE__POOL_SIZE=2`).

- **Cold start with VPC.** Lambda in a VPC requires ENI attachment on cold
  start. AWS pre-provisions ENIs via Hyperplane, so this typically adds <1s. The
  private subnets must have sufficient free IPs for Lambda scaling.

- **Warm instance reuse.** Between invocations on the same instance, the entire
  Rust process persists (DB pool, tag cache, reqwest client). This is the
  desired behavior — `lambda_http::run()` keeps the process alive and reuses
  state across invocations.

### Payload & Timeout Limits

| Limit | Value | Impact |
|---|---|---|
| API Gateway request body | 10 MB | Not a concern — MCP payloads are small JSON |
| Lambda response payload (sync) | 6 MB | Not a concern — API responses are small |
| API Gateway integration timeout | 29 seconds | Must be ≤ Lambda timeout. Our 30s Lambda timeout leaves 1s margin |
| Lambda timeout | 30 seconds | Matches current `request_timeout_secs` config |

### Secrets Management

SSM parameters are resolved at Terraform plan/apply time via
`data "aws_ssm_parameter"` and injected as Lambda environment variables. This
means:

- Secret rotation requires a Terraform apply (or `aws lambda update-function-configuration`).
- Secrets are visible in the Lambda console environment variables section
  (encrypted at rest by AWS, decrypted at runtime).
- This matches the current ECS pattern where secrets are resolved by the ECS
  agent at task startup.

### cargo-lambda Build Risks

- **Zig compiler + C code:** The default Zig toolchain may fail compiling
  libpq or OpenSSL from source (Zig uses `-nostdinc` which excludes system
  headers). If this happens, fall back to `--compiler cross` which builds inside
  a Linux Docker container. Document both paths.

- **pq-sys bundled version:** The `bundled` feature was added in pq-sys 0.5.0.
  Our dependency resolves to 0.7.x which supports it. Pin the version in
  `Cargo.toml` to avoid surprises.

### MCP Session State

`LocalSessionManager` stores sessions in memory. On Lambda cold start, all
sessions are lost. MCP clients are expected to re-initialize when a session
becomes invalid — this is standard MCP behavior. No code changes needed.

### API Gateway `{proxy+}` Routing

The `{proxy+}` resource does **not** match the root path `/`. A request to
`https://api.tokenoverflow.io/prod/` would hit the root resource, not
`{proxy+}`. Since we only define `/health` and `/{proxy+}`, any request to `/`
without a path returns 403 (missing API key) or API Gateway's default response.
This is acceptable — there is no root endpoint in the API.

## Test Plan

### Existing Tests (must remain green)

| Tier | Command | Expected Outcome |
|---|---|---|
| Unit | `cargo test -p tokenoverflow --test unit` | All pass. No changes to tested code. |
| Integration | `cargo test -p tokenoverflow --test integration` | All pass. Testcontainers unaffected. |
| E2E (local) | `docker compose up -d --build api && cargo test -p tokenoverflow --test e2e` | All pass. Local Docker uses the TCP listener path (no `AWS_LAMBDA_RUNTIME_API`). |

### New Verification Steps

| Step | Command | Success Criteria |
|---|---|---|
| Build ZIP | `cargo lambda build -p tokenoverflow --release --arm64 --output-format zip` | Exits 0. Produces `target/lambda/tokenoverflow/bootstrap.zip`. |
| Binary is ARM64 | `file target/lambda/tokenoverflow/bootstrap` | Output contains `aarch64` and `ELF 64-bit`. |
| Binary is statically linked (libpq) | `ldd target/lambda/tokenoverflow/bootstrap` | No `libpq.so` in output (or `not a dynamic executable` on musl). |
| Config bundled in ZIP | `unzip -l target/lambda/tokenoverflow/bootstrap.zip` | Lists `bootstrap` and `config/` directory (auto-bundled). |
| Terraform plan | `cd infra/terraform/live/prod/lambda && terragrunt plan` | No errors. Creates expected resources. |
| Terraform plan (API GW) | `cd infra/terraform/live/prod/api_gateway && terragrunt plan` | No errors. Creates REST API, usage plan, Lambda permission. |
| Lambda health check | `curl https://{api-gw-url}/prod/health` | Returns `{"status":"ok","database":"connected"}` |
| Lambda MCP (with API key) | `curl -X POST https://{api-gw-url}/prod/mcp -H "x-api-key: {key}" -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"initialize",...}'` | Returns MCP initialize response |
| API key enforcement | `curl https://{api-gw-url}/prod/mcp` (no key) | Returns 403 Forbidden |
| E2E against Lambda | `TOKENOVERFLOW_ENV=production cargo test -p tokenoverflow --test e2e` | All pass against the deployed Lambda |

## Documentation Changes

### README.md Updates

1. **Architecture diagram:** Replace Fargate references with Lambda + API
   Gateway in the architecture section.

2. **Deployment section (new):** Add instructions for building and deploying to
   Lambda:

   ```text
   ## Deployment

   ### Build for Lambda
   cargo lambda build -p tokenoverflow --release --arm64 --output-format zip

   ### Bundle config and deploy
   # (see deploy script)
   ```

3. **Infrastructure section:** Update the Terragrunt dependency chain:

   ```text
   vpc → nat → rds → lambda → api_gateway
   ```

4. **Local development:** Add note that local development is unchanged —
   `docker compose up -d` still works. Lambda mode is only activated when
   `AWS_LAMBDA_RUNTIME_API` is set.

## Development Environment Changes

### New Tool: `cargo-lambda`

Add `cargo-lambda` to the project setup. Install via Homebrew:

```bash
brew install cargo-lambda
```

This should be added to the `setup` function in `scripts/src/includes.sh`
(alongside existing tool checks).

### No Other Changes

- Local development workflow is **unchanged**: `docker compose up -d` runs the
  API with the TCP listener path.
- `TOKENOVERFLOW_ENV=local` and `TOKENOVERFLOW_CONFIG_DIR=apps/api/config`
  continue to work as before (set via `.cargo/config.toml`).
- Unit and integration tests run identically — the Lambda branch is never
  triggered locally.

## Tasks

### Task 1: Add Lambda Dependencies and Release Profile

**Scope:** Modify `apps/api/Cargo.toml` and workspace `Cargo.toml`.

**Requirements:**
- Add `lambda_http = "0.14"` to `[dependencies]` in `apps/api/Cargo.toml`
- Add `pq-sys = { version = "0.7", features = ["bundled"] }` to
  `[dependencies]`
- Add `openssl-sys = { version = "0.9", features = ["vendored"] }` to
  `[dependencies]`
- Add `uuid = { version = "1", features = ["v4"] }` to `[dependencies]`
- Add `[profile.release]` to workspace `Cargo.toml` with: `opt-level = "s"`,
  `lto = "thin"`, `strip = "symbols"`, `panic = "abort"`, `codegen-units = 1`

**Success criteria:** `cargo check -p tokenoverflow` succeeds.

---

### Task 2: Add Lambda Runtime Branch and Trace ID Middleware

**Scope:** Modify `apps/api/src/api/server.rs`, create
`apps/api/src/api/middleware.rs`.

**Requirements:**
- Create `apps/api/src/api/middleware.rs` with the `trace_id` middleware:
    - In Lambda mode: extract `requestContext.requestId` from `lambda_http`
      request extensions
    - In local mode: generate a UUID
    - Add `trace_id` field to current tracing span
    - Add `X-Trace-Id` response header
- In `server.rs`, wire the trace ID middleware into the router layer stack
- In `async_run()`, after building the Axum `app`, add a branch:
    - If `AWS_LAMBDA_RUNTIME_API` env var is set → `lambda_http::run(app).await`
    - Otherwise → existing TCP listener + `serve_until_shutdown` path
- Do NOT change: `main.rs`, `config.rs`, `state.rs`, `mcp/`, routes, or any
  other file
- Everything before the branch (config load, app state creation, MCP service,
  router, middleware) must remain identical

**Success criteria:**
- `cargo test -p tokenoverflow --test unit` passes
- `cargo test -p tokenoverflow --test integration` passes
- `docker compose up -d --build api && cargo test -p tokenoverflow --test e2e`
  passes (local TCP path still works, trace ID middleware generates UUIDs)

---

### Task 3: Verify cargo-lambda Build

**Scope:** Build the Lambda ZIP and verify the binary.

**Requirements:**
- Run `cargo lambda build -p tokenoverflow --release --arm64 --output-format zip`
- If Zig compiler fails on C code (pq-sys/openssl-sys), retry with
  `--compiler cross`
- Verify binary is ARM64 ELF
- Verify no dynamic `libpq.so` dependency
- Verify ZIP contents: `bootstrap` + `config/` directory (auto-bundled via
  `[package.metadata.lambda.build]`)

**Success criteria:** `bootstrap.zip` exists with correct contents and the
binary is `aarch64` ELF with no `libpq.so` dependency.

---

### Task 4: Create Lambda Terraform Module

**Scope:** Create `infra/terraform/modules/lambda/` with all files.

**Requirements:**
- `iam.tf`: Execution role with `lambda.amazonaws.com` assume-role,
  `AWSLambdaBasicExecutionRole`, `AWSLambdaVPCAccessExecutionRole`, SSM read
  policy
- `function.tf`: `aws_lambda_function` with `provided.al2023` runtime, ARM64,
  S3 source (`api/{SHA}.zip`), VPC config, environment variables, lifecycle
  ignore on `s3_key`/`s3_object_version`/`source_code_hash`
- `s3.tf`: S3 bucket (`tokenoverflow-lambda-{env}`) with versioning
- `security_groups.tf`: Lambda SG with all-outbound egress, RDS SG ingress
  rule for TCP 5432 from Lambda SG
- `cloudwatch.tf`: Log group `/aws/lambda/tokenoverflow-api-{env}`
- `variables.tf`: All input variables
- `outputs.tf`: function_name, function_arn, invoke_arn, security_group_id,
  log_group_name, role_arn, s3_bucket_name
- Follow existing module conventions: tags with `Environment` + `ManagedBy`,
  use `data "aws_ssm_parameter"` for secrets

**Success criteria:** `terragrunt plan` in `live/prod/lambda/` succeeds with
no errors.

---

### Task 5: Create API Gateway Terraform Module

**Scope:** Create `infra/terraform/modules/api_gateway/` with all files.

**Requirements:**
- `api.tf`: REST API named `main_api` (REGIONAL), `/health` resource (GET, no
  API key), `/{proxy+}` resource (ANY, API key required), Lambda proxy
  integrations for both
- `stage.tf`: Deployment with trigger hash, stage with access logging enabled
  (JSON format including `$context.requestId`, `sourceIp`, `httpMethod`, `path`,
  `status`, `responseLatency`, `apiKeyId`), CloudWatch log group for access
  logs, IAM role for API Gateway to push to CloudWatch
- `usage_plans.tf`: `for_each` over `var.usage_plans` map, throttle + quota
  settings
- `permissions.tf`: `aws_lambda_permission` allowing API Gateway to invoke
  Lambda
- `variables.tf`: env_name, lambda_invoke_arn, lambda_function_name,
  usage_plans, log_retention_days
- `outputs.tf`: rest_api_id, stage_invoke_url, rest_api_execution_arn,
  access_log_group_name

**Success criteria:** `terragrunt plan` in `live/prod/api_gateway/` succeeds
with no errors.

---

### Task 6: Create Terragrunt Live Configs

**Scope:** Create terragrunt.hcl files for Lambda and API Gateway.

**Requirements:**
- `infra/terraform/live/prod/lambda/terragrunt.hcl`: Dependencies on VPC and
  RDS, all inputs as specified in the Logic section
- `infra/terraform/live/prod/api_gateway/terragrunt.hcl`: Dependency on Lambda,
  free usage plan with rate_limit=10, burst_limit=20, quota_limit=1000,
  quota_period=DAY

**Success criteria:** Both `terragrunt plan` commands succeed.

---

### Task 7: Delete ECS Module

**Scope:** Remove all ECS Terraform files.

**Requirements:**
- Delete `infra/terraform/modules/ecs/` entirely (api.tf, cluster.tf, ecr.tf,
  variables.tf, outputs.tf)
- Delete `infra/terraform/live/prod/ecs/` entirely (terragrunt.hcl, lock file,
  cache)

**Success criteria:** No ECS files remain. Other modules' `terragrunt plan`
still succeeds (no broken dependencies — nothing depends on ECS outputs).

---

### Task 8: Update Documentation and Dev Environment

**Scope:** Update README.md and setup scripts.

**Requirements:**
- Update README.md architecture section: replace Fargate with Lambda + API
  Gateway
- Add deployment instructions to README.md
- Update infrastructure dependency chain in README.md
- Add `cargo-lambda` install to `scripts/src/includes.sh` setup function
- Add note that local development is unchanged

**Success criteria:** README.md accurately reflects the new architecture.
`source scripts/src/includes.sh && setup` installs cargo-lambda.

## Deployment Runbook

### 1. Destroy Fargate (nothing deployed, just Terraform state)

```bash
# Destroy ECS resources (cluster, service, task def, SGs, IAM roles, ECR, logs)
cd infra/terraform/live/prod/ecs
terragrunt destroy -auto-approve

# Verify clean destruction
terragrunt state list  # Should return empty
```

### 2. Build the Lambda ZIP

```bash
# Cross-compile for ARM64 Linux with static linking
# config/ is auto-bundled into the ZIP via [package.metadata.lambda.build]
cargo lambda build -p tokenoverflow --release --arm64 --output-format zip

# If Zig fails on C code (pq-sys/openssl), fall back to Docker-based cross:
# cargo lambda build -p tokenoverflow --release --arm64 --output-format zip --compiler cross

# Verify
file target/lambda/tokenoverflow/bootstrap   # Must say: ELF 64-bit, aarch64
unzip -l target/lambda/tokenoverflow/bootstrap.zip  # Must list: bootstrap, config/
```

### 3. Deploy Lambda infrastructure

```bash
# Create Lambda module (S3 bucket, IAM role, SG, function, CloudWatch)
cd infra/terraform/live/prod/lambda
terragrunt apply

# Note: first apply will fail on the Lambda function because no ZIP exists in
# S3 yet. Upload the ZIP first, then re-apply:
SHA=$(shasum -a 256 target/lambda/tokenoverflow/bootstrap.zip | cut -c1-12)
aws s3 cp target/lambda/tokenoverflow/bootstrap.zip \
  s3://tokenoverflow-lambda-prod/api/${SHA}.zip \
  --profile tokenoverflow-prod-admin

# Re-apply with the S3 key
terragrunt apply -var="lambda_s3_key=api/${SHA}.zip"
```

### 4. Deploy API Gateway infrastructure

```bash
# Create API Gateway (REST API, stage, usage plan, logging, Lambda permission)
cd infra/terraform/live/prod/api_gateway
terragrunt apply
```

### 5. Create an API key and associate with free plan

```bash
# Create API key
aws apigateway create-api-key \
  --name "dogfood-key" \
  --enabled \
  --profile tokenoverflow-prod-admin

# Get the usage plan ID
PLAN_ID=$(aws apigateway get-usage-plans \
  --profile tokenoverflow-prod-admin \
  --query "items[?name=='free-prod'].id" --output text)

# Get the API key ID from the create-api-key output
KEY_ID=<from above output>

# Associate key with free plan
aws apigateway create-usage-plan-key \
  --usage-plan-id ${PLAN_ID} \
  --key-id ${KEY_ID} \
  --key-type "API_KEY" \
  --profile tokenoverflow-prod-admin
```

### 6. Verify

```bash
# Get the API Gateway URL
cd infra/terraform/live/prod/api_gateway
API_URL=$(terragrunt output -raw stage_invoke_url)

# Health check (no API key needed)
curl ${API_URL}/health
# Expected: {"status":"ok","database":"connected"}

# MCP endpoint (API key required)
API_KEY=<value from step 5>
curl -X POST ${API_URL}/mcp \
  -H "x-api-key: ${API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}'
# Expected: MCP initialize response with server capabilities

# Verify API key enforcement
curl ${API_URL}/mcp
# Expected: 403 Forbidden

# Verify trace ID in response
curl -I -X POST ${API_URL}/mcp \
  -H "x-api-key: ${API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{}'
# Expected: X-Trace-Id header in response

# Run E2E tests against production
TOKENOVERFLOW_ENV=production cargo test -p tokenoverflow --test e2e
```

### 7. Clean up ECS files from repo

```bash
# Delete ECS module and live config (already destroyed in step 1)
rm -rf infra/terraform/modules/ecs/
rm -rf infra/terraform/live/prod/ecs/
```

### Rollback

If something goes wrong after deployment:

```bash
# Rollback Lambda code to a previous SHA
aws lambda update-function-code \
  --function-name api \
  --s3-bucket tokenoverflow-lambda-prod \
  --s3-key "api/<PREVIOUS_SHA>.zip" \
  --architectures arm64 \
  --profile tokenoverflow-prod-admin

# Nuclear option: destroy everything and re-deploy Fargate
cd infra/terraform/live/prod/api_gateway && terragrunt destroy -auto-approve
cd infra/terraform/live/prod/lambda && terragrunt destroy -auto-approve
# Then git revert the code changes and re-deploy ECS
```
