output "bucket_name" {
  value       = aws_s3_bucket.landing.id
  description = "S3 bucket name for aws s3 sync targets"
}

output "bucket_arn" {
  value       = aws_s3_bucket.landing.arn
  description = "S3 bucket ARN for downstream IAM"
}

output "cloudfront_distribution_id" {
  value       = aws_cloudfront_distribution.landing.id
  description = "CloudFront distribution ID for cache invalidations"
}

output "cloudfront_distribution_domain_name" {
  value       = aws_cloudfront_distribution.landing.domain_name
  description = "CloudFront distribution domain name (used by DNS records and for debugging)"
}

output "cloudflare_zone_id" {
  value       = var.cloudflare_zone_id
  description = "Echoed back so downstream tooling can read it from tg outputs"
}
