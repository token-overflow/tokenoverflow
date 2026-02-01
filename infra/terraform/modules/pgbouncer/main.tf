data "aws_ssm_parameter" "al2023-arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

data "aws_ssm_parameter" "db-password" {
  name = var.database_password_ssm_name
}

# Dedicated ENI with a fixed private IP. This ENI persists across ASG instance
# replacements, giving Lambda a stable target address. AL2023's
# amazon-ec2-net-utils automatically configures policy routing when attached.
resource "aws_network_interface" "pgbouncer" {
  subnet_id       = var.subnet_id
  private_ips     = [var.eni_private_ip]
  security_groups = [aws_security_group.pgbouncer.id]

  tags = {
    Name        = "pgbouncer"
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_launch_template" "pgbouncer" {
  name          = "pgbouncer"
  image_id      = data.aws_ssm_parameter.al2023-arm64.value
  instance_type = var.instance_type

  iam_instance_profile {
    arn = aws_iam_instance_profile.pgbouncer.arn
  }

  # Primary network interface (eth0) -- ASG-assigned IP, used for outbound
  # AWS service calls (SSM, package repos). The dedicated ENI (eth1) is
  # attached by user-data after boot. AL2023 handles routing automatically.
  network_interfaces {
    associate_public_ip_address = false
    security_groups             = [aws_security_group.pgbouncer.id]
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

  user_data = base64encode(templatefile("${path.module}/user_data.sh.tftpl", {
    eni_id            = aws_network_interface.pgbouncer.id
    rds_endpoint      = var.rds_endpoint
    database_name     = var.database_name
    database_user     = var.database_user
    database_password = data.aws_ssm_parameter.db-password.value
  }))

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name        = "pgbouncer"
      Environment = var.env_name
      ManagedBy   = "opentofu"
    }
  }

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_autoscaling_group" "pgbouncer" {
  name                = "pgbouncer"
  min_size            = 1
  max_size            = 1
  desired_capacity    = 1
  vpc_zone_identifier = [var.subnet_id]

  launch_template {
    id      = aws_launch_template.pgbouncer.id
    version = "$Latest"
  }

  tag {
    key                 = "Name"
    value               = "pgbouncer"
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

# IAM role for the PgBouncer instance. Needs:
# 1. SSM Session Manager access (for debugging, no SSH)
# 2. EC2 ENI attach/detach permissions (for user-data to attach the dedicated ENI)
resource "aws_iam_role" "pgbouncer" {
  name = "pgbouncer"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
    }]
  })

  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}

resource "aws_iam_role_policy_attachment" "ssm" {
  role       = aws_iam_role.pgbouncer.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_role_policy" "eni-manage" {
  name = "pgbouncer-eni-manage"
  role = aws_iam_role.pgbouncer.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ec2:AttachNetworkInterface",
          "ec2:DetachNetworkInterface"
        ]
        Resource = "*"
        Condition = {
          StringEquals = {
            "ec2:ResourceTag/Name" = "pgbouncer"
          }
        }
      },
      {
        Effect   = "Allow"
        Action   = "ec2:DescribeNetworkInterfaces"
        Resource = "*"
      }
    ]
  })
}

resource "aws_iam_instance_profile" "pgbouncer" {
  name = "pgbouncer"
  role = aws_iam_role.pgbouncer.name
}
