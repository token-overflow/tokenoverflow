resource "aws_apigatewayv2_api" "web" {
  name          = "web-api"
  protocol_type = "HTTP"

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_apigatewayv2_stage" "web" {
  api_id      = aws_apigatewayv2_api.web.id
  name        = "$default"
  auto_deploy = true

  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.web_api.arn
    format = jsonencode({
      requestId = "$context.requestId"
      ip        = "$context.identity.sourceIp"
      method    = "$context.httpMethod"
      path      = "$context.path"
      status    = "$context.status"
      latency   = "$context.responseLatency"
    })
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_cloudwatch_log_group" "web_api" {
  name              = "/aws/apigateway/web-http-api"
  retention_in_days = var.log_retention_days

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_lambda_permission" "apigw_web" {
  statement_id  = "AllowHTTPAPIInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.web.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.web.execution_arn}/*/*"
}
