variable "env_name" {
  description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC where the NAT instance will be deployed."
  type        = string
}

variable "subnet_id" {
  description = "ID of the public subnet where the NAT instance will be placed."
  type        = string
}

variable "route_tables_ids" {
  description = "Map of route table names to IDs. These route tables will have 0.0.0.0/0 pointed to the fck-nat ENI."
  type        = map(string)
}

variable "instance_type" {
  description = "EC2 instance type for the NAT instance."
  type        = string
  default     = "t4g.nano"
}
