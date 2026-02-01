# Bucket policy: allow GetObject only when the request originates from the
# CloudFront distribution (Origin Access Control signs each request with
# SigV4 and the policy restricts to the specific distribution ARN). Direct
# requests to the S3 REST endpoint return 403 because the SourceArn check
# only matches CloudFront's signing principal.
data "aws_iam_policy_document" "landing" {
  statement {
    sid       = "allow-cloudfront-oac-reads"
    effect    = "Allow"
    actions   = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.landing.arn}/*"]

    principals {
      type        = "Service"
      identifiers = ["cloudfront.amazonaws.com"]
    }

    condition {
      test     = "StringEquals"
      variable = "AWS:SourceArn"
      values   = [aws_cloudfront_distribution.landing.arn]
    }
  }
}
