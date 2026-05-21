resource "aws_cloudwatch_log_group" "web" {
  name              = "/aws/lambda/web"
  retention_in_days = var.log_retention_days

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
