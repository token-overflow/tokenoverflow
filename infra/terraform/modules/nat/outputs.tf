output "eni_id" {
  description = "The ID of the static ENI used by the fck-nat instance."
  value       = module.fck_nat.eni_id
}

output "security_group_ids" {
  description = "List of security group IDs used by fck-nat ENIs."
  value       = module.fck_nat.security_group_ids
}

output "role_arn" {
  description = "The ARN of the IAM role used by the fck-nat instance profile."
  value       = module.fck_nat.role_arn
}

output "autoscaling_group_arn" {
  description = "The ARN of the autoscaling group managing the fck-nat instance."
  value       = module.fck_nat.autoscaling_group_arn
}

output "launch_template_id" {
  description = "The ID of the launch template used to spawn fck-nat instances."
  value       = module.fck_nat.launch_template_id
}
