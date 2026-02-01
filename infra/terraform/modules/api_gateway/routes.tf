resource "aws_apigatewayv2_integration" "lambda" {
  api_id                 = aws_apigatewayv2_api.main.id
  integration_type       = "AWS_PROXY"
  integration_uri        = var.lambda_invoke_arn
  payload_format_version = "2.0"
}

# Public routes (no authorizer)
resource "aws_apigatewayv2_route" "health" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "GET /health"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

resource "aws_apigatewayv2_route" "well-known" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "GET /.well-known/{proxy+}"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

# OAuth proxy routes (no authorizer): part of the MCP OAuth 2.1 flow,
# called before the client has a token.
resource "aws_apigatewayv2_route" "oauth2-authorize" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "GET /oauth2/authorize"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

resource "aws_apigatewayv2_route" "oauth2-token" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "POST /oauth2/token"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

resource "aws_apigatewayv2_route" "oauth2-register" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "POST /oauth2/register"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

# MCP route: POST only, no Gateway JWT authorizer.
#
# Why POST only (not ANY):
# The MCP server runs in **stateless mode** which only accepts POST
# and returns 405 for GET/DELETE. Routing only POST at the Gateway is
# defense-in-depth.
#
# Why no authorizer:
# The MCP Streamable HTTP protocol sends requests that the Gateway cannot
# validate (e.g., the initial unauthenticated handshake for OAuth
# discovery via 401 + WWW-Authenticate, and JSON-RPC session setup
# messages before the Bearer token is attached). Axum middleware handles
# JWT validation for authenticated MCP requests.
resource "aws_apigatewayv2_route" "mcp" {
  api_id    = aws_apigatewayv2_api.main.id
  route_key = "POST /mcp"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

# Everything else requires JWT auth (defense-in-depth: pre-filters invalid
# tokens before they reach the Lambda, reducing cold-start waste)
resource "aws_apigatewayv2_route" "default" {
  api_id             = aws_apigatewayv2_api.main.id
  route_key          = "$default"
  target             = "integrations/${aws_apigatewayv2_integration.lambda.id}"
  authorization_type = "JWT"
  authorizer_id      = aws_apigatewayv2_authorizer.jwt.id
}
