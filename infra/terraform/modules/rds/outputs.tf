output "db_instance_endpoint" {
  description = "The connection endpoint of the RDS instance (host:port)."
  value       = module.rds.db_instance_endpoint
}

output "db_instance_address" {
  description = "The hostname of the RDS instance."
  value       = module.rds.db_instance_address
}

output "db_instance_port" {
  description = "The port of the RDS instance."
  value       = module.rds.db_instance_port
}

output "db_instance_name" {
  description = "The name of the database."
  value       = module.rds.db_instance_name
}

output "db_instance_username" {
  description = "The master username."
  value       = module.rds.db_instance_username
  sensitive   = true
}

output "db_instance_identifier" {
  description = "The RDS instance identifier."
  value       = module.rds.db_instance_identifier
}

output "db_instance_arn" {
  description = "The ARN of the RDS instance."
  value       = module.rds.db_instance_arn
}

output "security_group_id" {
  description = "The ID of the RDS security group."
  value       = aws_security_group.rds.id
}
