# ACM certificate covering the apex and the www subdomain. Issued in
# us-east-1 because CloudFront only consumes certs from that region. DNS
# validation records are written into the Cloudflare zone via
# cloudflare_dns_record below.
resource "aws_acm_certificate" "landing" {
  domain_name               = var.domain_apex
  subject_alternative_names = [var.www_domain]
  validation_method         = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "cloudflare_dns_record" "cert_validation" {
  for_each = {
    for opt in aws_acm_certificate.landing.domain_validation_options : opt.domain_name => {
      name  = opt.resource_record_name
      type  = opt.resource_record_type
      value = opt.resource_record_value
    }
  }

  zone_id = var.cloudflare_zone_id
  name    = each.value.name
  type    = each.value.type
  content = each.value.value
  proxied = false
  ttl     = 60
  comment = "ACM DNS validation for landing certificate"
}

resource "aws_acm_certificate_validation" "landing" {
  certificate_arn         = aws_acm_certificate.landing.arn
  validation_record_fqdns = [for record in cloudflare_dns_record.cert_validation : record.name]
}
