resource "aws_acm_certificate" "main" {
  for_each          = var.domains
  domain_name       = each.value.domain_name
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_acm_certificate_validation" "main" {
  for_each        = var.domains
  certificate_arn = aws_acm_certificate.main[each.key].arn
  validation_record_fqdns = [
    for dvo in aws_acm_certificate.main[each.key].domain_validation_options :
    dvo.resource_record_name
  ]

  depends_on = [cloudflare_dns_record.acm_validation]
}
