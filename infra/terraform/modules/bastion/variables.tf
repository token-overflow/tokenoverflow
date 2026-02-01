variable "env_name" {
  description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC where the bastion will be deployed."
  type        = string
}

variable "subnet_id" {
  description = "ID of the private subnet where the bastion will be placed."
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type for the bastion."
  type        = string
  default     = "t4g.nano"
}

variable "ssh_public_key" {
  description = "SSH public key for the bastion key pair. Only required on initial creation or key rotation."
  type        = string
  default     = ""
}
