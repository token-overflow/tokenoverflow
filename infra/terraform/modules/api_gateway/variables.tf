variable "env_name" {
  description = "Environment name"
  type        = string
}

variable "lambda_invoke_arn" {
  description = "Lambda invoke ARN"
  type        = string
}

variable "lambda_function_name" {
  description = "Lambda function name for invoke permission"
  type        = string
}

variable "jwt_issuer" {
  description = "JWT issuer URL (WorkOS)"
  type        = string
}

variable "jwt_audience" {
  description = "JWT audience (list of allowed audiences)"
  type        = list(string)
}

variable "default_rate_limit" {
  description = "Default stage-level rate limit (requests/second)"
  type        = number
  default     = 500
}

variable "default_burst_limit" {
  description = "Default stage-level burst limit"
  type        = number
  default     = 1000
}

variable "log_retention_days" {
  description = "CloudWatch log retention in days"
  type        = number
  default     = 14
}
