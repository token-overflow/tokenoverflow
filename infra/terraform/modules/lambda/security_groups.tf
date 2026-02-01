resource "aws_security_group" "lambda" {
  name        = "api-lambda"
  description = "Security group for Lambda function"
  vpc_id      = var.vpc_id

  tags = {
    Name        = "api-lambda"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_vpc_security_group_egress_rule" "lambda_all_outbound" {
  security_group_id = aws_security_group.lambda.id
  description       = "Allow all outbound traffic"
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}

resource "aws_vpc_security_group_ingress_rule" "rds_from_lambda" {
  security_group_id            = var.rds_security_group_id
  description                  = "Allow PostgreSQL from Lambda"
  from_port                    = 5432
  to_port                      = 5432
  ip_protocol                  = "tcp"
  referenced_security_group_id = aws_security_group.lambda.id
}
