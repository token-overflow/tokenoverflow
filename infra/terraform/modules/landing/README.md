# Landing

Infrastructure for the static landing page.

## Architecture

The site lives in a private S3 bucket fronted by CloudFront. Origin Access
Control (OAC) signs every CloudFront-to-S3 request with SigV4, and the
bucket policy restricts reads to the specific distribution ARN.

The viewer-request CloudFront Function rewrites pretty URLs to
`/foo/index.html` and 301s `www.tokenoverflow.io` to the apex. Origin
403/404 responses are translated into the branded `/404.html` body with
HTTP status 404 via two `custom_error_response` blocks. A response
headers policy attaches the same CSP/HSTS/X-Frame-Options/Referrer-Policy/
Permissions-Policy/COOP/CORP/XCTO baseline the previous Cloudflare
ruleset emitted.

### Security Posture

The bucket is fully private. The bucket policy contains exactly one
`Allow` statement, scoped to the CloudFront service principal and gated
by an `AWS:SourceArn` equality check against the distribution ARN.
Direct `curl` to
`tokenoverflow-landing.s3.us-east-1.amazonaws.com/index.html` returns
`403 Forbidden`.

## Budget alarm

`aws_budgets_budget.landing` is a single COST budget with two
notifications: forecasted spend at 100% (early warning) and actual spend
at 100% (backstop). Both email `var.budget_alert_email`.

## WAF toggle

`waf.tf` authors a Web ACL with the AWS Managed Common Rule Set, the AWS
Managed Known Bad Inputs Rule Set, and a per-IP rate-based rule, plus an
association to the distribution. Every WAF resource is gated behind
`count = var.waf_enabled ? 1 : 0` with a default of `false`. Flip the
variable to `true` and `terragrunt apply` before public release.
