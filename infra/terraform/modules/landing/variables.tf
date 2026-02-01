variable "cloudflare_zone_id" {
  description = "Cloudflare zone ID for tokenoverflow.io"
  type        = string
}

variable "domain_apex" {
  description = "Apex domain served by the landing"
  type        = string
  default     = "tokenoverflow.io"
}

variable "www_domain" {
  description = "www domain that 301s to the apex"
  type        = string
  default     = "www.tokenoverflow.io"
}

variable "bucket_name" {
  description = "S3 bucket name (globally unique, must contain no dots)"
  type        = string
  default     = "tokenoverflow-landing"
}

variable "monthly_budget_limit_usd" {
  description = "Monthly cost ceiling above which the operator is emailed. Tune after observing real spend."
  type        = number
  default     = 50
}

variable "budget_alert_email" {
  description = "Email subscribed to the AWS Budget notifications."
  type        = string
}

variable "waf_enabled" {
  description = "If true, attach an AWS WAF Web ACL to the distribution. Costs roughly $8/month when enabled."
  type        = bool
  default     = false
}

variable "waf_rate_limit_per_5min" {
  description = "Per-IP request rate limit applied by the WAF rate-based rule. Only used when waf_enabled = true."
  type        = number
  default     = 2000
}

variable "viewer_request_source" {
  description = "JavaScript source for the CloudFront Function (cloudfront-js-2.0). Read by Terragrunt from apps/landing/src/cloudfront/viewer_request.js and passed in as a string so the module does not need a path relative to the Terragrunt cache directory."
  type        = string
}
