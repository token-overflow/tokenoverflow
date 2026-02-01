# Administrators

resource "aws_identitystore_group" "administrators" {
  identity_store_id = local.identity_store_id
  display_name      = "administrators"
}

resource "aws_identitystore_group_membership" "administrators_berkay" {
  identity_store_id = local.identity_store_id
  group_id          = aws_identitystore_group.administrators.group_id
  member_id         = aws_identitystore_user.berkay.user_id
}
