resource "aws_apigatewayv2_api" "main" {
  name          = "main-api"
  protocol_type = "HTTP"

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
