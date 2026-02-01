locals {
  aws_region                  = "us-east-1"
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  env_name                    = local.env_vars.locals.env_name
  backend_bucket              = local.env_vars.locals.backend_bucket
  aws_provider_version        = "6.35.1"
  cloudflare_provider_version = "5.18.0"
}

remote_state {
  backend = "s3"
  generate = {
    path      = "backend.tf"
    if_exists = "overwrite"
  }
  config = {
    bucket       = local.backend_bucket
    key          = "${trimprefix(path_relative_to_include(), "${local.env_name}/")}/tofu.tfstate"
    region       = local.aws_region
    encrypt      = true
    use_lockfile = true
  }
}

generate "providers" {
  path      = "provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "aws" {
  region = "${local.aws_region}"
}

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "${local.aws_provider_version}"
    }
  }
}
EOF
}
