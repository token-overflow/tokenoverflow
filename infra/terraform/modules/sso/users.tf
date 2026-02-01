resource "aws_identitystore_user" "berkay" {
  identity_store_id = local.identity_store_id
  user_name         = "berkay"
  display_name      = "Berkay Ozturk"

  name {
    given_name  = "Berkay"
    family_name = "Ozturk"
  }

  emails {
    primary = true
    type    = "work"
    value   = "aws+berkay@tokenoverflow.io"
  }
}
