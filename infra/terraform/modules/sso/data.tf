locals {
  identity_store_id = tolist(data.aws_ssoadmin_instances.tokenoverflow.identity_store_ids)[0]
  sso_instance_arn  = tolist(data.aws_ssoadmin_instances.tokenoverflow.arns)[0]

  accounts = {
    for account in data.aws_organizations_organization.tokenoverflow.accounts :
    lower(account.name) => account.id
  }
}

data "aws_ssoadmin_instances" "tokenoverflow" {}

data "aws_organizations_organization" "tokenoverflow" {}
