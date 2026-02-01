resource "aws_security_group" "rds" {
  name        = "${local.identifier}-rds"
  description = "Allow PostgreSQL access from private subnets only"
  vpc_id      = var.vpc_id

  tags = {
    Name        = "${local.identifier}-rds"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_vpc_security_group_ingress_rule" "postgresql" {
  for_each = toset(var.private_subnet_cidrs)

  security_group_id = aws_security_group.rds.id
  description       = "PostgreSQL from ${each.value}"
  from_port         = 5432
  to_port           = 5432
  ip_protocol       = "tcp"
  cidr_ipv4         = each.value
}

resource "aws_vpc_security_group_ingress_rule" "bastion" {
  count = var.bastion_security_group_id != "" ? 1 : 0

  security_group_id            = aws_security_group.rds.id
  description                  = "Allow PostgreSQL from bastion"
  from_port                    = 5432
  to_port                      = 5432
  ip_protocol                  = "tcp"
  referenced_security_group_id = var.bastion_security_group_id
}
