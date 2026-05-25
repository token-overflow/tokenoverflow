resource "aws_security_group" "web" {
  name        = "web-lambda"
  description = "Security group for the BFF (web) Lambda. Egress only; no RDS ingress."
  vpc_id      = var.vpc_id

  tags = {
    Name        = "web-lambda"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

# TODO(#156): https://github.com/token-overflow/tokenoverflow/issues/156
# trivy:ignore:AWS-0104
resource "aws_vpc_security_group_egress_rule" "web_https_outbound" {
  security_group_id = aws_security_group.web.id
  description       = "Allow HTTPS to AuthKit and the API"
  from_port         = 443
  to_port           = 443
  ip_protocol       = "tcp"
  cidr_ipv4         = "0.0.0.0/0"
}
