locals {
  identifier = var.identifier != "" ? var.identifier : var.project_name
}

module "rds" {
  source  = "terraform-aws-modules/rds/aws"
  version = "7.0.0"

  identifier = local.identifier

  # Engine
  engine               = "postgres"
  engine_version       = var.engine_version
  family               = "postgres18"
  major_engine_version = "18"

  # Instance
  instance_class               = var.instance_class
  multi_az                     = var.multi_az

  # Storage
  allocated_storage     = var.allocated_storage
  max_allocated_storage = var.max_allocated_storage
  storage_type          = "gp3"
  storage_encrypted     = true
  # Uses default AWS managed key (aws/rds) - no kms_key_id needed

  # Database
  db_name  = var.db_name
  username = var.username
  port     = 5432

  # Credentials: write-only password (never stored in state)
  manage_master_user_password = false
  password_wo                 = var.password_wo
  password_wo_version         = var.password_wo_version

  # Network
  db_subnet_group_name   = var.database_subnet_group_name
  vpc_security_group_ids = [aws_security_group.rds.id]
  publicly_accessible    = false

  # Backups
  backup_retention_period          = 7
  backup_window                    = "03:00-04:00"
  maintenance_window               = "mon:04:00-mon:05:00"
  skip_final_snapshot              = false
  final_snapshot_identifier_prefix = "${local.identifier}-final"
  copy_tags_to_snapshot            = true
  deletion_protection              = true

  # Parameter group: use module-created default
  create_db_parameter_group = true

  # Tags
  tags = {
    Environment = var.env_name
    ManagedBy   = "opentofu"
  }
}
