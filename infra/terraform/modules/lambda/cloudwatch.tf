resource "aws_cloudwatch_log_group" "api" {
  name              = "/aws/lambda/api"
  retention_in_days = var.log_retention_days

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
