# Cloudflare DNS, proxy off. Cloudflare flattens the apex CNAME to A/AAAA at
# resolution time, so no ALIAS record is required. Both apex and www point at
# the CloudFront distribution domain.
resource "cloudflare_dns_record" "apex" {
  zone_id = var.cloudflare_zone_id
  name    = var.domain_apex
  type    = "CNAME"
  content = aws_cloudfront_distribution.landing.domain_name
  proxied = false
  ttl     = 1
  comment = "Landing apex (CloudFront, proxy off, flattened)"
}

resource "cloudflare_dns_record" "www" {
  zone_id = var.cloudflare_zone_id
  name    = var.www_domain
  type    = "CNAME"
  content = aws_cloudfront_distribution.landing.domain_name
  proxied = false
  ttl     = 1
  comment = "Landing www (CloudFront, proxy off, redirected to apex by viewer-request Function)"
}
