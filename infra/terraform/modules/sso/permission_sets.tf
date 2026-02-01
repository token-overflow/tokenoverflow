resource "aws_ssoadmin_permission_set" "administrator_access" {
  instance_arn = local.sso_instance_arn
  name         = "AdministratorAccess"
  description  = "Provides full access to AWS services and resources."
}

resource "aws_ssoadmin_managed_policy_attachment" "administrator_access" {
  instance_arn       = local.sso_instance_arn
  permission_set_arn = aws_ssoadmin_permission_set.administrator_access.arn
  managed_policy_arn = "arn:aws:iam::aws:policy/AdministratorAccess"
}
