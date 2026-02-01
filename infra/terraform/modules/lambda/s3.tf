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
