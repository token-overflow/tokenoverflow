include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/nat"
}

dependency "vpc" {
  config_path = "../vpc"
}

inputs = {
  env_name  = "prod"
  vpc_id    = dependency.vpc.outputs.vpc_id
  subnet_id = dependency.vpc.outputs.public_subnet_ids[0]

  route_tables_ids = {
    for idx, id in dependency.vpc.outputs.private_route_table_ids :
    "private-${idx}" => id
  }

  # All other inputs use module defaults:
  # instance_type = "t4g.nano"
}
