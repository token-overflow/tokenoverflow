data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

resource "aws_key_pair" "bastion" {
  key_name   = "bastion"
  public_key = var.ssh_public_key

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }

  lifecycle {
    ignore_changes = [public_key]
  }
}

resource "aws_security_group" "bastion" {
  name        = "bastion"
  description = "Security group for SSM bastion (no inbound needed)"
  vpc_id      = var.vpc_id

  tags = {
    Name        = "bastion"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_vpc_security_group_egress_rule" "all_outbound" {
  security_group_id = aws_security_group.bastion.id
  description       = "Allow all outbound traffic (SSM agent, DB access)"
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}

resource "aws_launch_template" "bastion" {
  name          = "bastion"
  image_id      = data.aws_ssm_parameter.al2023_arm64.value
  instance_type = var.instance_type
  key_name      = aws_key_pair.bastion.key_name

  iam_instance_profile {
    arn = aws_iam_instance_profile.bastion.arn
  }

  network_interfaces {
    associate_public_ip_address = false
    security_groups             = [aws_security_group.bastion.id]
  }

  metadata_options {
    http_endpoint = "enabled"
    http_tokens   = "required"
  }

  block_device_mappings {
    device_name = "/dev/xvda"
    ebs {
      volume_size = 8
      volume_type = "gp3"
      encrypted   = true
    }
  }

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name        = "bastion"
      Environment = var.env_name
      ManagedBy   = "opentofu"
    }
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_autoscaling_group" "bastion" {
  name                 = "bastion"
  min_size             = 1
  max_size             = 1
  desired_capacity     = 1
  vpc_zone_identifier  = [var.subnet_id]
  capacity_rebalance = true

  mixed_instances_policy {
    instances_distribution {
      on_demand_base_capacity                  = 1
      on_demand_percentage_above_base_capacity = 0
      spot_allocation_strategy                 = "capacity-optimized"
    }
    launch_template {
      launch_template_specification {
        launch_template_id = aws_launch_template.bastion.id
        version            = "$Latest"
      }
    }
  }

  tag {
    key                 = "Name"
    value               = "bastion"
    propagate_at_launch = true
  }

  tag {
    key                 = "Environment"
    value               = var.env_name
    propagate_at_launch = true
  }

  tag {
    key                 = "ManagedBy"
    value               = "opentofu"
    propagate_at_launch = true
  }
}
