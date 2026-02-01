locals {
  api_gateway_domains = {
    for k, v in var.domains : k => v if v.backend.type == "api_gateway"
  }
}

resource "aws_apigatewayv2_domain_name" "main" {
  for_each    = local.api_gateway_domains
  domain_name = each.value.domain_name

  domain_name_configuration {
    certificate_arn = aws_acm_certificate_validation.main[each.key].certificate_arn
    endpoint_type   = "REGIONAL"
    security_policy = "TLS_1_2"
  }
}

resource "aws_apigatewayv2_api_mapping" "main" {
  for_each    = local.api_gateway_domains
  api_id      = each.value.backend.api_id
  domain_name = aws_apigatewayv2_domain_name.main[each.key].domain_name
  stage       = each.value.backend.stage_name
}
