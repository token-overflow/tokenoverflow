locals {
  # Flatten domain_validation_options across all domains into a map
  # keyed by "domain_key.dvo_domain" for for_each
  acm_validations = merge([
    for dk, cert in aws_acm_certificate.main : {
      for dvo in cert.domain_validation_options :
      "${dk}.${dvo.domain_name}" => {
        domain_key   = dk
        record_name  = trimsuffix(dvo.resource_record_name, ".")
        record_value = trimsuffix(dvo.resource_record_value, ".")
        record_type  = dvo.resource_record_type
      }
    }
  ]...)
}

# ACM DNS validation records (must NOT be proxied)
resource "cloudflare_dns_record" "acm_validation" {
  for_each = local.acm_validations
  zone_id  = var.cloudflare_zone_id
  name     = each.value.record_name
  type     = each.value.record_type
  content  = each.value.record_value
  proxied  = false
  ttl      = 60
  comment  = "ACM validation for ${each.value.domain_key}"
}

# CNAME records pointing domains to their backends
resource "cloudflare_dns_record" "cname" {
  for_each = var.domains
  zone_id  = var.cloudflare_zone_id
  name     = each.value.domain_name
  type     = "CNAME"
  content  = aws_apigatewayv2_domain_name.main[each.key].domain_name_configuration[0].target_domain_name
  proxied  = each.value.proxied
  ttl      = each.value.proxied ? 1 : 300
  comment  = "Routes to ${each.value.backend.type} backend"
}
