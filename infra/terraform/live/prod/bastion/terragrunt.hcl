include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/bastion"
}

dependency "vpc" {
  config_path = "../vpc"
}

dependency "nat" {
  config_path = "../nat"
}

inputs = {
  env_name  = "prod"
  vpc_id    = dependency.vpc.outputs.vpc_id
  subnet_id = dependency.vpc.outputs.private_subnet_ids[0]

  # ssh_public_key is only needed for initial creation or key rotation.
  # Provide via environment variable:
  #   TF_VAR_ssh_public_key="$(cat ~/.ssh/bastion.pub)"
}
