resource "aws_organizations_policy" "deny_sso_instance" {
    name        = "DenySSOAccountInstances"
    description = "Prevent member accounts from creating IAM Identity Center account instances."
    type        = "SERVICE_CONTROL_POLICY"

    content = jsonencode({
        Version = "2012-10-17"
        Statement = [
            {
                Sid      = "DenyMemberAccountInstances"
                Effect   = "Deny"
                Action   = ["sso:CreateInstance"]
                Resource = "*"
            }
        ]
    })
}

resource "aws_organizations_policy_attachment" "deny_sso_instance" {
    policy_id = aws_organizations_policy.deny_sso_instance.id
    target_id = local.root_unit_id
}
