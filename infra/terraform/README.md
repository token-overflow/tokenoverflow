# Terraform

## Structure

- `infra/terraform/modules`: Terraform code for the infrastructure is defined
  here once. Anything in the infrastructure that's supposed to be different
  between environments is exposed as variables.
- `infra/terraform/live`: Instantiates the modules as Terragrunt
  [units](https://terragrunt.gruntwork.io/docs/features/units/) for each
  environment. Units should not have any Terraform code. They should only
  specify where to download the code and what values to pass into the variables.
- `infra/terraform/live/global`: The global
  [implicit stack](https://terragrunt.gruntwork.io/docs/features/stacks/#implicit-stacks-directory-based-organization)
  is for managing the core infrastructure used by all environments, such as AWS
  SSO. Requires an AWS profile with access to the root account.

## State Backend

Our S3 state backend is
[bootstrapped](https://terragrunt.gruntwork.io/docs/features/state-backend/#create-remote-state-resources-automatically)
by Terragrunt. Each environment's state is stored in a separate S3 bucket.

```shell
aws sso login --profile tokenoverflow-root-admin
cd infra/terraform/live/global/sso
export AWS_PROFILE=tokenoverflow-root-admin
terragrunt init --backend-bootstrap
```

## Run Stack

Use the `tg` helper function to log in to AWS using the correct profile and run
commands against the entire stack of your choice: `global`, `dev`, and `prod`.

```shell
source "${PROJECTS}/tokenoverflow/scripts/src/includes.sh"
tg plan global
```

## Provider

All modules use `hashicorp/aws` as the provider source. This resolves to the
OpenTofu registry (`registry.opentofu.org/hashicorp/aws`) and is the
recommended approach for compatibility with community modules.
