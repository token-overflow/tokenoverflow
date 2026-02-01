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

resource "aws_lambda_function" "api" {
  function_name = "api"
  role          = aws_iam_role.lambda.arn

  s3_bucket = aws_s3_bucket.deployments.id
  s3_key    = var.lambda_s3_key

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
      TOKENOVERFLOW_ENV               = var.tokenoverflow_env
      TOKENOVERFLOW_DATABASE_PASSWORD = data.aws_ssm_parameter.db_password.value
      TOKENOVERFLOW_EMBEDDING_API_KEY = data.aws_ssm_parameter.embedding_key.value
      TOKENOVERFLOW_WORKOS_API_KEY        = data.aws_ssm_parameter.auth_workos_api_key.value
      TOKENOVERFLOW_GITHUB_CLIENT_SECRET = data.aws_ssm_parameter.github_client_secret.value
    }
  }

  lifecycle {
    ignore_changes = [s3_key, s3_object_version, source_code_hash]
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
