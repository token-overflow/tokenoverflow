# Design: Cloudflare Custom Domain

## Architecture Overview

### Goal

Route `api.tokenoverflow.io` from Cloudflare DNS to the existing AWS API Gateway
(REST, REGIONAL) in the prod account, with DDoS protection, fully managed via
OpenTofu/Terragrunt.

### Scope

This design covers:

- ACM certificate for `api.tokenoverflow.io` with DNS validation via Cloudflare
- API Gateway custom domain name and base path mapping
- Cloudflare DNS records (ACM validation + CNAME to API Gateway)
- Cloudflare proxy mode for DDoS protection
- New `dns` Terraform module and Terragrunt unit
- Cloudflare Terraform provider integration
- New `stage_name` output on the existing `api_gateway` module

This design does NOT cover:

- Dev environment subdomain (`api.dev.tokenoverflow.io`) — deferred
- Cloudflare WAF rules beyond default DDoS protection
- Cloudflare page rules or cache rules
- Zone creation (already exists from domain purchase)

### DDoS Protection: Cloudflare Proxy vs AWS Alternatives

The user requires DDoS protection at low or zero cost. Three viable options
exist:

| Criteria | Cloudflare Proxy (Free) | AWS Shield Standard (Free) | AWS CloudFront + WAF |
|---|---|---|---|
| L3/L4 DDoS | Yes | Yes | Yes (via Shield Standard) |
| L7 DDoS | Yes | No | Only with WAF rules ($) |
| WAF | Basic managed rules (free) | Not included | $5/mo per ACL + $1/mo per rule + $0.60/M requests |
| Origin IP hiding | Yes | No | Yes (via CloudFront) |
| Monthly cost | $0 | $0 | ~$7-15/mo minimum |
| Setup complexity | Low (DNS CNAME + proxy toggle) | Zero (auto-enabled) | High (CloudFront distribution + origin config + WAF ACL) |
| Latency impact | +5-20ms (double TLS) | None | +5-15ms (CloudFront hop) |
| CORS risk | Moderate (header manipulation) | None | Low |

**Full-cost AWS alternative**: AWS Shield Advanced provides L3-L7 DDoS
protection comparable to Cloudflare but costs **$3,000/month** — clearly out of
scope for a startup.

#### Decision: Cloudflare Proxy

Cloudflare proxy on the free plan provides L3-L7 DDoS protection, basic WAF, and
origin IP hiding at zero cost. AWS Shield Standard (automatically enabled) only
covers L3/L4. Achieving equivalent L7 protection on AWS would require
CloudFront + WAF at ~$7-15/month minimum, with significantly more
infrastructure complexity.

The trade-off is a small latency increase (~5-20ms from double TLS termination)
and moderate CORS risk from header manipulation. For an API consumed by AI agents
(not browsers), CORS is not a concern. The latency overhead is acceptable given
the free L7 DDoS protection.

**SSL mode**: Full (Strict) — Cloudflare terminates TLS at its edge and
re-encrypts to the API Gateway origin using a validated ACM certificate. This
provides true end-to-end encryption.

### Resource Dependency Chain

```text
aws_acm_certificate (per domain)
    |
    v
cloudflare_dns_record (validation CNAMEs, proxied=false)
    |
    v
aws_acm_certificate_validation
    |
    v
aws_api_gateway_domain_name (REGIONAL, TLS 1.2)
    |
    v
aws_api_gateway_base_path_mapping (stage = prod)
    |
    v
cloudflare_dns_record (CNAME: api.tokenoverflow.io -> regional_domain_name, proxied=true)
```

### Module Structure

**New module**: `infra/terraform/modules/dns`
**New unit**: `infra/terraform/live/prod/dns/terragrunt.hcl`

A single `dns` module manages all domains in one Terragrunt unit. Domains are
passed as a map, and resources use `for_each` to iterate over them. Adding a new
domain is a matter of adding an entry to the map in the unit's inputs.

The module manages both AWS resources (ACM, API Gateway custom domain) and
Cloudflare resources (DNS records) because they form a tight dependency chain
that cannot be split across separate Terraform states without circular
dependencies or manual orchestration.

The Cloudflare provider is generated only in this unit's `terragrunt.hcl` via a
`generate` block, keeping it isolated from all other units.

### Alternatives Considered: Module Structure

