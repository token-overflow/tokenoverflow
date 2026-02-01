# AWS WAF Web ACL. Disabled by default because the rate-based rule and
# managed rule groups carry an ~$8/month floor regardless of traffic. Flip
# var.waf_enabled to true before public announce, then terragrunt apply.
resource "aws_wafv2_web_acl" "landing" {
  count = var.waf_enabled ? 1 : 0

  name  = "landing"
  scope = "CLOUDFRONT"

  default_action {
    allow {}
  }

  rule {
    name     = "aws-managed-common"
    priority = 1

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        vendor_name = "AWS"
        name        = "AWSManagedRulesCommonRuleSet"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "aws-managed-common"
      sampled_requests_enabled   = true
    }
  }

  rule {
    name     = "aws-managed-known-bad-inputs"
    priority = 2

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        vendor_name = "AWS"
        name        = "AWSManagedRulesKnownBadInputsRuleSet"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "aws-managed-known-bad-inputs"
      sampled_requests_enabled   = true
    }
  }

  rule {
    name     = "rate-limit"
    priority = 3

    action {
      block {}
    }

    statement {
      rate_based_statement {
        limit              = var.waf_rate_limit_per_5min
        aggregate_key_type = "IP"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "rate-limit"
      sampled_requests_enabled   = true
    }
  }

  visibility_config {
    cloudwatch_metrics_enabled = true
    metric_name                = "landing"
    sampled_requests_enabled   = true
  }
}

resource "aws_wafv2_web_acl_association" "landing" {
  count = var.waf_enabled ? 1 : 0

  resource_arn = aws_cloudfront_distribution.landing.arn
  web_acl_arn  = aws_wafv2_web_acl.landing[0].arn
}
