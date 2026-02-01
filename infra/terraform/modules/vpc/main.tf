module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "6.6.0"

  name = var.env_name
  cidr = var.vpc_cidr
  azs  = var.azs

  # Tier 1: Public subnets
  public_subnets = var.public_subnets
  public_subnet_names = [
    for i, az in var.azs : "${var.env_name}-public-${az}"
  ]

  # Tier 2: Private subnets
  private_subnets = var.private_subnets
  private_subnet_names = [
    for i, az in var.azs : "${var.env_name}-private-${az}"
  ]

  # Tier 3: Isolated subnets (databases)
  database_subnets = var.database_subnets
  database_subnet_names = [
    for i, az in var.azs : "${var.env_name}-isolated-${az}"
  ]

  # Database subnet group for RDS
  create_database_subnet_group       = true
  database_subnet_group_name         = var.env_name
  create_database_subnet_route_table = true

  # No internet access for database subnets (isolated)
  create_database_internet_gateway_route = false
  create_database_nat_gateway_route      = false

  # No NAT Gateway (deferred)
  enable_nat_gateway = false

  # DNS
  enable_dns_hostnames = true
  enable_dns_support   = true

  # Tags
  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }

  public_subnet_tags = {
    Tier = "public"
  }

  private_subnet_tags = {
    Tier = "private"
  }

  database_subnet_tags = {
    Tier = "isolated"
  }
}
