output "domain_names" {
  description = "Map of domain key to custom domain name"
  value       = { for k, v in var.domains : k => v.domain_name }
}

output "acm_certificate_arns" {
  description = "Map of domain key to ACM certificate ARN"
  value       = { for k, v in aws_acm_certificate.main : k => v.arn }
}
