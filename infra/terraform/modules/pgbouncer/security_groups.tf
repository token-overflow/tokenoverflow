resource "aws_security_group" "pgbouncer" {
  name        = "pgbouncer"
  description = "Security group for PgBouncer connection pooler"
  vpc_id      = var.vpc_id

  tags = {
    Name        = "pgbouncer"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

# Ingress: Allow PgBouncer port from Lambda
resource "aws_vpc_security_group_ingress_rule" "pgbouncer-from-lambda" {
  security_group_id            = aws_security_group.pgbouncer.id
  description                  = "Allow PgBouncer from Lambda"
  from_port                    = 6432
  to_port                      = 6432
  ip_protocol                  = "tcp"
  referenced_security_group_id = var.lambda_security_group_id
}

# Ingress: Allow PgBouncer port from bastion (for DB admin via bastion -> PgBouncer -> RDS)
resource "aws_vpc_security_group_ingress_rule" "pgbouncer-from-bastion" {
  count = var.bastion_security_group_id != "" ? 1 : 0

  security_group_id            = aws_security_group.pgbouncer.id
  description                  = "Allow PgBouncer from bastion"
  from_port                    = 6432
  to_port                      = 6432
  ip_protocol                  = "tcp"
  referenced_security_group_id = var.bastion_security_group_id
}

# Egress: Allow all outbound traffic (SSM agent, package repos, RDS)
resource "aws_vpc_security_group_egress_rule" "all-outbound" {
  security_group_id = aws_security_group.pgbouncer.id
  description       = "Allow all outbound traffic (SSM agent, package repos, RDS)"
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}

# RDS SG: Allow ingress from PgBouncer (cross-module rule, same pattern as Lambda module)
resource "aws_vpc_security_group_ingress_rule" "rds-from-pgbouncer" {
  security_group_id            = var.rds_security_group_id
  description                  = "Allow PostgreSQL from PgBouncer"
  from_port                    = 5432
  to_port                      = 5432
  ip_protocol                  = "tcp"
  referenced_security_group_id = aws_security_group.pgbouncer.id
}
