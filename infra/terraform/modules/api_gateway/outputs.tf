output "api_id" {
  description = "HTTP API ID"
  value       = aws_apigatewayv2_api.main.id
}

output "api_endpoint" {
  description = "HTTP API default endpoint"
  value       = aws_apigatewayv2_api.main.api_endpoint
}

output "execution_arn" {
  description = "HTTP API execution ARN"
  value       = aws_apigatewayv2_api.main.execution_arn
}

output "stage_name" {
  description = "Stage name"
  value       = aws_apigatewayv2_stage.prod.name
}
