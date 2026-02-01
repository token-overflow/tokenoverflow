include "root" {
  path           = find_in_parent_folders("root.hcl")
  expose         = true
  merge_strategy = "deep"
}

terraform {
  source = "../../../modules/landing"
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
  }
}
EOF
}

inputs = {
  cloudflare_zone_id    = "30617bb3eecb28a8cbc132be997560f5"
  budget_alert_email    = "founder@tokenoverflow.io"
  viewer_request_source = file("${get_repo_root()}/apps/landing/src/cloudfront/viewer_request.js")
}
