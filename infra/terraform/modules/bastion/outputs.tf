output "security_group_id" {
  description = "The ID of the bastion security group."
  value       = aws_security_group.bastion.id
}

output "autoscaling_group_arn" {
  description = "The ARN of the bastion autoscaling group."
  value       = aws_autoscaling_group.bastion.arn
}

output "autoscaling_group_name" {
  description = "The name of the bastion autoscaling group."
  value       = aws_autoscaling_group.bastion.name
}

output "instance_profile_arn" {
  description = "The ARN of the bastion instance profile."
  value       = aws_iam_instance_profile.bastion.arn
}
