# CloudFront Origin Access Control: signs every CloudFront-to-S3 request with
# SigV4. Replaces the Cloudflare IP-allowlist + shared-secret pattern.
resource "aws_cloudfront_origin_access_control" "landing" {
  name                              = "landing"
  description                       = "OAC for the landing S3 origin"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

# Viewer-request CloudFront Function: rewrites pretty URLs to directory-index
# form and 301s www to apex. Source lives under apps/landing/ so it ships
# next to the Vitest harness that exercises it.
resource "aws_cloudfront_function" "viewer_request" {
  name    = "landing-viewer-request"
  runtime = "cloudfront-js-2.0"
  comment = "URL rewrite + www-to-apex 301 at the viewer-request event"
  publish = true
  code    = var.viewer_request_source
}

# Security headers.
resource "aws_cloudfront_response_headers_policy" "security" {
  name    = "landing-security-headers"
  comment = "Token Overflow security header baseline"

  security_headers_config {
    content_security_policy {
      content_security_policy = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'; require-trusted-types-for 'script'; upgrade-insecure-requests"
      override                = true
    }

    strict_transport_security {
      access_control_max_age_sec = 63072000
      include_subdomains         = true
      preload                    = true
      override                   = true
    }

    content_type_options {
      override = true
    }

    referrer_policy {
      referrer_policy = "strict-origin-when-cross-origin"
      override        = true
    }

    frame_options {
      frame_option = "DENY"
      override     = true
    }
  }

  custom_headers_config {
    items {
      header   = "permissions-policy"
      value    = "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=(), fullscreen=(self)"
      override = true
    }

    items {
      header   = "cross-origin-resource-policy"
      value    = "same-origin"
      override = true
    }

    items {
      header   = "cross-origin-opener-policy"
      value    = "same-origin"
      override = true
    }
  }
}

# Long-TTL cache policy for content-addressed /_astro/* assets.
resource "aws_cloudfront_cache_policy" "long" {
  name        = "landing-long-ttl"
  comment     = "One-year TTL for content-addressed /_astro/* assets"
  default_ttl = 31536000
  min_ttl     = 0
  max_ttl     = 31536000

  parameters_in_cache_key_and_forwarded_to_origin {
    enable_accept_encoding_brotli = true
    enable_accept_encoding_gzip   = true

    headers_config {
      header_behavior = "none"
    }

    cookies_config {
      cookie_behavior = "none"
    }

    query_strings_config {
      query_string_behavior = "none"
    }
  }
}

# Short-TTL cache policy for HTML and the apex root. Matches the current
# Cloudflare HTML revalidation cadence.
resource "aws_cloudfront_cache_policy" "short" {
  name        = "landing-short-ttl"
  comment     = "Five-minute TTL for HTML and the apex root"
  default_ttl = 300
  min_ttl     = 0
  max_ttl     = 300

  parameters_in_cache_key_and_forwarded_to_origin {
    enable_accept_encoding_brotli = true
    enable_accept_encoding_gzip   = true

    headers_config {
      header_behavior = "none"
    }

    cookies_config {
      cookie_behavior = "none"
    }

    query_strings_config {
      query_string_behavior = "none"
    }
  }
}

resource "aws_cloudfront_distribution" "landing" {
  enabled             = true
  is_ipv6_enabled     = true
  aliases             = [var.domain_apex, var.www_domain]
  default_root_object = "index.html"
  price_class         = "PriceClass_100"
  comment             = "landing"

  origin {
    domain_name              = aws_s3_bucket.landing.bucket_regional_domain_name
    origin_id                = "landing-s3"
    origin_access_control_id = aws_cloudfront_origin_access_control.landing.id
  }

  default_cache_behavior {
    target_origin_id           = "landing-s3"
    viewer_protocol_policy     = "redirect-to-https"
    compress                   = true
    allowed_methods            = ["GET", "HEAD"]
    cached_methods             = ["GET", "HEAD"]
    cache_policy_id            = aws_cloudfront_cache_policy.short.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.security.id

    function_association {
      event_type   = "viewer-request"
      function_arn = aws_cloudfront_function.viewer_request.arn
    }
  }

  # Hashed Astro assets bypass the URL rewrite Function: the path always
  # carries an extension so the rewrite would no-op anyway, and skipping the
  # association saves invocation cycles.
  ordered_cache_behavior {
    path_pattern               = "/_astro/*"
    target_origin_id           = "landing-s3"
    viewer_protocol_policy     = "redirect-to-https"
    compress                   = true
    allowed_methods            = ["GET", "HEAD"]
    cached_methods             = ["GET", "HEAD"]
    cache_policy_id            = aws_cloudfront_cache_policy.long.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.security.id
  }

  # Origin 403/404 -> branded /404.html with HTTP 404. Replaces the Worker.
  custom_error_response {
    error_code            = 403
    response_code         = 404
    response_page_path    = "/404.html"
    error_caching_min_ttl = 60
  }

  custom_error_response {
    error_code            = 404
    response_code         = 404
    response_page_path    = "/404.html"
    error_caching_min_ttl = 60
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.landing.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  lifecycle {
    create_before_destroy = true
  }
}
