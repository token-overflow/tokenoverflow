# TODO(#157): https://github.com/token-overflow/tokenoverflow/issues/157
# TODO(#158): https://github.com/token-overflow/tokenoverflow/issues/158
# trivy:ignore:AWS-0086
# trivy:ignore:AWS-0087
# trivy:ignore:AWS-0091
# trivy:ignore:AWS-0093
# trivy:ignore:AWS-0132
resource "aws_s3_bucket" "deployments" {
  bucket = "tokenoverflow-lambda-${var.env_name}"

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_s3_bucket_versioning" "deployments" {
  bucket = aws_s3_bucket.deployments.id
  versioning_configuration {
    status = "Enabled"
  }
}
