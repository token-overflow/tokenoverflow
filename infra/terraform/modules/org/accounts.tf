resource "aws_organizations_account" "root" {
  name  = "tokenoverflow"
  email = "aws+root@tokenoverflow.io"
}

resource "aws_organizations_account" "dev" {
  name  = "Dev"
  email = "aws+dev@tokenoverflow.io"
}

resource "aws_organizations_account" "prod" {
  name  = "Prod"
  email = "aws+prod@tokenoverflow.io"
}
