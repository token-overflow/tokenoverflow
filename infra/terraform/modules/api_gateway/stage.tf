resource "aws_apigatewayv2_stage" "prod" {
  api_id      = aws_apigatewayv2_api.main.id
  name        = "$default"
  auto_deploy = true

  default_route_settings {
    throttling_rate_limit  = var.default_rate_limit
    throttling_burst_limit = var.default_burst_limit
  }

  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.api_gateway.arn
    format = jsonencode({
      requestId  = "$context.requestId"
      ip         = "$context.identity.sourceIp"
      method     = "$context.httpMethod"
      path       = "$context.path"
      status     = "$context.status"
      latency    = "$context.responseLatency"
      authStatus = "$context.authorizer.status"
      authError  = "$context.authorizer.error"
    })
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_cloudwatch_log_group" "api_gateway" {
  name              = "/aws/apigateway/main-http-api"
  retention_in_days = var.log_retention_days

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