| Option | Description | Pros | Cons |
|---|---|---|---|
| A: One unit per domain | Reusable module instantiated as separate Terragrunt units per domain | Blast radius isolation per domain, simple flat variables | More files, more state files to manage |
| **B: Single unit, all domains** | One module takes a map of domains, one Terragrunt unit | Simple to operate, one place for all DNS, fewer files | Shared blast radius, all backend dependencies in one unit |

**Decision: Option B** — For the current scale (one domain, growing to a
handful), operational simplicity wins. A single unit with a domain map is easier
to manage than N separate units. If the number of domains grows significantly or
blast radius becomes a concern, this can be refactored into per-domain units
without changing the module itself.

```text
infra/terraform/
  modules/dns/
    acm.tf              # ACM certificate + validation
    api_gateway.tf      # Custom domain name + base path mapping
    cloudflare.tf       # DNS records (validation + CNAME)
    variables.tf
    outputs.tf
  live/prod/
    dns/
      terragrunt.hcl    # Single unit for all domains
```

## Interfaces

### Module Inputs (`modules/dns/variables.tf`)

```hcl
variable "cloudflare_zone_id" {
  description = "Cloudflare zone ID for tokenoverflow.io"
  type        = string
}

variable "domains" {
  description = "Map of domains to configure"
  type = map(object({
    domain_name = string
    proxied     = optional(bool, true)
    backend = object({
      type        = string                # "api_gateway"
      rest_api_id = optional(string)
      stage_name  = optional(string)
    })
  }))
}
```

For the current scope, only `type = "api_gateway"` is supported. Future backend
types (e.g., `cloudfront`, `alb`) can be added by extending the `backend` object
and adding conditional resource blocks.

### Module Outputs (`modules/dns/outputs.tf`)

```hcl
output "domain_names" {
  description = "Map of domain key to custom domain name"
  value       = { for k, v in var.domains : k => v.domain_name }
}

output "acm_certificate_arns" {
  description = "Map of domain key to ACM certificate ARN"
  value       = { for k, v in aws_acm_certificate.main : k => v.arn }
}
```

### Terragrunt Unit (`live/prod/dns/terragrunt.hcl`)

```hcl
include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/dns"
}

dependency "api_gateway" {
  config_path = "../api_gateway"
}

generate "cloudflare_provider" {
  path      = "cloudflare_provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "cloudflare" {}

terraform {
  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "5.18.0"
    }
  }
}
EOF
}

inputs = {
  cloudflare_zone_id = "REPLACE_WITH_ZONE_ID"

  domains = {
    api = {
      domain_name = "api.tokenoverflow.io"
      proxied     = true
      backend = {
        type        = "api_gateway"
        rest_api_id = dependency.api_gateway.outputs.rest_api_id
        stage_name  = dependency.api_gateway.outputs.stage_name
      }
    }
  }
}
```

### Existing Module Change: `api_gateway` outputs

Add a `stage_name` output to `modules/api_gateway/outputs.tf`:

```hcl
output "stage_name" {
  description = "API Gateway stage name"
  value       = aws_api_gateway_stage.prod.stage_name
}
```

### Cloudflare API Token

Authentication uses the `CLOUDFLARE_API_TOKEN` environment variable (provider
auto-reads it). This follows the Cloudflare provider convention and does not need
the `TOKENOVERFLOW_` prefix since it is a third-party tool convention.

| Context | How the token is provided |
|---|---|
| Local development | Developer exports `CLOUDFLARE_API_TOKEN` in shell or via `direnv` |
| GitHub Actions | Stored as repository secret `CLOUDFLARE_API_TOKEN`, injected as env var in the Terraform workflow |

The token requires **Zone:DNS:Edit** permission scoped to the `tokenoverflow.io`
zone.

### GHA Workflow Change

Add `CLOUDFLARE_API_TOKEN` as an environment variable to the `plan-prod` and
`apply-prod` jobs in `.github/workflows/terraform.yml`:

```yaml
env:
  CLOUDFLARE_API_TOKEN: ${{ secrets.CLOUDFLARE_API_TOKEN }}
```

## Logic

### `modules/dns/acm.tf`

```hcl
resource "aws_acm_certificate" "main" {
  for_each          = var.domains
  domain_name       = each.value.domain_name
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_acm_certificate_validation" "main" {
  for_each        = var.domains
  certificate_arn = aws_acm_certificate.main[each.key].arn
  validation_record_fqdns = [
    for dvo in aws_acm_certificate.main[each.key].domain_validation_options :
    dvo.resource_record_name
  ]

  depends_on = [cloudflare_dns_record.acm_validation]
}
```

### `modules/dns/cloudflare.tf`

