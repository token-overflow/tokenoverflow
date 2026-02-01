# Design: scp-deny-sso-instance

## Architecture Overview

### Goal

Add a Service Control Policy (SCP) to the AWS Organization root that prevents
member accounts from creating new IAM Identity Center instances via the
`sso:CreateInstance` API action. This hardens the organization by ensuring that
identity management remains centralized in the management account's existing
IAM Identity Center instance.

### Scope

This design covers:

- An SCP that denies `sso:CreateInstance` for all member accounts
- OpenTofu resources for the policy and its attachment to the organization root
- Integration into the existing `aws-organizations` module

This design does NOT cover:

- Additional SCP policies (e.g., deny root user actions, region restrictions)
- Changes to IAM Identity Center configuration
- Changes to the `aws-sso` module
- SCP guardrails for any other AWS service

### Background

IAM Identity Center supports two types of instances: an **organization
instance** (managed from the management account) and **account instances**
(created within individual member accounts). Account instances allow member
accounts to set up their own identity providers and permission sets, bypassing
the centralized governance provided by the organization instance.

AWS recommends using an SCP to prevent member accounts from creating account
instances. SCPs do not affect users or roles in the management account, so the
management account retains the ability to manage the organization instance
without any exception conditions in the policy.

### SCP Policy

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "DenyMemberAccountInstances",
            "Effect": "Deny",
            "Action": [
                "sso:CreateInstance"
            ],
            "Resource": "*"
        }
    ]
}
```

This policy is intentionally simple. No `Condition` block is needed because:

1. SCPs never apply to the management account -- this is an inherent property
   of AWS Organizations, not something the policy needs to account for.
2. There are no delegated administrator accounts for IAM Identity Center in
   this organization that would need an exception.

### Attachment Target

The SCP will be attached to the **organization root** (`r-8qwc`). This means
it applies to all organizational units and all member accounts (Dev, Prod)
within the organization. Attaching at the root is the correct level because:

- The policy should apply uniformly to all member accounts.
- Attaching to individual OUs or accounts would require updating the attachment
  whenever new OUs or accounts are added.
- The management account (tokenoverflow, `058170691494`) is automatically
  excluded by AWS.

### Where This Lives in the Codebase

The SCP is a property of the AWS Organization, not of any individual
environment. It belongs in the existing `aws-organizations` module, which
already manages the organization resource, organizational units, and accounts.
Adding the SCP here keeps all organization-level governance in one place and
avoids creating a new module for a single policy resource and its attachment.

#### Alternatives Considered

| Approach                                          | Description                                                                                                        | Pros                                                                                               | Cons                                                                                                                                                  |
|---------------------------------------------------|--------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| **A: Add to `aws-organizations` module (chosen)** | Add `aws_organizations_policy` and `aws_organizations_policy_attachment` resources directly to the existing module | Consistent with existing pattern; no new module/unit; SCP is tightly coupled to the org it governs | Module grows slightly; if many SCPs are added later, the module could become large                                                                    |
| **B: New `scp` module + Terragrunt unit**         | Create `modules/scp/` and `live/global/scp/terragrunt.hcl`                                                         | Clean separation of concerns; easier to manage many policies independently                         | Overhead for a single policy; introduces a cross-module dependency (needs the org root ID); inconsistent with how org resources are currently managed |
| **C: New `scp` module, existing Terragrunt unit** | Create a separate module but reference it from the existing `aws-organizations` Terragrunt unit                    | Modular code without extra Terragrunt unit                                                         | Terragrunt units map 1:1 to modules in this project; mixing two module sources in one unit is not supported                                           |

**Decision:** Option A. The `aws-organizations` module already owns the
organization, its OUs, and its accounts. SCPs are an intrinsic part of
organization governance. Adding the SCP here follows the existing pattern. If
the number of SCPs grows significantly in the future, extraction into a
dedicated module can be done at that time.

---

## Interfaces

This section documents every file that will be created or modified, and the
contracts between them.

### Modified Files

#### `infra/terraform/modules/aws-organizations/main.tf`

The `aws_organizations_organization` resource already enables
`SERVICE_CONTROL_POLICY` in `enabled_policy_types`. No changes to this
resource are needed.

### New Files

#### `infra/terraform/modules/aws-organizations/policies.tf`

A new file is created to house all SCP resources. Separating policies from
the organization definition in `main.tf` keeps the module organized as the
number of SCPs grows over time.

```hcl
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
```

The `jsonencode()` function is used instead of a heredoc string to ensure the
JSON is always syntactically valid and consistently formatted.

### Files NOT Modified

| File                                           | Reason                                                                                                             |
|------------------------------------------------|--------------------------------------------------------------------------------------------------------------------|
| `modules/aws-organizations/main.tf`            | SCP resources live in the new `policies.tf` file; `main.tf` is unchanged                                          |
| `modules/aws-organizations/data.tf`            | `local.root_unit_id` already exists and is derived from `aws_organizations_organization.tokenoverflow.roots[0].id` |
| `modules/aws-organizations/units.tf`           | No changes to organizational units                                                                                 |
| `modules/aws-organizations/accounts.tf`        | No changes to accounts                                                                                             |
| `modules/aws-organizations/imports.tf`         | No imports needed -- this is a new resource, not an existing one being brought under IaC                           |
| `modules/aws-sso/*`                            | The SCP is an organization-level policy, not an SSO configuration change                                           |
| `live/global/aws-organizations/terragrunt.hcl` | No new inputs needed -- the module has no new variables                                                            |
| `live/global/env.hcl`                          | No changes                                                                                                         |
| `live/root.hcl`                                | No changes                                                                                                         |
| `live/prod/*`                                  | SCPs are global, not environment-specific                                                                          |
| `live/dev/*`                                   | SCPs are global, not environment-specific                                                                          |

---

## Logic

This section defines the exact sequence of operations to implement the SCP.

### Phase 1: Add the SCP resources to the aws-organizations module

**Step 1.1:** Create `infra/terraform/modules/aws-organizations/policies.tf`.

Add the two new resources (`aws_organizations_policy.deny_sso_instance`
and `aws_organizations_policy_attachment.deny_sso_instance`). See the
Interfaces section for the exact HCL content.

**Step 1.2:** Validate the module syntax.

```bash
cd infra/terraform/modules/aws-organizations
tofu validate
```

This verifies that the HCL is syntactically correct and that references to
`local.root_unit_id` resolve properly.

### Phase 2: Plan and apply

**Step 2.1:** Log in to the root management account.

```bash
aws sso login --profile tokenoverflow-root-admin
```

**Step 2.2:** Run a plan to verify the changes.

```bash
source scripts/src/includes.sh
tg plan global
```

Expected plan output for `aws-organizations`:

- 1 resource to add: `aws_organizations_policy.deny_sso_instance`
- 1 resource to add: `aws_organizations_policy_attachment.deny_sso_instance`
- 0 resources to change
- 0 resources to destroy

The `aws-sso` unit should show "No changes."

**Step 2.3:** Apply the changes.

```bash
tg apply global
```

Confirm the apply prompt. The SCP is created and attached to the organization
root in a single operation.

### Phase 3: Verify

**Step 3.1:** Verify the SCP exists in AWS Organizations.

```bash
aws organizations list-policies \
  --filter SERVICE_CONTROL_POLICY \
  --profile tokenoverflow-root-admin \
  --region us-east-1
```

The output should include a policy named `DenySSOAccountInstances`.

**Step 3.2:** Verify the SCP is attached to the organization root.

```bash
aws organizations list-policies-for-target \
  --target-id r-8qwc \
  --filter SERVICE_CONTROL_POLICY \
  --profile tokenoverflow-root-admin \
  --region us-east-1
```

The output should include both the default `FullAWSAccess` policy and the
new `DenySSOAccountInstances` policy.

### Phase 4: Commit

```bash
git add infra/terraform/modules/aws-organizations/main.tf
git commit -m "infra: add SCP to deny IAM Identity Center account instances"
```

---

## Edge Cases & Constraints

### 1. SCPs do not affect the management account

**Risk:** Someone might expect the SCP to also prevent the management account
from calling `sso:CreateInstance`.

**Mitigation:** This is expected AWS behavior. SCPs never restrict the
management account. The management account must retain the ability to manage
the organization-level IAM Identity Center instance. This is documented in the
Architecture Overview section.

### 2. The default FullAWSAccess SCP must remain attached

**Risk:** AWS Organizations attaches a default `FullAWSAccess` SCP to the root
when SCPs are enabled. If this default policy is removed, all member accounts
lose access to all AWS services (since SCPs are deny-by-default when no Allow
policy exists).

**Mitigation:** This design does not remove or modify the default
`FullAWSAccess` policy. It only adds a new Deny policy alongside it. Deny
statements in SCPs always override Allow statements, so the new policy takes
effect without needing to modify the existing one. The `FullAWSAccess` policy
is not managed by OpenTofu and should not be imported or touched.

### 3. Existing member account SSO instances

**Risk:** If a member account already has an IAM Identity Center account
instance, the SCP does not remove it. The SCP only prevents creation of new
instances.

**Mitigation:** No action needed. There are no existing account-level IAM
Identity Center instances in the Dev or Prod accounts. The organization uses a
single organization-level instance managed from the management account, as
configured in the `aws-sso` module.

### 4. Future delegated administrator accounts

**Risk:** If a delegated administrator account for IAM Identity Center is
designated in the future, it may need to call `sso:CreateInstance`. The current
policy would block this.

**Mitigation:** If a delegated administrator is added later, the SCP can be
updated with a `Condition` block to exclude that account:

```json
{
    "Condition": {
        "StringNotEquals": {
            "aws:PrincipalAccount": [
                "<delegated-admin-account-id>"
            ]
        }
    }
}
```

This is a future concern and is out of scope for this design. The current
organization has no delegated administrators.

### 5. SCP character limit

**Risk:** AWS SCPs have a maximum size of 5,120 characters.

**Mitigation:** The policy in this design is approximately 200 characters. This
is well within the limit.

### 6. SCP count limit

**Risk:** AWS Organizations has a default limit of 5 SCPs per organization.

**Mitigation:** This adds 1 SCP. Combined with the default `FullAWSAccess`
policy, the total is 2 out of 5. If more SCPs are needed in the future, the
limit can be increased via an AWS support request, or multiple policy
statements can be combined into a single SCP.

### 7. Plan shows changes to existing resources

**Risk:** Running `tg plan global` might show unexpected changes to existing
`aws-organizations` or `aws-sso` resources due to provider drift or version
differences.

**Mitigation:** The plan output must be carefully reviewed before applying.
Only the 2 new resources should appear. If any existing resources show changes,
stop and investigate before proceeding.

---

## Test Plan

### Verification Checklist

Infrastructure changes are verified through plan output inspection,
post-apply validation, and behavioral verification using the AWS CLI. There
are no application-level tests since this is purely an organizational policy
change.

#### 1. Plan shows exactly 2 new resources

```bash
source scripts/src/includes.sh
tg plan global
```

**Success:** The plan for `aws-organizations` shows:

- `aws_organizations_policy.deny_sso_instance` will be created
- `aws_organizations_policy_attachment.deny_sso_instance` will be created
- No other resources are added, changed, or destroyed
- `aws-sso` shows "No changes"

#### 2. Post-apply: SCP exists in AWS Organizations

```bash
aws organizations list-policies \
  --filter SERVICE_CONTROL_POLICY \
  --profile tokenoverflow-root-admin \
  --region us-east-1 \
  --query 'Policies[?Name==`DenySSOAccountInstances`]'
```

**Success:** Returns a single policy object with `Name =
DenySSOAccountInstances` and `Type = SERVICE_CONTROL_POLICY`.

#### 3. Post-apply: SCP is attached to the organization root

```bash
aws organizations list-policies-for-target \
  --target-id r-8qwc \
  --filter SERVICE_CONTROL_POLICY \
  --profile tokenoverflow-root-admin \
  --region us-east-1 \
  --query 'Policies[?Name==`DenySSOAccountInstances`]'
```

**Success:** Returns the `DenySSOAccountInstances` policy, confirming it is
attached to the root.

#### 4. Post-apply: SCP content is correct

```bash
POLICY_ID=$(aws organizations list-policies \
  --filter SERVICE_CONTROL_POLICY \
  --profile tokenoverflow-root-admin \
  --region us-east-1 \
  --query 'Policies[?Name==`DenySSOAccountInstances`].Id' \
  --output text)

aws organizations describe-policy \
  --policy-id "$POLICY_ID" \
  --profile tokenoverflow-root-admin \
  --region us-east-1 \
  --query 'Policy.Content' \
  --output text | python3 -m json.tool
```

**Success:** The policy content matches the JSON defined in the Architecture
Overview section: a single Deny statement for `sso:CreateInstance` on
`Resource: *`.

#### 5. Behavioral verification: member account cannot create SSO instance

```bash
aws sso-admin create-instance \
  --profile tokenoverflow-dev-admin \
  --region us-east-1
```

**Success:** The command fails with an `AccessDeniedException` or
`OrganizationAccessDeniedException` error, confirming the SCP is enforced.

**Note:** This is a destructive test -- it attempts to create a resource that
should be denied. If the SCP is not working, this command would create an
unwanted IAM Identity Center instance in the Dev account. Run this test only
after confirming the SCP is attached (tests 2 and 3).

#### 6. Behavioral verification: management account is unaffected

No test needed. SCPs do not apply to the management account by definition.
The existing IAM Identity Center organization instance (managed by the
`aws-sso` module) continues to function normally.

---

## Documentation Changes

### Files to Update

| File                                                  | Change                                                   |
|-------------------------------------------------------|----------------------------------------------------------|
| `infra/terraform/modules/aws-organizations/README.md` | Add SCP section documenting the deny-SSO-instance policy |

### Content to Add to `infra/terraform/modules/aws-organizations/README.md`

Append the following after the existing account tree:

```markdown
## Service Control Policies

| Policy Name | Target | Effect |
|-------------|--------|--------|
| DenySSOAccountInstances | Root (all member accounts) | Denies `sso:CreateInstance` to prevent member accounts from creating IAM Identity Center account instances |
```

### Files NOT Updated

Historical design documents are not updated. They are a snapshot of the
codebase at the time they were written.

---

## Development Environment Changes

### Brewfile

No changes needed. `tofuenv`, `terragrunt`, and `tflint` are already
installed.

### Environment Variables

No new environment variables are introduced.

### Setup Flow

No changes. The `source scripts/src/includes.sh && setup` command continues
to work. The `tg` helper function already supports the `global` environment.

---

## Tasks

### Task 1: Add SCP resources to the aws-organizations module

**What:** Create `infra/terraform/modules/aws-organizations/policies.tf` with
the `aws_organizations_policy` and `aws_organizations_policy_attachment`
resources.

**Steps:**

1. Create `infra/terraform/modules/aws-organizations/policies.tf` with the two
   new resources. See the Interfaces section for the exact HCL content.

**Success:** The new file contains the SCP policy and its attachment. `main.tf`
is unchanged. The HCL is syntactically valid.

### Task 2: Plan, verify, and apply

**What:** Run the OpenTofu plan to verify the changes, then apply.

**Steps:**

1. Log in: `aws sso login --profile tokenoverflow-root-admin`
2. Plan: `source scripts/src/includes.sh && tg plan global`
3. Verify the plan shows exactly 2 new resources (policy + attachment) and no
   changes to existing resources
4. Apply: `tg apply global`
5. Verify SCP exists:

   ```bash
   aws organizations list-policies \
     --filter SERVICE_CONTROL_POLICY \
     --profile tokenoverflow-root-admin \
     --region us-east-1 \
     --query 'Policies[?Name==`DenySSOAccountInstances`]'
   ```

6. Verify SCP is attached to root:

   ```bash
   aws organizations list-policies-for-target \
     --target-id r-8qwc \
     --filter SERVICE_CONTROL_POLICY \
     --profile tokenoverflow-root-admin \
     --region us-east-1 \
     --query 'Policies[?Name==`DenySSOAccountInstances`]'
   ```

7. Verify SCP blocks member accounts (optional, see Test Plan section 5)
8. Commit:

   ```bash
   git add infra/terraform/modules/aws-organizations/policies.tf
   git commit -m "infra: add SCP to deny IAM Identity Center account instances"
   ```

**Success:** SCP is created, attached to root, and member accounts are denied
from calling `sso:CreateInstance`. Plan showed 2 additions and 0 changes to
existing resources.

### Task 3: Update documentation

**What:** Update the aws-organizations module README to document the new SCP.

**Steps:**

1. Edit `infra/terraform/modules/aws-organizations/README.md`: add the SCP
   table (see Documentation Changes section)
2. Commit:

   ```bash
   git add infra/terraform/modules/aws-organizations/README.md
   git commit -m "docs: document DenySSOAccountInstances SCP in aws-organizations README"
   ```

**Success:** README includes the SCP table with policy name, target, and
effect.
