resource "aws_organizations_organizational_unit" "engineering" {
  name      = "Engineering"
  parent_id = local.root_unit_id
}
