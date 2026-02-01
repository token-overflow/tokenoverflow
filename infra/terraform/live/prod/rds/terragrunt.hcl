include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/rds"
}

dependency "vpc" {
  config_path = "../vpc"
}

dependency "bastion" {
  config_path = "../bastion"
}

inputs = {
  env_name                   = "prod"
  vpc_id                     = dependency.vpc.outputs.vpc_id
  database_subnet_group_name = dependency.vpc.outputs.database_subnet_group_name
  private_subnet_cidrs = ["10.0.10.0/24", "10.0.11.0/24"]
  identifier                 = "main"
  db_name                    = "tokenoverflow"
  username                   = "tokenoverflow"

  bastion_security_group_id = dependency.bastion.outputs.security_group_id

  # password_wo and password_wo_version are only needed for initial creation or
  # password rotation. Provide via environment variables:
  #   TF_VAR_password_wo=<password>
  #   TF_VAR_password_wo_version=<version>
}
