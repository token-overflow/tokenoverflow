output "private_ip" {
  description = "The fixed private IP address of the PgBouncer ENI. Use this as the database host."
  value       = var.eni_private_ip
}

output "security_group_id" {
  description = "The ID of the PgBouncer security group."
  value       = aws_security_group.pgbouncer.id
}

output "eni_id" {
  description = "The ID of the dedicated ENI used by PgBouncer."
  value       = aws_network_interface.pgbouncer.id
}

output "autoscaling_group_name" {
  description = "The name of the PgBouncer autoscaling group."
  value       = aws_autoscaling_group.pgbouncer.name
}
