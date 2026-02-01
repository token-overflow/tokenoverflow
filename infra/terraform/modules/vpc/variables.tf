variable "env_name" {
  description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
  type        = string
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC."
  type        = string
}

variable "azs" {
  description = "List of availability zones."
  type        = list(string)
}

variable "public_subnets" {
  description = "List of CIDR blocks for public subnets (one per AZ)."
  type        = list(string)
}

variable "private_subnets" {
  description = "List of CIDR blocks for private subnets (one per AZ)."
  type        = list(string)
}

variable "database_subnets" {
  description = "List of CIDR blocks for isolated/database subnets (one per AZ)."
  type        = list(string)
}