```hcl
locals {
  # Flatten domain_validation_options across all domains into a map
  # keyed by "domain_key.dvo_domain" for for_each
  acm_validations = merge([
    for dk, cert in aws_acm_certificate.main : {
      for dvo in cert.domain_validation_options :
      "${dk}.${dvo.domain_name}" => {
        domain_key   = dk
        record_name  = trimsuffix(dvo.resource_record_name, ".")
        record_value = trimsuffix(dvo.resource_record_value, ".")
        record_type  = dvo.resource_record_type
      }
    }
  ]...)
}

# ACM DNS validation records (must NOT be proxied)
resource "cloudflare_dns_record" "acm_validation" {
  for_each = local.acm_validations
  zone_id  = var.cloudflare_zone_id
  name     = each.value.record_name
  type     = each.value.record_type
  content  = each.value.record_value
  proxied  = false
  comment  = "ACM validation for ${each.value.domain_key}"
}

# CNAME records pointing domains to their backends
resource "cloudflare_dns_record" "cname" {
  for_each = var.domains
  zone_id  = var.cloudflare_zone_id
  name     = each.value.domain_name
  type     = "CNAME"
  content  = aws_api_gateway_domain_name.main[each.key].regional_domain_name
  proxied  = each.value.proxied
  comment  = "Routes to ${each.value.backend.type} backend"
}
```

The `trimsuffix` calls on validation records are critical: AWS returns FQDNs with
trailing dots (e.g., `_abc.example.com.`) but Cloudflare normalizes them without
dots. Without `trimsuffix`, Terraform detects drift on every plan.

### `modules/dns/api_gateway.tf`

```hcl
locals {
  api_gateway_domains = {
    for k, v in var.domains : k => v if v.backend.type == "api_gateway"
  }
}

resource "aws_api_gateway_domain_name" "main" {
  for_each    = local.api_gateway_domains
  domain_name = each.value.domain_name

  regional_certificate_arn = aws_acm_certificate_validation.main[each.key].certificate_arn

  endpoint_configuration {
    types = ["REGIONAL"]
  }

  security_policy = "TLS_1_2"
}

resource "aws_api_gateway_base_path_mapping" "main" {
  for_each    = local.api_gateway_domains
  api_id      = each.value.backend.rest_api_id
  stage_name  = each.value.backend.stage_name
  domain_name = aws_api_gateway_domain_name.main[each.key].domain_name
}
```

## Edge Cases & Constraints

### ACM Certificate Region

For REGIONAL API Gateway endpoints, the ACM certificate **must** be in the same
region as the API Gateway. Since the API Gateway is in `us-east-1` and the AWS
provider in `root.hcl` is already configured for `us-east-1`, no cross-region
provider alias is needed.

### ACM Validation Timing

ACM DNS validation can take 1-30 minutes after the Cloudflare DNS records are
created. The `aws_acm_certificate_validation` resource will block until the
certificate reaches `ISSUED` state. On first apply, expect the Terraform run to
pause for several minutes during this step.

### ACM Certificate Renewal

ACM automatically renews DNS-validated certificates as long as the validation
CNAME records remain in place. Since Terraform manages these records, renewal is
automatic — do not remove the validation records after initial setup.

### Cloudflare Zone ID

