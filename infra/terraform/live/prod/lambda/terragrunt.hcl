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

  database_password_ssm_name     = "/tokenoverflow/prod/database-password"
  embedding_api_key_ssm_name     = "/tokenoverflow/prod/embedding-api-key"
  auth_workos_api_key_ssm_name   = "/tokenoverflow/prod/workos_api_key"
  github_client_secret_ssm_name  = "/tokenoverflow/prod/github-client-secret"

  rds_security_group_id = dependency.rds.outputs.security_group_id

  log_retention_days = 14
}
