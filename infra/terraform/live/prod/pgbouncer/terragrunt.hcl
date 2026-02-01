include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/pgbouncer"
}

dependency "vpc" {
  config_path = "../vpc"
}

dependency "rds" {
  config_path = "../rds"
}

dependency "lambda" {
  config_path = "../lambda"
}

dependency "bastion" {
  config_path = "../bastion"
}

inputs = {
  env_name  = "prod"
  vpc_id    = dependency.vpc.outputs.vpc_id
  subnet_id = dependency.vpc.outputs.private_subnet_ids[0]

  rds_endpoint          = dependency.rds.outputs.db_instance_address
  rds_security_group_id = dependency.rds.outputs.security_group_id

  lambda_security_group_id  = dependency.lambda.outputs.security_group_id
  bastion_security_group_id = dependency.bastion.outputs.security_group_id

  database_name              = "tokenoverflow"
  database_user              = "tokenoverflow"
  database_password_ssm_name = "/tokenoverflow/prod/database-password"

  # Fixed private IP for the dedicated ENI. Must be within 10.0.10.0/24
  # and not conflict with other resources. Using .200 to avoid DHCP range.
  eni_private_ip = "10.0.10.200"
}
