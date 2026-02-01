include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/oidc"
}

inputs = {
  github_repo = "token-overflow/tokenoverflow"
  env_name    = "global"
}
