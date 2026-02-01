variable "env_name" {
  description = "Environment name (e.g., prod)"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID where Lambda will be deployed"
  type        = string
}

variable "private_subnet_ids" {
  description = "Private subnet IDs for Lambda VPC config"
  type        = list(string)
}

variable "lambda_s3_key" {
  description = "S3 key for the Lambda deployment ZIP (e.g., api/{SHA}.zip)"
  type        = string
  default     = "api/initial.zip"
}

variable "memory_size" {
  description = "Lambda memory in MB"
  type        = number
  default     = 512
}

variable "timeout" {
  description = "Lambda timeout in seconds"
  type        = number
  default     = 30
}

variable "tokenoverflow_env" {
  description = "TOKENOVERFLOW_ENV value (e.g., production)"
  type        = string
  default     = "production"
}

variable "database_password_ssm_name" {
  description = "SSM parameter name for the database password"
  type        = string
}

variable "embedding_api_key_ssm_name" {
  description = "SSM parameter name for the embedding API key"
  type        = string
}

variable "rds_security_group_id" {
  description = "RDS security group ID (for ingress rule)"
  type        = string
}

variable "auth_workos_api_key_ssm_name" {
  description = "SSM parameter name for the WorkOS API key"
  type        = string
}

variable "github_client_secret_ssm_name" {
  description = "SSM parameter name for the GitHub OAuth App client secret"
  type        = string
}

variable "log_retention_days" {
  description = "CloudWatch log retention in days"
  type        = number
  default     = 14
}
