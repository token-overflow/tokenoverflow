include "root" {
  path           = find_in_parent_folders("root.hcl")
  expose         = true
  merge_strategy = "deep"
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

# Override the root `provider.tf` so the API Lambda module gets the
# `archive` provider in addition to AWS. The archive provider is required
# by the placeholder zip pipeline in `modules/lambda/function.tf`.
generate "providers" {
  path      = "provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "aws" {
  region = "us-east-1"
}

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "${include.root.locals.aws_provider_version}"
    }
    archive = {
      source  = "hashicorp/archive"
      version = "${include.root.locals.archive_provider_version}"
    }
  }
}
EOF
}

inputs = {
  env_name           = "prod"
  vpc_id             = dependency.vpc.outputs.vpc_id
  private_subnet_ids = dependency.vpc.outputs.private_subnet_ids

  memory_size = 512
  timeout     = 30

  tokenoverflow_env = "production"

  database_password_ssm_name    = "/tokenoverflow/prod/database-password"
  embedding_api_key_ssm_name    = "/tokenoverflow/prod/embedding-api-key"
  auth_workos_api_key_ssm_name  = "/tokenoverflow/prod/workos_api_key"
  github_client_secret_ssm_name = "/tokenoverflow/prod/github-client-secret"

  rds_security_group_id = dependency.rds.outputs.security_group_id

  log_retention_days = 14
}
