resource "aws_ssoadmin_account_assignment" "administrators_root" {
  instance_arn       = local.sso_instance_arn
  permission_set_arn = aws_ssoadmin_permission_set.administrator_access.arn
  principal_id       = aws_identitystore_group.administrators.group_id
  principal_type     = "GROUP"
  target_id          = local.accounts.tokenoverflow
  target_type        = "AWS_ACCOUNT"
}

resource "aws_ssoadmin_account_assignment" "administrators_dev" {
  instance_arn       = local.sso_instance_arn
  permission_set_arn = aws_ssoadmin_permission_set.administrator_access.arn
  principal_id       = aws_identitystore_group.administrators.group_id
  principal_type     = "GROUP"
  target_id          = local.accounts.dev
  target_type        = "AWS_ACCOUNT"
}

resource "aws_ssoadmin_account_assignment" "administrators_prod" {
  instance_arn       = local.sso_instance_arn
  permission_set_arn = aws_ssoadmin_permission_set.administrator_access.arn
  principal_id       = aws_identitystore_group.administrators.group_id
  principal_type     = "GROUP"
  target_id          = local.accounts.prod
  target_type        = "AWS_ACCOUNT"
}
