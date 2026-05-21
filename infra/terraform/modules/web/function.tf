data "aws_ssm_parameter" "web_oauth_client_secret" {
  name = var.web_oauth_client_secret_ssm_name
}

data "aws_ssm_parameter" "cookie_signing_key" {
  name = var.cookie_signing_key_ssm_name
}

# Generate a tiny placeholder zip at plan time. Terraform owns the initial
# upload to S3 so `CreateFunction` has a real object to point at on a fresh
# environment. CI ships the real artifact to a SHA-suffixed key
# (`web/{sha}.zip`) via `aws lambda update-function-code` and never touches
# this `initial.zip` key after that. The `lifecycle.ignore_changes` on
# `aws_s3_object` keeps Terraform from re-uploading on subsequent plans
# even if the placeholder content drifts from the data source's output.
#
# This is the canonical pattern documented by `terraform-aws-modules/
# terraform-aws-lambda` for "infra and code in separate pipelines"; see
# https://github.com/terraform-aws-modules/terraform-aws-lambda#readme.
data "archive_file" "placeholder" {
  type        = "zip"
  output_path = "${path.module}/.placeholder.zip"

  source {
    filename = "index.mjs"
    content  = <<-EOT
      export const handler = async () => ({
        statusCode: 503,
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ error: "BFF not yet deployed" }),
      });
    EOT
  }
}

resource "aws_s3_object" "placeholder" {
  bucket = var.lambda_s3_bucket
  key    = var.lambda_s3_key

  source = data.archive_file.placeholder.output_path
  etag   = data.archive_file.placeholder.output_md5

  lifecycle {
    ignore_changes = [source, etag]
  }
}

resource "aws_lambda_function" "web" {
  function_name = "web"
  role          = aws_iam_role.web.arn

  s3_bucket = aws_s3_object.placeholder.bucket
  s3_key    = aws_s3_object.placeholder.key

  handler       = "index.handler"
  runtime       = "nodejs22.x"
  architectures = ["arm64"]
  memory_size   = var.memory_size
  timeout       = var.timeout

  vpc_config {
    subnet_ids         = var.private_subnet_ids
    security_group_ids = [aws_security_group.web.id]
  }

  environment {
    variables = {
      TOKENOVERFLOW_ENV                       = var.tokenoverflow_env
      TOKENOVERFLOW_WEB_AUTHKIT_CLIENT_SECRET = data.aws_ssm_parameter.web_oauth_client_secret.value
      TOKENOVERFLOW_WEB_COOKIE_SIGNING_KEY    = data.aws_ssm_parameter.cookie_signing_key.value
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
