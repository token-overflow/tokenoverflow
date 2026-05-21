resource "aws_iam_role" "web" {
  name = "web"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_iam_role_policy_attachment" "web_basic" {
  role       = aws_iam_role.web.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy_attachment" "web_vpc" {
  role       = aws_iam_role.web.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaVPCAccessExecutionRole"
}

resource "aws_iam_role_policy" "web_ssm_read" {
  name = "ssm-read-secrets"
  role = aws_iam_role.web.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["ssm:GetParameters", "ssm:GetParameter"]
      Resource = [
        data.aws_ssm_parameter.web_oauth_client_secret.arn,
        data.aws_ssm_parameter.cookie_signing_key.arn,
      ]
    }]
  })
}
