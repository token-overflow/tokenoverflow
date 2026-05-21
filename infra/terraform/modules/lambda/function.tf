data "aws_ssm_parameter" "db_password" {
  name = var.database_password_ssm_name
}

data "aws_ssm_parameter" "embedding_key" {
  name = var.embedding_api_key_ssm_name
}

data "aws_ssm_parameter" "auth_workos_api_key" {
  name = var.auth_workos_api_key_ssm_name
}

data "aws_ssm_parameter" "github_client_secret" {
  name = var.github_client_secret_ssm_name
}

# Generate a tiny placeholder zip at plan time. Terraform owns the initial
# upload to S3 so `CreateFunction` has a real object to point at on a fresh
# environment. CI ships the real artifact to a SHA-suffixed key
# (`api/{sha}.zip`) via `aws lambda update-function-code` and never touches
# this `initial.zip` key after that. The `lifecycle.ignore_changes` on
# `aws_s3_object` keeps Terraform from re-uploading on subsequent plans
# even if the placeholder content drifts from the source file's output.
#
# This is the canonical pattern documented by `terraform-aws-modules/
# terraform-aws-lambda` for "infra and code in separate pipelines"; see
# https://github.com/terraform-aws-modules/terraform-aws-lambda#readme.
#
# The bootstrap script is a real file (mode 0755 in git) so the zip
# preserves the executable bit the `provided.al2023` runtime requires.
data "archive_file" "placeholder" {
  type        = "zip"
  output_path = "${path.module}/.placeholder.zip"
  source_file = "${path.module}/files/bootstrap"
}

resource "aws_s3_object" "placeholder" {
  bucket = aws_s3_bucket.deployments.id
  key    = var.lambda_s3_key

  source = data.archive_file.placeholder.output_path
  etag   = data.archive_file.placeholder.output_md5

  lifecycle {
    ignore_changes = [source, etag]
  }
}

resource "aws_lambda_function" "api" {
  function_name = "api"
  role          = aws_iam_role.lambda.arn

  s3_bucket = aws_s3_object.placeholder.bucket
  s3_key    = aws_s3_object.placeholder.key

  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["arm64"]
  memory_size   = var.memory_size
  timeout       = var.timeout

  vpc_config {
    subnet_ids         = var.private_subnet_ids
    security_group_ids = [aws_security_group.lambda.id]
  }

  environment {
    variables = {
      TOKENOVERFLOW_ENV                  = var.tokenoverflow_env
      TOKENOVERFLOW_DATABASE_PASSWORD    = data.aws_ssm_parameter.db_password.value
      TOKENOVERFLOW_EMBEDDING_API_KEY    = data.aws_ssm_parameter.embedding_key.value
      TOKENOVERFLOW_WORKOS_API_KEY       = data.aws_ssm_parameter.auth_workos_api_key.value
      TOKENOVERFLOW_GITHUB_CLIENT_SECRET = data.aws_ssm_parameter.github_client_secret.value
    }
  }

  lifecycle {
    # CI ships SHA-suffixed keys via `aws lambda update-function-code`;
    # ignore the drift here so subsequent terraform applies don't revert
    # the function's code pointer back to the placeholder. `last_modified`
    # is a computed-only attribute (no configured value to diff against),
    # so OpenTofu rejects it in `ignore_changes`.
    ignore_changes = [s3_key, s3_object_version, source_code_hash]
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
