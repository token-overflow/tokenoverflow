variable "env_name" {
  description = "Environment name (e.g., prod). Used for resource naming and tagging."
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC where PgBouncer will be deployed."
  type        = string
}

variable "subnet_id" {
  description = "ID of the private subnet where PgBouncer will be placed."
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type for the PgBouncer instance."
  type        = string
  default     = "t4g.nano"
}

variable "rds_endpoint" {
  description = "RDS instance endpoint hostname (e.g., main.xxx.us-east-1.rds.amazonaws.com)."
  type        = string
}

variable "rds_security_group_id" {
  description = "Security group ID of the RDS instance. Used to add an ingress rule allowing PgBouncer."
  type        = string
}

variable "lambda_security_group_id" {
  description = "Security group ID of the Lambda function. Used to allow ingress from Lambda to PgBouncer."
  type        = string
}

variable "bastion_security_group_id" {
  description = "Security group ID of the bastion. Used to allow ingress from bastion to PgBouncer. Set to empty string to skip."
  type        = string
  default     = ""
}

variable "database_name" {
  description = "Name of the PostgreSQL database to pool."
  type        = string
  default     = "tokenoverflow"
}

variable "database_user" {
  description = "PostgreSQL user for PgBouncer authentication."
  type        = string
  default     = "tokenoverflow"
}

variable "database_password_ssm_name" {
  description = "SSM Parameter Store name containing the database password."
  type        = string
}

variable "eni_private_ip" {
  description = "Fixed private IP address for the dedicated ENI. Must be within the subnet CIDR."
  type        = string
}