The `cloudflare_zone_id` is hardcoded in the Terragrunt inputs rather than
looked up via a data source. This is intentional: the `cloudflare_zone` data
source in provider v5 has known filter bugs (GitHub issues #4958, #5347). Zone
IDs are stable and never change for a given domain.

To find the zone ID: Cloudflare dashboard > tokenoverflow.io > Overview >
right sidebar > "Zone ID".

### Cloudflare SSL Mode

The Cloudflare zone's SSL mode must be set to **Full (Strict)** for
`api.tokenoverflow.io`. This is a zone-level setting in the Cloudflare dashboard.
It is NOT managed by this Terraform module because:

1. The `cloudflare_zone_settings_override` resource in provider v5 has been
   deprecated in favor of per-setting resources.
2. SSL mode is a zone-wide default that affects all subdomains. Managing it in
   Terraform risks accidentally changing settings for other subdomains.
3. It is a one-time manual configuration.

**Manual step**: Set SSL/TLS encryption mode to "Full (Strict)" in the
Cloudflare dashboard before first apply.

### Provider Version Pinning

The Cloudflare provider is pinned to `5.18.0` (latest stable as of March 2026)
in the Terragrunt `generate` block, matching the pattern used for the AWS
provider in `root.hcl`.

## Test Plan

### Pre-apply Verification

1. Run `terragrunt plan` in `live/prod/dns/` and verify:
   - 6 resources to create (1 ACM cert, 1 ACM validation, 1 APIGW domain name,
     1 APIGW base path mapping, 1 Cloudflare validation record, 1 Cloudflare
     CNAME)
   - No changes to existing `api_gateway` state (only new `stage_name` output)

### Post-apply Verification

1. **ACM certificate status**:

   ```bash
   aws acm list-certificates --query \
     "CertificateSummaryList[?DomainName=='api.tokenoverflow.io'].Status"
   # Expected: "ISSUED"
   ```

2. **DNS resolution**:

   ```bash
   dig api.tokenoverflow.io CNAME +short
   # When proxied: returns Cloudflare IPs (not the API Gateway domain)

   dig api.tokenoverflow.io +short
   # Returns Cloudflare anycast IPs (e.g., 104.x.x.x, 172.x.x.x)
   ```

3. **API health check**:

   ```bash
   curl https://api.tokenoverflow.io/health
   # Expected: {"status":"ok","database":"connected"}
   ```

4. **TLS certificate chain**:

   ```bash
   echo | openssl s_client -connect api.tokenoverflow.io:443 -servername \
     api.tokenoverflow.io 2>/dev/null | openssl x509 -noout -issuer
   # Expected: Cloudflare Inc (since proxy terminates TLS)
   ```

5. **E2E tests** against the custom domain:

   ```bash
   TOKENOVERFLOW_ENV=production cargo test -p tokenoverflow --test e2e
   ```

   The E2E test base URL should work with the new custom domain once DNS
   propagates. No test code changes needed if the production config already
   points to `api.tokenoverflow.io` (or is updated to do so).

## Documentation Changes

Update `README.md` Architecture section to note the custom domain:

- Add `api.tokenoverflow.io` as the production API endpoint
- Note Cloudflare proxy provides DDoS protection

Update `.act.secrets.example` to include `CLOUDFLARE_API_TOKEN` placeholder.

## Development Environment Changes

None. The Cloudflare API token is only needed for Terraform operations (not local
application development). Developers who run Terraform locally need to export
`CLOUDFLARE_API_TOKEN`.

## Tasks

### Task 1: Add `stage_name` output to `api_gateway` module

**Files**: `infra/terraform/modules/api_gateway/outputs.tf`

Add:

```hcl
output "stage_name" {
  description = "API Gateway stage name"
  value       = aws_api_gateway_stage.prod.stage_name
}
```

**Success criteria**: `terragrunt plan` in `live/prod/api_gateway/` shows only
the new output, no resource changes.

### Task 2: Create `dns` module

**Files**:

- `infra/terraform/modules/dns/variables.tf`
- `infra/terraform/modules/dns/acm.tf`
- `infra/terraform/modules/dns/api_gateway.tf`
- `infra/terraform/modules/dns/cloudflare.tf`
- `infra/terraform/modules/dns/outputs.tf`

Implement the module as specified in the Logic section.

**Success criteria**: `terraform validate` passes in the module directory.

### Task 3: Create Terragrunt unit for prod DNS

**Files**: `infra/terraform/live/prod/dns/terragrunt.hcl`

Create the unit with:

- Cloudflare provider `generate` block (v5.18.0)
- Dependency on `api_gateway`
- Domain map with `api.tokenoverflow.io` entry
- Cloudflare zone ID (to be filled in from dashboard)

**Success criteria**: `terragrunt validate` passes, `terragrunt plan` shows 6
resources to create.

### Task 4: Update GHA workflow

**Files**: `.github/workflows/terraform.yml`

Add `CLOUDFLARE_API_TOKEN: ${{ secrets.CLOUDFLARE_API_TOKEN }}` to the `env`
block of `plan-prod` and `apply-prod` jobs.

**Success criteria**: Workflow YAML is valid (test with `act` locally, the
Cloudflare steps will be skipped since the secret is not present in local runs).

### Task 5: Update documentation

**Files**: `README.md`, `.act.secrets.example`

- Add custom domain info to README
- Add `CLOUDFLARE_API_TOKEN` placeholder to secrets example

### Task 6: Manual configuration (not Terraform)

1. Set Cloudflare SSL/TLS mode to "Full (Strict)" in the dashboard
2. Create Cloudflare API token with Zone:DNS:Edit permission
3. Add `CLOUDFLARE_API_TOKEN` to GitHub Actions repository secrets
4. Find and fill in the Cloudflare zone ID in the Terragrunt unit
