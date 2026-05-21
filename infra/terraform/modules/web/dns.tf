# ACM certificate for the BFF host. DNS-validated via Cloudflare records
# below. `create_before_destroy` lets renewals roll without dropping
# traffic on `app.tokenoverflow.io`.
resource "aws_acm_certificate" "web" {
  domain_name       = var.web_domain
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

# DNS validation records published to Cloudflare. ACM hands us one record
# per SAN; today we issue for the apex `app.` only, so this is a single
# record, but the for_each shape lets us add SANs later without code
# changes.
resource "cloudflare_dns_record" "cert_validation" {
  for_each = {
    for opt in aws_acm_certificate.web.domain_validation_options : opt.domain_name => {
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
  comment = "ACM DNS validation for the BFF certificate"
}

resource "aws_acm_certificate_validation" "web" {
  certificate_arn         = aws_acm_certificate.web.arn
  validation_record_fqdns = [for record in cloudflare_dns_record.cert_validation : record.name]
}

# API Gateway custom domain pointing at the regional endpoint. The
# `endpoint_type` matches the HTTP API, which is regional by default.
resource "aws_apigatewayv2_domain_name" "web" {
  domain_name = var.web_domain

  domain_name_configuration {
    certificate_arn = aws_acm_certificate_validation.web.certificate_arn
    endpoint_type   = "REGIONAL"
    security_policy = "TLS_1_2"
  }
}

resource "aws_apigatewayv2_api_mapping" "web" {
  api_id      = aws_apigatewayv2_api.web.id
  domain_name = aws_apigatewayv2_domain_name.web.domain_name
  stage       = aws_apigatewayv2_stage.web.name
}

# Cloudflare CNAME pointing the BFF host at the API Gateway regional
# domain. Proxied through Cloudflare so the public DNS answer is a
# Cloudflare anycast IP rather than the API Gateway domain directly.
resource "cloudflare_dns_record" "web" {
  zone_id = var.cloudflare_zone_id
  name    = var.web_domain
  type    = "CNAME"
  content = aws_apigatewayv2_domain_name.web.domain_name_configuration[0].target_domain_name
  proxied = true
  ttl     = 1
  comment = "BFF custom domain (CNAME -> API Gateway)"
}
