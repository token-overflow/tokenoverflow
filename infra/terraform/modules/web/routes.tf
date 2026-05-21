resource "aws_apigatewayv2_integration" "web_lambda" {
  api_id                 = aws_apigatewayv2_api.web.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.web.invoke_arn
  payload_format_version = "2.0"
}

# All BFF routes are public; the BFF runs the OAuth state machine itself
# and does not delegate auth to the gateway.

resource "aws_apigatewayv2_route" "web_auth_start" {
  api_id    = aws_apigatewayv2_api.web.id
  route_key = "GET /auth/start"
  target    = "integrations/${aws_apigatewayv2_integration.web_lambda.id}"
}

resource "aws_apigatewayv2_route" "web_auth_callback" {
  api_id    = aws_apigatewayv2_api.web.id
  route_key = "GET /auth/callback"
  target    = "integrations/${aws_apigatewayv2_integration.web_lambda.id}"
}

resource "aws_apigatewayv2_route" "web_health" {
  api_id    = aws_apigatewayv2_api.web.id
  route_key = "GET /health"
  target    = "integrations/${aws_apigatewayv2_integration.web_lambda.id}"
}
