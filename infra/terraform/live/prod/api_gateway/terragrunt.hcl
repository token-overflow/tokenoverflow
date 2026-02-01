include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../modules/api_gateway"
}

dependency "lambda" {
  config_path = "../lambda"
}

inputs = {
  env_name             = "prod"
  lambda_invoke_arn    = dependency.lambda.outputs.invoke_arn
  lambda_function_name = dependency.lambda.outputs.function_name
  jwt_issuer           = "https://intimate-figure-17.authkit.app"
  jwt_audience         = ["client_01KKZDZQ26HJSBXSWQRSWABFMX"]
  default_rate_limit   = 500
  default_burst_limit  = 1000
  log_retention_days   = 14
}
