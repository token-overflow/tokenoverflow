variable "env_name" {
  description = "Environment name (e.g., prod)"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID where the BFF Lambda will be deployed"
  type        = string
}

variable "private_subnet_ids" {
  description = "Private subnet IDs for the BFF Lambda VPC config"
  type        = list(string)
}

variable "lambda_s3_bucket" {
  description = "Deployment S3 bucket name (shared with the API Lambda module)"
  type        = string
}

variable "lambda_s3_key" {
  description = "S3 key for the BFF Lambda zip (e.g., web/{SHA}.zip)"
  type        = string
  default     = "web/initial.zip"
}

variable "memory_size" {
  description = "BFF Lambda memory in MB"
  type        = number
  default     = 512
}

variable "timeout" {
  description = "BFF Lambda timeout in seconds. The API call from the BFF has a 10s ceiling, plus AuthKit token exchange and overhead."
  type        = number
  default     = 15
}

variable "tokenoverflow_env" {
  description = "TOKENOVERFLOW_ENV value (e.g., production). Set to production so the local AuthKit bypass branch is unreachable."
  type        = string
  default     = "production"
}

variable "web_oauth_client_secret_ssm_name" {
  description = "SSM parameter name holding the WorkOS confidential client secret for the `TokenOverflow Web` Connect App (BFF only)"
  type        = string
}

variable "cookie_signing_key_ssm_name" {
  description = "SSM parameter name holding the BFF state cookie HMAC key"
  type        = string
}

variable "log_retention_days" {
  description = "CloudWatch log retention in days"
  type        = number
  default     = 14
}

variable "cloudflare_zone_id" {
  description = "Cloudflare zone ID hosting the BFF custom domain"
  type        = string
}

variable "web_domain" {
  description = "Public hostname the BFF is served on (e.g., app.tokenoverflow.io)"
  type        = string
  default     = "app.tokenoverflow.io"
}
