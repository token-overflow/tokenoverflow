variable "env_name" {
  description = "Environment name (e.g., prod, dev). Used for resource naming and tagging."
  type        = string
}

variable "project_name" {
  description = "Project name. Used as prefix for the RDS instance identifier."
  type        = string
  default     = "tokenoverflow"
}

variable "vpc_id" {
  description = "ID of the VPC where the RDS security group will be created."
  type        = string
}

variable "database_subnet_group_name" {
  description = "Name of the database subnet group for RDS placement."
  type        = string
}

variable "private_subnet_cidrs" {
  description = "CIDR blocks of private subnets allowed to connect to RDS."
  type        = list(string)
}

variable "engine_version" {
  description = "PostgreSQL engine version (e.g., '17'). RDS resolves to the latest minor version."
  type        = string
  default     = "18.3"
}

variable "instance_class" {
  description = "RDS instance class (e.g., 'db.t4g.micro')."
  type        = string
  default     = "db.t4g.micro"
}

variable "allocated_storage" {
  description = "Initial allocated storage in GB."
  type        = number
  default     = 20
}

variable "max_allocated_storage" {
  description = "Maximum storage in GB for autoscaling. Set to 0 to disable."
  type        = number
  default     = 100
}

variable "db_name" {
  description = "Name of the initial database to create. Must be alphanumeric."
  type        = string
}

variable "identifier" {
  description = "RDS instance identifier. Defaults to project_name-env_name if not set."
  type        = string
  default     = ""
}

variable "username" {
  description = "Master username for the database."
  type        = string
  default     = "tokenoverflow"
}

variable "multi_az" {
  description = "Enable Multi-AZ deployment."
  type        = bool
  default     = false
}

variable "password_wo" {
  description = "Master password for the database. Write-only: never stored in state. Pass via TF_VAR_password_wo for initial creation or rotation."
  type        = string
  sensitive   = true
  ephemeral   = true
  default     = null
}

variable "password_wo_version" {
  description = "Increment to trigger a password update. OpenTofu cannot detect password changes since the value is not stored."
  type        = number
  default     = null
}

variable "bastion_security_group_id" {
  description = "Security group ID of the bastion. When set, allows PostgreSQL access from the bastion."
  type        = string
  default     = ""
}
