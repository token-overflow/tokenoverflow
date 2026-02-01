# VPC

The VPC module (`modules/vpc/`) provisions a 3-tier network architecture using
the `terraform-aws-modules/vpc/aws` community module:

| Tier     | Subnets        | Purpose                       |
|----------|----------------|-------------------------------|
| Public   | 2 (one per AZ) | ALB, NAT GW (future)          |
| Private  | 2 (one per AZ) | Application servers           |
| Isolated | 2 (one per AZ) | Databases (no internet route) |
