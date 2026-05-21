include "root" {
  path           = find_in_parent_folders("root.hcl")
  expose         = true
  merge_strategy = "deep"
}

terraform {
  source = "../../../modules/web"
}

dependency "vpc" {
  config_path = "../vpc"
}

dependency "lambda" {
  config_path = "../lambda"
}

generate "providers" {
  path      = "provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "aws" {
  region = "us-east-1"
}

provider "cloudflare" {}

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "${include.root.locals.aws_provider_version}"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "${include.root.locals.cloudflare_provider_version}"
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

  lambda_s3_bucket = dependency.lambda.outputs.s3_bucket_name

  memory_size = 512
  timeout     = 15

  tokenoverflow_env = "production"

  web_oauth_client_secret_ssm_name = "/tokenoverflow/prod/web-oauth-client-secret"
  cookie_signing_key_ssm_name      = "/tokenoverflow/prod/web-cookie-signing-key"

  log_retention_days = 14

  cloudflare_zone_id = "30617bb3eecb28a8cbc132be997560f5"
  web_domain         = "app.tokenoverflow.io"
}
