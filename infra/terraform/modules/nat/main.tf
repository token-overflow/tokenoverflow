resource "aws_eip" "nat" {
  domain = "vpc"

  tags = {
    Name        = "nat"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

module "fck_nat" {
  source  = "RaJiska/fck-nat/aws"
  version = "1.4.0"

  name      = "nat"
  vpc_id    = var.vpc_id
  subnet_id = var.subnet_id

  # HA mode: NAT instance wrapped in ASG (min=max=desired=1)
  # ASG auto-replaces unhealthy instances. Floating ENI persists across replacements.
  ha_mode            = true
  eip_allocation_ids = [aws_eip.nat.allocation_id]

  # Instance configuration
  instance_type      = var.instance_type
  use_spot_instances = false

  # Route table updates: point 0.0.0.0/0 to the fck-nat ENI
  update_route_tables = true
  route_tables_ids    = var.route_tables_ids

  # CloudWatch agent: disabled ($17/month is too expensive for a $3/month instance)
  use_cloudwatch_agent = false

  # SSM agent: enabled for remote access without SSH
  attach_ssm_policy = true

  # SSH: disabled (use SSM Session Manager instead)
  use_ssh = false

  # EBS encryption: enabled (uses default AWS managed key)
  encryption = true

  # Tags
  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
