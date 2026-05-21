output "function_name" {
  description = "BFF Lambda function name"
  value       = aws_lambda_function.web.function_name
}

output "function_arn" {
  description = "BFF Lambda function ARN"
  value       = aws_lambda_function.web.arn
}

output "lambda_invoke_arn" {
  description = "BFF Lambda invoke ARN (for the DNS module to wire into the API Gateway integration)"
  value       = aws_lambda_function.web.invoke_arn
}

output "security_group_id" {
  description = "BFF Lambda security group ID"
  value       = aws_security_group.web.id
}

output "log_group_name" {
  description = "CloudWatch log group name for the BFF Lambda"
  value       = aws_cloudwatch_log_group.web.name
}

output "role_arn" {
  description = "BFF Lambda execution role ARN"
  value       = aws_iam_role.web.arn
}

output "api_id" {
  description = "BFF API Gateway HTTP API ID (for the DNS module to consume)"
  value       = aws_apigatewayv2_api.web.id
}

output "api_endpoint" {
  description = "BFF API Gateway default endpoint"
  value       = aws_apigatewayv2_api.web.api_endpoint
}

output "stage_name" {
  description = "BFF API Gateway stage name"
  value       = aws_apigatewayv2_stage.web.name
}

output "web_domain" {
  description = "Public hostname the BFF is served on"
  value       = var.web_domain
}

output "acm_certificate_arn" {
  description = "ACM certificate ARN issued for the BFF custom domain"
  value       = aws_acm_certificate.web.arn
}
