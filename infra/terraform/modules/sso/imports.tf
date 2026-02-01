import {
  to = aws_identitystore_user.berkay
  id = "d-906600a5bd/54f8e4b8-b091-70ca-749e-ff8f1420e0a2"
}

import {
  to = aws_identitystore_group.administrators
  id = "d-906600a5bd/34d85488-d0e1-7051-7acc-5364d8ee8c7b"
}

import {
  to = aws_identitystore_group_membership.administrators_berkay
  id = "d-906600a5bd/64e80408-f051-7001-0814-e5691aa8ec83"
}

import {
  to = aws_ssoadmin_permission_set.administrator_access
  id = "arn:aws:sso:::permissionSet/ssoins-7223c7e4f3123a32/ps-722307c26ecd15f2,arn:aws:sso:::instance/ssoins-7223c7e4f3123a32"
}

import {
  to = aws_ssoadmin_managed_policy_attachment.administrator_access
  id = "arn:aws:iam::aws:policy/AdministratorAccess,arn:aws:sso:::permissionSet/ssoins-7223c7e4f3123a32/ps-722307c26ecd15f2,arn:aws:sso:::instance/ssoins-7223c7e4f3123a32"
}

import {
  to = aws_ssoadmin_account_assignment.administrators_root
  id = "34d85488-d0e1-7051-7acc-5364d8ee8c7b,GROUP,058170691494,AWS_ACCOUNT,arn:aws:sso:::permissionSet/ssoins-7223c7e4f3123a32/ps-722307c26ecd15f2,arn:aws:sso:::instance/ssoins-7223c7e4f3123a32"
}
